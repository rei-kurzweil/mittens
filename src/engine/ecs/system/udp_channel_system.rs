//! Scene adapter: owns socket I/O and explicit receive bindings; no pose replication policy.
use crate::engine::ecs::component::{
    GLTFComponent, MorphTargetMapComponent, UdpBinding, UdpChannelComponent, UdpSocketComponent,
};
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, SignalEmitter, World};
use crate::engine::transform::TransformTrs;
use crate::engine::udp_channels::{
    ChannelId, ChannelSocket, PublicationId, Schema, SubscriptionId,
};
use std::{
    collections::{BTreeSet, HashMap},
    time::Instant,
};

pub fn transform_schema() -> Schema {
    use meow_meow_script::{RecordType, TypeTag};
    Schema::record(RecordType(std::collections::BTreeMap::from([
        ("translation".into(), TypeTag::Vec3),
        ("rotation_quat_xyzw".into(), TypeTag::Vec4),
        ("scale".into(), TypeTag::Vec3),
    ])))
    .unwrap()
}
enum Channel {
    Publication {
        socket: ComponentId,
        id: PublicationId,
    },
    Subscription {
        socket: ComponentId,
        id: SubscriptionId,
        last: Option<(ChannelId, u64)>,
        binding: Option<UdpBinding>,
        morph_keys: Vec<crate::engine::ecs::component::MorphTargetKey>,
    },
}
#[derive(Default)]
pub struct UdpChannelSystem {
    registered: BTreeSet<ComponentId>,
    scratch: Vec<ComponentId>,
    sockets: HashMap<ComponentId, ChannelSocket>,
    channels: HashMap<ComponentId, Channel>,
}
impl std::fmt::Debug for UdpChannelSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UdpChannelSystem")
            .field("registered", &self.registered.len())
            .field("sockets", &self.sockets.len())
            .field("channels", &self.channels.len())
            .finish()
    }
}
impl UdpChannelSystem {
    pub fn register(&mut self, id: ComponentId) {
        self.registered.insert(id);
    }
    pub fn remove(&mut self, world: &mut World, id: ComponentId) {
        if let Some(UdpBinding::Morph { target, slot }) = world
            .get_component_by_id_as::<UdpChannelComponent>(id)
            .and_then(|c| c.binding.clone())
        {
            clear_morph(world, target, &slot);
        }
        self.registered.remove(&id);
        if self.sockets.remove(&id).is_some() {
            let dependents: Vec<_> = self
                .channels
                .iter()
                .filter_map(|(component, channel)| {
                    let socket = match channel {
                        Channel::Publication { socket, .. }
                        | Channel::Subscription { socket, .. } => socket,
                    };
                    (*socket == id).then_some(*component)
                })
                .collect();
            for dependent in dependents {
                self.remove(world, dependent);
            }
        }
        if let Some(c) = world.get_component_by_id_as_mut::<UdpChannelComponent>(id) {
            c.active = false;
            c.ready = false;
            c.pending = None;
        }
        if let Some(channel) = self.channels.remove(&id) {
            match channel {
                Channel::Publication { socket, id } => {
                    if let Some(s) = self.sockets.get_mut(&socket) {
                        let _ = s.remove_publication(id);
                    }
                }
                Channel::Subscription { socket, id, .. } => {
                    if let Some(s) = self.sockets.get_mut(&socket) {
                        let _ = s.remove_subscription(id);
                    }
                }
            }
        }
    }
    pub fn tick(&mut self, world: &mut World, emit: &mut dyn SignalEmitter) {
        // Registration is a bounded per-component operation, not a world scan.
        let mut ids = std::mem::take(&mut self.scratch);
        ids.clear();
        ids.extend(self.registered.iter().copied());
        for id in &ids {
            if world.get_component_record(*id).is_none() {
                self.remove(world, *id);
                continue;
            }
            if self.sockets.contains_key(id) || self.channels.contains_key(id) {
                continue;
            }
            let result = if let Some(config) =
                world.get_component_by_id_as::<UdpSocketComponent>(*id)
            {
                config
                    .address
                    .parse()
                    .map_err(|e| format!("{e}"))
                    .and_then(|addr| ChannelSocket::bind(addr).map_err(|e| e.to_string()))
                    .map(|socket| {
                        self.sockets.insert(*id, socket);
                    })
            } else if let Some(config) = world.get_component_by_id_as::<UdpChannelComponent>(*id) {
                let Some(socket) = self.sockets.get_mut(&config.socket) else {
                    let message = format!("socket {:?} is not active", config.socket);
                    state(world, emit, *id, false, Some(message));
                    continue;
                };
                let result = (|| {
                    let schema = config.schema.as_ref().ok_or("channel has no type")?.clone();
                    let peer = config.peer.ok_or("channel has no peer address")?;
                    if let Some(binding) = &config.binding {
                        let expected = match binding {
                            UdpBinding::Transform { .. } => transform_schema(),
                            UdpBinding::Morph { .. } => {
                                Schema::primitive(meow_meow_script::TypeTag::F32)
                            }
                        };
                        if schema != expected {
                            return Err("binding payload type does not match target".to_string());
                        }
                        if self
                            .channels
                            .keys()
                            .filter_map(|id| {
                                world.get_component_by_id_as::<UdpChannelComponent>(*id)
                            })
                            .any(|c| c.binding.as_ref().is_some_and(|b| same_target(b, binding)))
                        {
                            return Err("target already has a channel writer".into());
                        }
                    }
                    if config.publishing {
                        socket
                            .publish(peer, &config.channel_name, schema)
                            .map(|native| Channel::Publication {
                                socket: config.socket,
                                id: native,
                            })
                            .map_err(|e| e.to_string())
                    } else {
                        socket
                            .subscribe(peer, &config.channel_name, schema)
                            .map(|native| Channel::Subscription {
                                socket: config.socket,
                                id: native,
                                last: None,
                                binding: config.binding.clone(),
                                morph_keys: Vec::new(),
                            })
                            .map_err(|e| e.to_string())
                    }
                })();
                result.map(|channel| {
                    self.channels.insert(*id, channel);
                    if let Some(c) = world.get_component_by_id_as_mut::<UdpChannelComponent>(*id) {
                        c.active = true;
                    }
                })
            } else {
                continue;
            };
            if let Err(message) = result {
                state(world, emit, *id, false, Some(message));
            } else if let Some(socket) = self.sockets.get(id) {
                let address = socket.local_addr().ok().map(|a| a.to_string());
                if let Some(c) = world.get_component_by_id_as_mut::<UdpSocketComponent>(*id) {
                    c.local_address = address;
                }
                state(world, emit, *id, true, None);
            }
        }
        self.scratch = ids;
        for (socket_component, socket) in &mut self.sockets {
            match socket.poll(Instant::now()) {
                Ok(()) => state(world, emit, *socket_component, true, None),
                Err(e) => state(world, emit, *socket_component, false, Some(e.to_string())),
            }
            // State/sample delivery below queries retained native state, avoiding an extra event Vec.
            for event in socket.drain_events() {
                let rejection = match event {
                    crate::engine::udp_channels::ChannelEvent::SchemaMismatch(native) => {
                        self.channels.iter().find_map(|(component, c)| match c {
                            Channel::Subscription { socket, id, .. }
                                if socket == socket_component && *id == native =>
                            {
                                Some(*component)
                            }
                            _ => None,
                        })
                    }
                    _ => None,
                };
                if let Some(component) = rejection {
                    state(
                        world,
                        emit,
                        component,
                        false,
                        Some("publisher schema does not match subscription".into()),
                    );
                }
            }
        }
        for (component, channel) in &mut self.channels {
            let (socket_id, publishing) = match channel {
                Channel::Publication { socket, .. } => (*socket, true),
                Channel::Subscription { socket, .. } => (*socket, false),
            };
            let Some(socket) = self.sockets.get_mut(&socket_id) else {
                state(
                    world,
                    emit,
                    *component,
                    false,
                    Some("socket removed".into()),
                );
                continue;
            };
            let ready = match channel {
                Channel::Publication { id, .. } => socket.is_publication_ready(*id),
                Channel::Subscription { id, .. } => socket.channel(*id).is_some(),
            };
            let retained_error = (!publishing)
                .then(|| {
                    world
                        .get_component_by_id_as::<UdpChannelComponent>(*component)
                        .and_then(|c| c.error.clone())
                })
                .flatten();
            state(
                world,
                emit,
                *component,
                ready && retained_error.is_none(),
                retained_error,
            );
            if !ready {
                continue;
            }
            if publishing {
                if let Channel::Publication { id, .. } = channel {
                    let value = world
                        .get_component_by_id_as_mut::<UdpChannelComponent>(*component)
                        .and_then(|c| c.pending.take());
                    if let Some(value) = value {
                        if let Err(e) = socket.write(*id, &value) {
                            state(world, emit, *component, false, Some(e.to_string()));
                        }
                    }
                }
            } else if let Channel::Subscription {
                id,
                last,
                binding,
                morph_keys,
                ..
            } = channel
            {
                if let Some(sample) = socket.sample(*id) {
                    let identity = (sample.channel, sample.sequence);
                    if *last == Some(identity) {
                        continue;
                    }
                    let result = match binding.as_ref() {
                        Some(UdpBinding::Transform { target, space }) => {
                            let mut values = [0.; 10];
                            sample.schema.decode_numeric(sample.bytes, &mut values).map_err(|e| e.to_string()).and_then(|_| {
                                let norm = values[..4].iter().map(|v| v*v).sum::<f32>();
                                if !norm.is_finite() { return Err("quaternion norm outside finite f32 range".into()); }
                                let pose = TransformTrs::new(values[7..10].try_into().unwrap(), values[..4].try_into().unwrap(), values[4..7].try_into().unwrap()).normalized().map_err(|e| e.to_string())?;
                                if world.get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(*target).is_none() { return Err("transform target removed".into()); }
                                emit.push_intent_now(*target, IntentValue::SetTransformTrs { component_id: *target, space: *space, value: pose }); Ok(())
                            })
                        }
                        Some(UdpBinding::Morph { target, slot }) => {
                            let mut value = [0.];
                            sample
                                .schema
                                .decode_numeric(sample.bytes, &mut value)
                                .map_err(|e| e.to_string())
                                .and_then(|_| {
                                    if !(0.0..=1.0).contains(&value[0]) {
                                        return Err("morph weight outside [0,1]".into());
                                    }
                                    if morph_keys.is_empty() {
                                        *morph_keys = resolve_morph(world, *target, slot)?;
                                    }
                                    let model = world
                                        .get_component_by_id_as_mut::<GLTFComponent>(*target)
                                        .ok_or("morph target removed")?;
                                    for key in morph_keys.iter() {
                                        model
                                            .morph_factors
                                            .get_mut(key)
                                            .ok_or("morph target changed")?
                                            .external = Some(value[0]);
                                    }
                                    Ok(())
                                })
                        }
                        None => sample
                            .schema
                            .decode_value(sample.bytes)
                            .map_err(|e| e.to_string())
                            .map(|value| {
                                emit.push_event(*component, EventSignal::UdpSample { value })
                            }),
                    };
                    match result {
                        Ok(()) => {
                            *last = Some(identity);
                            state(world, emit, *component, true, None);
                        }
                        Err(message) => state(world, emit, *component, false, Some(message)),
                    }
                }
            }
        }
    }
}
fn state(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    id: ComponentId,
    ready: bool,
    error: Option<String>,
) {
    let changed = if let Some(c) = world.get_component_by_id_as_mut::<UdpChannelComponent>(id) {
        let changed = c.ready != ready || c.error != error;
        c.ready = ready;
        c.error = error.clone();
        changed
    } else if let Some(c) = world.get_component_by_id_as_mut::<UdpSocketComponent>(id) {
        let changed = c.ready != ready || c.error != error;
        c.ready = ready;
        c.error = error.clone();
        changed
    } else {
        false
    };
    if changed {
        emit.push_event(
            id,
            EventSignal::UdpState {
                ready,
                message: error.unwrap_or_default(),
            },
        );
    }
}

fn resolve_morph(
    world: &World,
    gltf: ComponentId,
    slot: &str,
) -> Result<Vec<crate::engine::ecs::component::MorphTargetKey>, String> {
    let label = world
        .children_of(gltf)
        .iter()
        .find_map(|id| {
            world
                .get_component_by_id_as::<MorphTargetMapComponent>(*id)
                .and_then(|map| map.slots().get(slot))
        })
        .cloned()
        .ok_or("morph semantic slot is not mapped")?;
    let model = world
        .get_component_by_id_as::<GLTFComponent>(gltf)
        .ok_or("morph target is not a GLTF instance")?;
    if !model.spawned {
        return Err("morph target is not initialized".into());
    }
    let keys: Vec<_> = model
        .morph_targets
        .iter()
        .filter(|info| info.label.as_ref() == Some(&label))
        .map(|info| info.key)
        .collect();
    if keys.is_empty() {
        Err("mapped morph target was not imported".into())
    } else {
        Ok(keys)
    }
}

fn clear_morph(world: &mut World, gltf: ComponentId, slot: &str) {
    let label = world
        .children_of(gltf)
        .iter()
        .find_map(|id| {
            world
                .get_component_by_id_as::<MorphTargetMapComponent>(*id)
                .and_then(|map| map.slots().get(slot))
        })
        .cloned();
    if let Some(model) = world.get_component_by_id_as_mut::<GLTFComponent>(gltf) {
        for info in &model.morph_targets {
            if label.is_some() && info.label == label {
                if let Some(factor) = model.morph_factors.get_mut(&info.key) {
                    factor.external = None;
                }
            }
        }
    }
}

fn same_target(a: &UdpBinding, b: &UdpBinding) -> bool {
    match (a, b) {
        (UdpBinding::Transform { target: a, .. }, UdpBinding::Transform { target: b, .. }) => {
            a == b
        }
        (
            UdpBinding::Morph {
                target: a,
                slot: sa,
            },
            UdpBinding::Morph {
                target: b,
                slot: sb,
            },
        ) => a == b && sa == sb,
        _ => false,
    }
}
