//! Socket values, ordinary channel methods, and options-table receive binding.
use super::host::MittensHost;
use crate::engine::ecs::{
    ComponentId,
    component::{UdpBinding, UdpChannelComponent, UdpSocketComponent},
};
use crate::engine::transform::TransformSpace;
use crate::engine::udp_channels::Schema;
use meow_meow_script as mms;

pub(crate) fn supports(ty: &str) -> bool {
    matches!(ty, "UdpSocket" | "UdpPublication" | "UdpSubscription")
}

impl MittensHost<'_, '_> {
    pub(super) fn invoke_udp(
        &mut self,
        id: ComponentId,
        ty: &str,
        method: &str,
        args: Vec<mms::Value>,
    ) -> Result<mms::HostResponse, mms::HostError> {
        use mms::{HostResponse as R, Value as V};
        let err = |s: &str| mms::HostError::failure(format!("{ty}.{method}"), s);
        let string = |v: &V| {
            if let V::String(s) = v {
                Ok(s.clone())
            } else {
                Err(err("expected string"))
            }
        };
        if ty == "UdpSocket" {
            if matches!(method, "ready" | "local_addr") {
                if !args.is_empty() {
                    return Err(err("socket getter takes no arguments"));
                }
                let socket = self
                    .world
                    .get_component_by_id_as::<UdpSocketComponent>(id)
                    .ok_or_else(|| err("stale socket"))?;
                return Ok(R::Value(if method == "ready" {
                    V::Bool(socket.ready)
                } else {
                    socket
                        .local_address
                        .clone()
                        .map(V::String)
                        .unwrap_or(V::Null)
                }));
            }
            if self
                .world
                .get_component_by_id_as::<UdpSocketComponent>(id)
                .is_none()
            {
                return Err(err("receiver is not a socket"));
            }
            if args.len() != 1 || !matches!(method, "publish" | "subscribe") {
                return Err(err("expected publish(name) or subscribe(name)"));
            }
            let name = string(&args[0])?;
            if name.is_empty() || name.len() > 96 {
                return Err(err("channel name must contain 1..=96 bytes"));
            }
            let publishing = method == "publish";
            let channel = self
                .world
                .add_component(UdpChannelComponent::new(id, publishing, name));
            let runtime_only = self
                .world
                .add_component(crate::engine::ecs::component::SerializeComponent::off());
            self.world
                .add_child(channel, runtime_only)
                .map_err(|e| err(&e))?;
            return Ok(R::Component {
                handle: Self::component_handle(channel),
                component_type: if publishing {
                    "UdpPublication"
                } else {
                    "UdpSubscription"
                }
                .into(),
            });
        }
        if method == "ready" {
            if !args.is_empty() {
                return Err(err("ready() takes no arguments"));
            }
            let channel = self
                .world
                .get_component_by_id_as::<UdpChannelComponent>(id)
                .ok_or_else(|| err("stale channel"))?;
            return Ok(R::Value(V::Bool(channel.ready)));
        }
        if method == "write" {
            let [value] = args.as_slice() else {
                return Err(err("write(value) takes one argument"));
            };
            let value = super::host::external_value_to_legacy(value.clone())?;
            let value = super::host::legacy_event_value_to_transport(value)?;
            let channel = self
                .world
                .get_component_by_id_as_mut::<UdpChannelComponent>(id)
                .ok_or_else(|| err("stale channel"))?;
            if !channel.publishing {
                return Err(err("only publications accept writes"));
            }
            if !channel.ready {
                return Err(err("channel is not ready"));
            }
            let schema = channel
                .schema
                .as_ref()
                .ok_or_else(|| err("missing channel type"))?;
            schema
                .encode_value(&value, &mut [0; 1200])
                .map_err(|e| err(&e.to_string()))?;
            channel.pending = Some(value);
            return Ok(R::Value(V::Null));
        }
        let channel = self
            .world
            .get_component_by_id_as_mut::<UdpChannelComponent>(id)
            .ok_or_else(|| err("stale channel"))?;
        if channel.active {
            return Err(err("configure channels before attachment/activation"));
        }
        match method {
            "type" => {
                let [value] = args.as_slice() else {
                    return Err(err("type expects one type tag/record"));
                };
                channel.schema = Some(match value {
                    V::TypeTag(tag) => Schema::primitive(*tag),
                    V::RecordType(record) => {
                        Schema::record(record.clone()).map_err(|e| err(&e.to_string()))?
                    }
                    _ => return Err(err("type expects an imported tag or record type")),
                });
            }
            "from" | "to" => {
                if (method == "to") != channel.publishing {
                    return Err(err("use to for publication, from for subscription"));
                }
                let [value] = args.as_slice() else {
                    return Err(err("expected one numeric address"));
                };
                channel.peer = Some(
                    string(value)?
                        .parse()
                        .map_err(|_| err("expected numeric IP:port"))?,
                );
            }
            "bind_transform" | "bind_morph" => {
                if channel.publishing {
                    return Err(err("receive bindings require a subscription"));
                }
                let Some(V::ComponentObject { id: target, .. }) = args.first() else {
                    return Err(err("expected component target"));
                };
                let target = Self::component_id(*target);
                let mut space = TransformSpace::World;
                let mut label = None;
                let options_index = if method == "bind_morph" { 2 } else { 1 };
                if args.len() > options_index + 1 {
                    return Err(err("too many binding arguments"));
                }
                if let Some(options) = args.get(options_index) {
                    let options = match options {
                        V::Map(map) => map.clone(),
                        V::Object(object) => object
                            .with_map(Clone::clone)
                            .ok_or_else(|| err("stale options table"))?,
                        _ => return Err(err("binding options must be a table")),
                    };
                    for (key, value) in options {
                        match key.as_str() {
                            "name" => label = Some(string(&value)?),
                            "space" if method == "bind_transform" => {
                                space = match string(&value)?.as_str() {
                                    "world" => TransformSpace::World,
                                    "local" => TransformSpace::Local,
                                    _ => return Err(err("space must be world or local")),
                                }
                            }
                            _ => return Err(err("unknown binding option")),
                        }
                    }
                }
                channel.binding = Some(if method == "bind_transform" {
                    UdpBinding::Transform { target, space }
                } else {
                    let Some(slot) = args.get(1) else {
                        return Err(err("bind_morph expects target and semantic slot"));
                    };
                    UdpBinding::Morph {
                        target,
                        slot: string(slot)?,
                    }
                });
                if let Some(label) = label {
                    if let Some(node) = self.world.get_component_record_mut(id) {
                        node.name = label;
                    }
                }
            }
            _ => return Err(err("unknown channel method")),
        }
        Ok(R::Component {
            handle: Self::component_handle(id),
            component_type: ty.into(),
        })
    }
}

pub(super) fn configure(
    component: &mut mms::ComponentBuilder<'_, super::runtime_config::MittensBinding>,
    canonical: &'static str,
) {
    use super::runtime_config::{component_signature, host_method};
    use mms::{ValueSignature as S, ValueType as T};
    if canonical == "UdpSocket" {
        host_method(component, canonical, "ready", S::new(vec![], T::Bool));
        host_method(component, canonical, "local_addr", S::new(vec![], T::Any));
        component.constructor("bind", component_signature([T::String]));
        for name in ["publish", "subscribe"] {
            host_method(
                component,
                canonical,
                name,
                S::new(vec![T::String], T::Component),
            );
        }
    } else {
        for name in ["type", "from", "to"] {
            host_method(
                component,
                canonical,
                name,
                S::new(vec![T::Any], T::Component),
            );
        }
        host_method(component, canonical, "ready", S::new(vec![], T::Bool));
        if canonical == "UdpPublication" {
            host_method(component, canonical, "write", S::new(vec![T::Any], T::Null));
        } else {
            host_method(
                component,
                canonical,
                "bind_transform",
                S::with_optional(vec![T::Component, T::Any], 1, T::Component),
            );
            host_method(
                component,
                canonical,
                "bind_morph",
                S::with_optional(vec![T::Component, T::String, T::Any], 2, T::Component),
            );
        }
    }
}

pub(crate) fn sample_value(value: &mms::TransportValue) -> super::object::Value {
    use super::object::Value as V;
    use mms::TransportValue as T;
    match value {
        T::Null => V::Null,
        T::Bool(v) => V::Bool(*v),
        T::Number(v) => V::Number(*v),
        T::String(v) => V::String(v.clone()),
        T::Array(v) => V::Array(v.iter().map(sample_value).collect()),
        T::Table(v) => V::Map(
            v.iter()
                .map(|(k, v)| (k.clone(), sample_value(v)))
                .collect(),
        ),
        _ => V::Null,
    }
}
