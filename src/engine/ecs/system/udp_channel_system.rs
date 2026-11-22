//! Typed-channel scene adapter. Raw I/O belongs to UdpSystem; no pose replication policy.
use super::TransformSystem;
use crate::engine::ecs::component::{
    GLTFComponent, MorphTargetMapComponent, TransformComponent, TransformOperatorComponent,
    UdpBinding, UdpChannelComponent, UdpSocketComponent,
};
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, SignalEmitter, World};
use crate::engine::transform::{TransformSpace, TransformTrs};
use crate::engine::udp_channels::{
    ChannelId, ChannelSocket, PublicationId, Sample, Schema, SubscriptionId,
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
        last_transform: Option<TransformTrs>,
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
    standalone_udp: crate::engine::udp::UdpSystem,
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
            c.final_transform = None;
        }
        if let Some(channel) = self.channels.remove(&id) {
            match channel {
                Channel::Publication { socket, id, .. } => {
                    if let Some(s) = self.sockets.get_mut(&socket) {
                        let _ = s.remove_publication(id);
                    }
                }
                Channel::Subscription {
                    socket,
                    id,
                    binding,
                    ..
                } => {
                    if let Some(UdpBinding::Morph { target, slot }) = binding {
                        clear_morph(world, target, &slot);
                    }
                    if let Some(s) = self.sockets.get_mut(&socket) {
                        let _ = s.remove_subscription(id);
                    }
                }
            }
        }
    }
    pub fn tick(&mut self, world: &mut World, emit: &mut dyn SignalEmitter) {
        let udp = self.standalone_udp.clone();
        self.tick_with_udp(world, emit, &udp);
    }
    pub fn tick_with_udp(
        &mut self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        udp: &crate::engine::udp::UdpSystem,
    ) {
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
                    .and_then(|addr| ChannelSocket::bind_on(udp, addr).map_err(|e| e.to_string()))
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
                            UdpBinding::Transform { target, space } => {
                                if world
                                    .get_component_by_id_as::<TransformComponent>(*target)
                                    .is_none()
                                    && (*space != TransformSpace::Local
                                        || world
                                            .get_component_by_id_as::<TransformOperatorComponent>(
                                                *target,
                                            )
                                            .and_then(TransformOperatorComponent::hinge_settings)
                                            .is_none())
                                {
                                    return Err(
                                        "expected a transform or a local TOP.hinge target".into()
                                    );
                                }
                                transform_schema()
                            }
                            UdpBinding::Morph { .. } => {
                                Schema::primitive(meow_meow_script::TypeTag::F32)
                            }
                        };
                        if schema != expected {
                            return Err("binding payload type does not match target".to_string());
                        }
                        if !config.publishing
                            && self
                                .channels
                                .keys()
                                .filter_map(|id| {
                                    world.get_component_by_id_as::<UdpChannelComponent>(*id)
                                })
                                .filter(|c| !c.publishing)
                                .any(|c| {
                                    c.binding.as_ref().is_some_and(|b| same_target(b, binding))
                                })
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
                                last_transform: None,
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
                        c.error = None;
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
            let socket_id = match channel {
                Channel::Publication { socket, .. } | Channel::Subscription { socket, .. } => {
                    *socket
                }
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
            if let Channel::Publication {
                id, last_transform, ..
            } = channel
            {
                if !socket.is_publication_ready(*id) {
                    *last_transform = None;
                    state(world, emit, *component, false, None);
                    continue;
                }
                let result = publish(world, socket, *component, *id, last_transform);
                state(world, emit, *component, result.is_ok(), result.err());
            } else if let Channel::Subscription {
                id,
                last,
                binding,
                morph_keys,
                ..
            } = channel
            {
                let ready = socket.channel(*id).is_some();
                let retained_error = world
                    .get_component_by_id_as::<UdpChannelComponent>(*component)
                    .and_then(|c| c.error.clone());
                state(
                    world,
                    emit,
                    *component,
                    ready && retained_error.is_none(),
                    retained_error,
                );
                if let Some(sample) = socket.sample(*id) {
                    let identity = (sample.channel, sample.sequence);
                    if *last == Some(identity) {
                        continue;
                    }
                    if binding.is_some()
                        && world
                            .get_component_by_id_as::<UdpChannelComponent>(*component)
                            .is_some_and(|c| !c.binding_enabled)
                    {
                        *last = Some(identity);
                        continue;
                    }
                    let result = match binding.as_ref() {
                        Some(UdpBinding::Transform { target, space }) => {
                            apply_transform_sample(world, emit, *target, *space, sample)
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

pub(crate) fn read_bound_transform(
    world: &World,
    target: ComponentId,
    space: TransformSpace,
) -> Result<TransformTrs, String> {
    match space {
        TransformSpace::World => {
            TransformSystem::world_trs(world, target).map_err(|e| e.to_string())
        }
        TransformSpace::Local => {
            if let Some(transform) = world.get_component_by_id_as::<TransformComponent>(target) {
                return Ok(transform.trs());
            }
            world
                .get_component_by_id_as::<TransformOperatorComponent>(target)
                .and_then(TransformOperatorComponent::hinge_settings)
                .map(|hinge| hinge.local_trs())
                .ok_or_else(|| "transform target removed".into())
        }
    }
}

fn publish(
    world: &mut World,
    socket: &mut ChannelSocket,
    component: ComponentId,
    id: PublicationId,
    last_transform: &mut Option<TransformTrs>,
) -> Result<(), String> {
    let config = world
        .get_component_by_id_as_mut::<UdpChannelComponent>(component)
        .ok_or("publication removed")?;
    let Some(UdpBinding::Transform { target, space }) = config.binding else {
        if let Some(value) = config.pending.take() {
            socket.write(id, &value).map_err(|e| e.to_string())?;
        }
        return Ok(());
    };
    let enabled = config.binding_enabled;
    let pose = if enabled {
        let pose = read_bound_transform(world, target, space)?;
        if *last_transform == Some(pose) {
            return Ok(());
        }
        pose
    } else {
        *last_transform = None;
        let Some(pose) = config.final_transform else {
            return Ok(());
        };
        pose
    };
    let [x, y, z, w] = pose.rotation_quat_xyzw;
    let [sx, sy, sz] = pose.scale;
    let [tx, ty, tz] = pose.translation;
    // The record schema orders fields by name.
    let sent = socket
        .write_numeric(id, &[x, y, z, w, sx, sy, sz, tx, ty, tz])
        .map_err(|e| e.to_string())?;
    if sent {
        if enabled {
            *last_transform = Some(pose);
        } else if let Some(config) =
            world.get_component_by_id_as_mut::<UdpChannelComponent>(component)
        {
            config.final_transform = None;
        }
    }
    Ok(())
}

fn apply_transform_sample(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    target: ComponentId,
    space: TransformSpace,
    sample: Sample<'_>,
) -> Result<(), String> {
    let mut values = [0.; 10];
    sample
        .schema
        .decode_numeric(sample.bytes, &mut values)
        .map_err(|e| e.to_string())?;
    let [x, y, z, w, sx, sy, sz, tx, ty, tz] = values;
    let rotation = [x, y, z, w];
    let norm = rotation.iter().map(|v| v * v).sum::<f32>();
    if !norm.is_finite() {
        return Err("quaternion norm outside finite f32 range".into());
    }
    let value = TransformTrs::new([tx, ty, tz], rotation, [sx, sy, sz])
        .normalized()
        .map_err(|e| e.to_string())?;
    if world
        .get_component_by_id_as::<TransformComponent>(target)
        .is_some()
    {
        emit.push_intent_now(
            target,
            IntentValue::SetTransformTrs {
                component_id: target,
                space,
                value,
            },
        );
        return Ok(());
    }
    if space == TransformSpace::Local {
        if let Some(hinge) = world
            .get_component_by_id_as_mut::<TransformOperatorComponent>(target)
            .and_then(TransformOperatorComponent::hinge_settings_mut)
        {
            if hinge.set_local_rotation(value.rotation_quat_xyzw)? {
                emit.push_intent_now(
                    target,
                    IntentValue::UpdateTransformWorld {
                        component_id: target,
                    },
                );
            }
            return Ok(());
        }
    }
    Err("transform target removed".into())
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
