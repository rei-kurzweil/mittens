use crate::engine::udp::{UdpEndpoint, UdpSystem};
use crate::engine::ecs::component::xr_eye_tracking::{EyeClosureSample, EyeGazeSample};
use crate::engine::ecs::component::{
    EyeTrackingSource, HTCEyeTrackingComponent, MediaPipeEyeTrackingComponent,
    VRChatOSCEyeTrackingComponent, XREyeTrackingComponent,
};
use crate::engine::ecs::{ComponentId, EventSignal, SignalEmitter, World};
use crate::utils::math::quat_rotate_vec3;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default)]
pub struct XREyeTrackingSystem {
    standalone_udp: UdpSystem,
    selectors: HashSet<ComponentId>,
    vrchat_osc_sources: HashSet<ComponentId>,
    htc_sources: HashSet<ComponentId>,
    mediapipe_sources: HashSet<ComponentId>,
    active_udp: Option<UdpSystem>,
    sockets: HashMap<ComponentId, UdpEndpoint>,
    htc_sockets: HashMap<ComponentId, UdpEndpoint>,
    /// A failed bind is retried so a port becoming available recovers without
    /// restarting the scene, but each active component reports that failure
    /// only once.
    failed_standard_binds: HashSet<ComponentId>,
    failed_htc_binds: HashSet<ComponentId>,
    /// OSC sends gaze vectors and openness as independent packets. Keep the
    /// latest value for every field so one packet cannot erase the others
    /// before the script callback sees them.
    standard_samples: HashMap<ComponentId, StandardEyeSample>,
    receive_sequence: u64,
}

#[derive(Debug, Clone, Copy, Default)]
struct StandardEyeSample {
    combined_look: Option<Look>,
    left_look: Option<Look>,
    right_look: Option<Look>,
    combined_openness: Option<f32>,
}
impl XREyeTrackingSystem {
    pub fn register_component(&mut self, world: &World, id: ComponentId) {
        if self.selectors.contains(&id)
            || self.vrchat_osc_sources.contains(&id)
            || self.htc_sources.contains(&id)
            || self.mediapipe_sources.contains(&id)
        {
            return;
        }
        if world
            .get_component_by_id_as::<XREyeTrackingComponent>(id)
            .is_some()
        {
            self.selectors.insert(id);
        } else if world
            .get_component_by_id_as::<VRChatOSCEyeTrackingComponent>(id)
            .is_some()
        {
            self.vrchat_osc_sources.insert(id);
        } else if world
            .get_component_by_id_as::<HTCEyeTrackingComponent>(id)
            .is_some()
        {
            self.htc_sources.insert(id);
        } else if world
            .get_component_by_id_as::<MediaPipeEyeTrackingComponent>(id)
            .is_some()
        {
            self.mediapipe_sources.insert(id);
        }
    }

    pub fn component_removed(&mut self, id: ComponentId) {
        self.selectors.remove(&id);
        self.vrchat_osc_sources.remove(&id);
        self.htc_sources.remove(&id);
        self.mediapipe_sources.remove(&id);
        self.sockets.remove(&id);
        self.htc_sockets.remove(&id);
        self.standard_samples.remove(&id);
        self.failed_standard_binds.remove(&id);
        self.failed_htc_binds.remove(&id);
    }

    fn prune_stale_registrations(&mut self, world: &World) {
        let mut stale = Vec::new();
        stale.extend(self.selectors.iter().copied().filter(|id| {
            world
                .get_component_by_id_as::<XREyeTrackingComponent>(*id)
                .is_none()
        }));
        stale.extend(self.vrchat_osc_sources.iter().copied().filter(|id| {
            world
                .get_component_by_id_as::<VRChatOSCEyeTrackingComponent>(*id)
                .is_none()
        }));
        stale.extend(self.htc_sources.iter().copied().filter(|id| {
            world
                .get_component_by_id_as::<HTCEyeTrackingComponent>(*id)
                .is_none()
        }));
        stale.extend(self.mediapipe_sources.iter().copied().filter(|id| {
            world
                .get_component_by_id_as::<MediaPipeEyeTrackingComponent>(*id)
                .is_none()
        }));
        stale.sort_unstable();
        stale.dedup();
        for id in stale {
            self.component_removed(id);
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
        udp: &UdpSystem,
    ) {
        if self.active_udp.as_ref().is_none_or(|old| !old.same_transport(udp)) {
            // Release leases before rebinding the same configured ports.
            self.sockets.clear();
            self.htc_sockets.clear();
            self.failed_standard_binds.clear();
            self.failed_htc_binds.clear();
            self.standard_samples.clear();
            self.active_udp = Some(udp.clone());
        }
        self.prune_stale_registrations(world);
        self.ensure_generic_sources(world);
        self.tick_standard(world, emit, udp);
        self.tick_htc(world, emit, udp);
        self.resolve_generic_trackers(world);
    }

    fn ensure_generic_sources(&mut self, world: &mut World) {
        let selectors: Vec<_> = self.selectors.iter().copied().collect();
        for selector_id in selectors {
            let (priority, legacy_osc_endpoint) = {
                let selector = world
                    .get_component_by_id_as::<XREyeTrackingComponent>(selector_id)
                    .expect("selector id came from the same world scan");
                (
                    selector.priority.clone(),
                    selector.legacy_osc_endpoint.clone(),
                )
            };
            for source in priority {
                let already_authored =
                    world
                        .children_of(selector_id)
                        .iter()
                        .copied()
                        .any(|child| match source {
                            EyeTrackingSource::VrChatOsc => world
                                .get_component_by_id_as::<VRChatOSCEyeTrackingComponent>(child)
                                .is_some(),
                            EyeTrackingSource::Htc => world
                                .get_component_by_id_as::<HTCEyeTrackingComponent>(child)
                                .is_some(),
                            EyeTrackingSource::MediaPipe => world
                                .get_component_by_id_as::<MediaPipeEyeTrackingComponent>(child)
                                .is_some(),
                        });
                if already_authored {
                    continue;
                }
                let child = match source {
                    EyeTrackingSource::VrChatOsc => {
                        let component = legacy_osc_endpoint
                            .clone()
                            .map(|(host, port)| VRChatOSCEyeTrackingComponent::listen(host, port))
                            .unwrap_or_else(VRChatOSCEyeTrackingComponent::on);
                        world.add_component(component)
                    }
                    EyeTrackingSource::Htc => world.add_component(HTCEyeTrackingComponent::on()),
                    EyeTrackingSource::MediaPipe => {
                        world.add_component(MediaPipeEyeTrackingComponent::on())
                    }
                };
                world
                    .add_child(selector_id, child)
                    .expect("new source and selector must exist and be acyclic");
                self.register_component(world, child);
            }
        }
    }

    fn resolve_generic_trackers(&mut self, world: &mut World) {
        let selectors: Vec<_> = self.selectors.iter().copied().collect();
        for selector_id in selectors {
            let priority = world
                .get_component_by_id_as::<XREyeTrackingComponent>(selector_id)
                .expect("selector id came from the same world scan")
                .priority
                .clone();
            let children = world.children_of(selector_id).to_vec();
            let mut gaze = None;
            let mut closure = None;
            for source in priority {
                for child in children.iter().copied() {
                    let samples = match source {
                        EyeTrackingSource::VrChatOsc => world
                            .get_component_by_id_as::<VRChatOSCEyeTrackingComponent>(child)
                            .map(|tracker| (tracker.gaze_sample, tracker.closure_sample)),
                        EyeTrackingSource::Htc => world
                            .get_component_by_id_as::<HTCEyeTrackingComponent>(child)
                            .map(|tracker| (tracker.gaze_sample, tracker.closure_sample)),
                        EyeTrackingSource::MediaPipe => None,
                    };
                    let Some((source_gaze, source_closure)) = samples else {
                        continue;
                    };
                    if gaze.is_none()
                        && source_gaze.sequence > 0
                        && (source_gaze.left.is_some() || source_gaze.right.is_some())
                    {
                        gaze = Some((source, source_gaze));
                    }
                    if closure.is_none()
                        && source_closure.sequence > 0
                        && (source_closure.left.is_some() || source_closure.right.is_some())
                    {
                        closure = Some((source, source_closure));
                    }
                }
            }
            let selector = world
                .get_component_by_id_as_mut::<XREyeTrackingComponent>(selector_id)
                .expect("selector still exists");
            selector.gaze_source = gaze.map(|(source, _)| source);
            if let Some((_, sample)) = gaze {
                selector.gaze_sample = sample;
            }
            selector.closure_source = closure.map(|(source, _)| source);
            selector.closure_sample = closure
                .map(|(_, sample)| sample)
                .unwrap_or_else(EyeClosureSample::default);
        }
    }
    fn tick_standard(&mut self, world: &mut World, emit: &mut dyn SignalEmitter, udp: &UdpSystem) {
        let ids: Vec<_> = self.vrchat_osc_sources.iter().copied().collect();
        for id in ids {
            // Closure is a live driver value, unlike retained gaze. If the
            // tracker stops sending it, AVC must release its morph override
            // on the next frame instead of leaving an avatar permanently
            // blinking.
            if let Some(component) =
                world.get_component_by_id_as_mut::<VRChatOSCEyeTrackingComponent>(id)
            {
                component.closure_sample.left = None;
                component.closure_sample.right = None;
            }
            let c = world
                .get_component_by_id_as::<VRChatOSCEyeTrackingComponent>(id)
                .unwrap();
            if !self.sockets.contains_key(&id) {
                match udp.bind(format!("{}:{}", c.host, c.port), 1024, "eye-osc") {
                    Ok(s) => {
                        self.failed_standard_binds.remove(&id);
                        eprintln!("[VRChatOSCEyeTracking] listening on {}:{}", c.host, c.port);
                        self.sockets.insert(id, s);
                    }
                    Err(error) => {
                        if self.failed_standard_binds.insert(id) {
                            eprintln!(
                                "[VRChatOSCEyeTracking] cannot bind {}:{}: {error}",
                                c.host, c.port
                            );
                        }
                    }
                }
            }
            if let Some(s) = self.sockets.get(&id) {
                let socket = s.clone();
                let mut received_update = false;
                let mut received_gaze = false;
                let mut received_closure = false;
                let _ = udp.poll(&socket, 256, |_from, packet| {
                    match decode_osc(packet) {
                        Some((combined_look, left_look, right_look, combined_openness)) => {
                            received_gaze |= combined_look.is_some()
                                || left_look.is_some()
                                || right_look.is_some();
                            received_closure |= combined_openness.is_some();
                            let sample = self.standard_samples.entry(id).or_default();
                            if combined_look.is_some() {
                                sample.combined_look = combined_look;
                            }
                            if left_look.is_some() {
                                sample.left_look = left_look;
                            }
                            if right_look.is_some() {
                                sample.right_look = right_look;
                            }
                            if combined_openness.is_some() {
                                sample.combined_openness = combined_openness;
                            }
                            received_update = true;
                        }
                        None => {}
                    }
                });
                // A face tracker can submit many UDP packets between rendered
                // frames. Keep only the newest sample so script callbacks and
                // renderer mutations remain bounded to one per frame/source.
                if received_update {
                    let sample = self.standard_samples.get(&id).copied().unwrap_or_default();
                    if received_gaze {
                        let gaze_sample = standard_gaze_sample(sample, self.next_sequence())
                            .unwrap_or(EyeGazeSample {
                                sequence: self.receive_sequence,
                                ..EyeGazeSample::default()
                            });
                        if let Some(component) =
                            world.get_component_by_id_as_mut::<VRChatOSCEyeTrackingComponent>(id)
                        {
                            component.gaze_sample = gaze_sample;
                        }
                    }
                    if received_closure {
                        if let Some(closure_sample) =
                            standard_closure_sample(sample.combined_openness, self.next_sequence())
                        {
                            if let Some(component) = world
                                .get_component_by_id_as_mut::<VRChatOSCEyeTrackingComponent>(id)
                            {
                                component.closure_sample = closure_sample;
                            }
                        }
                    }
                    let event = EventSignal::XrEyeTrackingUpdated {
                        combined_look: sample.combined_look,
                        left_look: sample.left_look,
                        right_look: sample.right_look,
                        combined_openness: sample.combined_openness,
                    };
                    emit.push_event(id, event.clone());
                    if let Some(selector) = world.parent_of(id).filter(|parent| {
                        world
                            .get_component_by_id_as::<XREyeTrackingComponent>(*parent)
                            .is_some()
                    }) {
                        emit.push_event(selector, event);
                    }
                }
            }
        }
    }
    fn tick_htc(&mut self, world: &mut World, emit: &mut dyn SignalEmitter, udp: &UdpSystem) {
        let ids: Vec<_> = self.htc_sources.iter().copied().collect();
        for id in ids {
            if let Some(component) = world.get_component_by_id_as_mut::<HTCEyeTrackingComponent>(id)
            {
                component.closure_sample.left = None;
                component.closure_sample.right = None;
            }
            let c = world
                .get_component_by_id_as::<HTCEyeTrackingComponent>(id)
                .unwrap();
            if !self.htc_sockets.contains_key(&id) {
                match udp.bind(format!("{}:{}", c.host, c.port), 1024, "eye-htc") {
                    Ok(s) => {
                        self.failed_htc_binds.remove(&id);
                        eprintln!("[HTCEyeTracking] listening on {}:{}", c.host, c.port);
                        self.htc_sockets.insert(id, s);
                    }
                    Err(error) => {
                        if self.failed_htc_binds.insert(id) {
                            eprintln!(
                                "[HTCEyeTracking] cannot bind {}:{}: {error}",
                                c.host, c.port
                            );
                        }
                    }
                }
            }
            if let Some(s) = self.htc_sockets.get(&id) {
                let socket = s.clone();
                let _ = udp.poll(&socket, 256, |_from, packet| {
                    let Some((left, right)) = decode_htc(packet) else {
                        return;
                    };
                    let gaze_sample = htc_gaze_sample(&left, &right, self.next_sequence());
                    let closure_sequence = self.next_sequence();
                    let closure_sample = htc_closure_sample(&left, &right, closure_sequence);
                    if let Some(component) =
                        world.get_component_by_id_as_mut::<HTCEyeTrackingComponent>(id)
                    {
                        if let Some(gaze_sample) = gaze_sample {
                            component.gaze_sample = gaze_sample;
                        }
                        component.closure_sample = closure_sample.unwrap_or(EyeClosureSample {
                            sequence: closure_sequence,
                            ..EyeClosureSample::default()
                        });
                    }
                    let event = EventSignal::XrEyeTrackingHtcUpdated { left, right };
                    emit.push_event(id, event.clone());
                    if let Some(selector) = world.parent_of(id).filter(|parent| {
                        world
                            .get_component_by_id_as::<XREyeTrackingComponent>(*parent)
                            .is_some()
                    }) {
                        emit.push_event(selector, event);
                    }
                });
            }
        }
    }
    fn next_sequence(&mut self) -> u64 {
        self.receive_sequence = self.receive_sequence.wrapping_add(1);
        self.receive_sequence
    }
}
pub type Look = [f32; 3];

fn normalized_look(look: Look) -> Option<Look> {
    if !look.iter().all(|value| value.is_finite()) {
        return None;
    }
    let length_sq = look.iter().map(|value| value * value).sum::<f32>();
    (length_sq > 1e-12).then(|| {
        let inv = length_sq.sqrt().recip();
        [look[0] * inv, look[1] * inv, look[2] * inv]
    })
}

fn standard_gaze_sample(sample: StandardEyeSample, sequence: u64) -> Option<EyeGazeSample> {
    let left = sample
        .left_look
        .and_then(normalized_look)
        .or_else(|| sample.combined_look.and_then(normalized_look));
    let right = sample
        .right_look
        .and_then(normalized_look)
        .or_else(|| sample.combined_look.and_then(normalized_look));
    let (left, right) = complete_eye_pair(left, right);
    left.is_some().then_some(EyeGazeSample {
        left,
        right,
        sequence,
    })
}

fn htc_gaze_sample(left: &HtcEye, right: &HtcEye, sequence: u64) -> Option<EyeGazeSample> {
    let left = left.look.and_then(normalized_look);
    let right = right.look.and_then(normalized_look);
    let (left, right) = complete_eye_pair(left, right);
    left.is_some().then_some(EyeGazeSample {
        left,
        right,
        sequence,
    })
}

fn complete_eye_pair<T: Copy>(left: Option<T>, right: Option<T>) -> (Option<T>, Option<T>) {
    match (left, right) {
        (Some(left), Some(right)) => (Some(left), Some(right)),
        (Some(value), None) | (None, Some(value)) => (Some(value), Some(value)),
        (None, None) => (None, None),
    }
}

fn standard_closure_sample(closure: Option<f32>, sequence: u64) -> Option<EyeClosureSample> {
    let closure = closure
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.0, 1.0))?;
    Some(EyeClosureSample {
        left: Some(closure),
        right: Some(closure),
        sequence,
    })
}

fn htc_closure_sample(left: &HtcEye, right: &HtcEye, sequence: u64) -> Option<EyeClosureSample> {
    let to_closure = |openness: Option<f32>| {
        openness
            .filter(|value| value.is_finite())
            .map(|value| (1.0 - value).clamp(0.0, 1.0))
    };
    let (left, right) = complete_eye_pair(to_closure(left.openness), to_closure(right.openness));
    left.is_some().then_some(EyeClosureSample {
        left,
        right,
        sequence,
    })
}
/// Decodes ALVR's VRChat Eye OSC messages. Unknown OSC arguments/messages are ignored.
pub fn decode_osc(
    packet: &[u8],
) -> Option<(Option<Look>, Option<Look>, Option<Look>, Option<f32>)> {
    let mut i = 0;
    let addr = osc_str(packet, &mut i)?;
    let tags = osc_str(packet, &mut i)?;
    let scalar = |i: &mut usize| {
        let v = osc_f32(packet, i)?;
        v.is_finite().then_some(v)
    };
    match (addr, tags) {
        ("/avatar/parameters/EyesClosedAmount" | "/tracking/eye/EyesClosedAmount", ",f") => Some((
            None,
            None,
            None,
            // VRChat Eye OSC reports closure directly: 0 = open, 1 = closed.
            Some(scalar(&mut i)?.clamp(0.0, 1.0)),
        )),
        ("/tracking/eye/CenterVec", ",fff") => Some((
            Some([scalar(&mut i)?, scalar(&mut i)?, scalar(&mut i)?]),
            None,
            None,
            None,
        )),
        ("/tracking/eye/LeftEyeVec", ",fff") => Some((
            None,
            Some([scalar(&mut i)?, scalar(&mut i)?, scalar(&mut i)?]),
            None,
            None,
        )),
        ("/tracking/eye/RightEyeVec", ",fff") => Some((
            None,
            None,
            Some([scalar(&mut i)?, scalar(&mut i)?, scalar(&mut i)?]),
            None,
        )),
        ("/tracking/eye/LeftRightVec", ",ffffff") => Some((
            None,
            Some([scalar(&mut i)?, scalar(&mut i)?, scalar(&mut i)?]),
            Some([scalar(&mut i)?, scalar(&mut i)?, scalar(&mut i)?]),
            None,
        )),
        ("/tracking/eye/CenterPitchYaw", ",ff") => Some((
            Some(look_from_pitch_yaw(scalar(&mut i)?, scalar(&mut i)?)),
            None,
            None,
            None,
        )),
        ("/tracking/eye/LeftRightPitchYaw", ",ffff") => Some((
            None,
            Some(look_from_pitch_yaw(scalar(&mut i)?, scalar(&mut i)?)),
            Some(look_from_pitch_yaw(scalar(&mut i)?, scalar(&mut i)?)),
            None,
        )),
        _ => None,
    }
}
fn look_from_pitch_yaw(pitch_deg: f32, yaw_deg: f32) -> Look {
    let pitch = pitch_deg.to_radians();
    let yaw = yaw_deg.to_radians();
    [
        yaw.sin() * pitch.cos(),
        -pitch.sin(),
        -yaw.cos() * pitch.cos(),
    ]
}
fn osc_str<'a>(b: &'a [u8], i: &mut usize) -> Option<&'a str> {
    let end = b.get(*i..)?.iter().position(|&x| x == 0)? + *i;
    let s = std::str::from_utf8(&b[*i..end]).ok()?;
    *i = (end + 4) & !3;
    Some(s)
}
fn osc_f32(b: &[u8], i: &mut usize) -> Option<f32> {
    let x = f32::from_bits(u32::from_be_bytes(b.get(*i..*i + 4)?.try_into().ok()?));
    *i += 4;
    Some(x)
}
#[derive(Debug, Clone, PartialEq)]
pub struct HtcEye {
    pub look: Option<Look>,
    pub position: Option<[f32; 2]>,
    pub openness: Option<f32>,
    pub wide: Option<f32>,
    pub squeeze: Option<f32>,
    pub pupil_diameter: Option<f32>,
}
/// ALVR Mittens packet: 84 bytes, `M`, protocol v1, little-endian floats.
pub fn decode_htc(b: &[u8]) -> Option<(HtcEye, HtcEye)> {
    if b.len() != 84 || b[0] != b'M' || b[1] != 1 {
        return None;
    }
    let flags = b[2];
    let f = |offset: usize| {
        let value = f32::from_le_bytes(b.get(offset..offset + 4)?.try_into().ok()?);
        value.is_finite().then_some(value)
    };
    let quat_look = |offset: usize| {
        let q = [f(offset)?, f(offset + 4)?, f(offset + 8)?, f(offset + 12)?];
        let length_sq = q.iter().map(|value| value * value).sum::<f32>();
        if length_sq <= 1e-12 {
            return None;
        }
        let inv_length = length_sq.sqrt().recip();
        Some(quat_rotate_vec3(
            q.map(|value| value * inv_length),
            [0.0, 0.0, -1.0],
        ))
    };
    let eye = |right: bool| {
        let gaze_bit = if right { 1 } else { 0 };
        let geometry_bit = if right { 3 } else { 2 };
        let diameter_bit = if right { 5 } else { 4 };
        let position_bit = if right { 7 } else { 6 };
        let gaze_offset = if right { 20 } else { 4 };
        let geometry_offset = if right { 48 } else { 36 };
        let diameter_offset = if right { 64 } else { 60 };
        let position_offset = if right { 76 } else { 68 };

        Some(HtcEye {
            look: (flags & (1 << gaze_bit) != 0)
                .then(|| quat_look(gaze_offset))
                .flatten(),
            position: (flags & (1 << position_bit) != 0)
                .then(|| Some([f(position_offset)?, f(position_offset + 4)?]))
                .flatten(),
            openness: (flags & (1 << geometry_bit) != 0)
                .then(|| f(geometry_offset))
                .flatten(),
            wide: (flags & (1 << geometry_bit) != 0)
                .then(|| f(geometry_offset + 4))
                .flatten(),
            squeeze: (flags & (1 << geometry_bit) != 0)
                .then(|| f(geometry_offset + 8))
                .flatten(),
            pupil_diameter: (flags & (1 << diameter_bit) != 0)
                .then(|| f(diameter_offset))
                .flatten(),
        })
    };

    Some((eye(false)?, eye(true)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::CommandQueue;
    use crate::engine::ecs::system::SystemWorld;
    use crate::engine::graphics::{RenderAssets, VisualWorld};

    #[test]
    fn eye_tracking_rebinds_on_transport_change_and_keeps_clone_leases() {
        let mut world = World::default();
        let osc = world.add_component(VRChatOSCEyeTrackingComponent::listen("127.0.0.1", 0));
        let htc = world.add_component(HTCEyeTrackingComponent::listen("127.0.0.1", 0));
        let mut system = XREyeTrackingSystem::default();
        system.register_component(&world, osc);
        system.register_component(&world, htc);
        let mut queue = CommandQueue::new();
        system.tick(&mut world, &mut queue);
        let standalone = system.standalone_udp.clone();
        let shared = UdpSystem::default();
        let sender = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let mut addresses = standalone.diagnostics();
        world.get_component_by_id_as_mut::<VRChatOSCEyeTrackingComponent>(osc).unwrap().port = addresses.iter().find(|e| e.label == "eye-osc").unwrap().local_addr.port();
        world.get_component_by_id_as_mut::<HTCEyeTrackingComponent>(htc).unwrap().port = addresses.iter().find(|e| e.label == "eye-htc").unwrap().local_addr.port();
        let mut old = standalone.clone();
        let mut previous_count = 0;
        for (i, udp) in [standalone.clone(), shared.clone(), shared.clone(), standalone.clone()].iter().enumerate() {
            system.tick_with_udp(&mut world, &mut queue, udp);
            if !old.same_transport(udp) { assert!(old.diagnostics().is_empty()); }
            addresses = udp.diagnostics();
            assert_eq!(addresses.len(), 2);
            let before: u64 = addresses.iter().map(|e| e.stats.received_packets).sum();
            if old.same_transport(udp) { assert_eq!(before, previous_count); }
            let value = 0.2 + i as f32 * 0.1;
            let mut packet = Vec::new();
            for text in ["/tracking/eye/EyesClosedAmount", ",f"] {
                packet.extend_from_slice(text.as_bytes()); packet.push(0);
                while packet.len() % 4 != 0 { packet.push(0); }
            }
            packet.extend_from_slice(&value.to_be_bytes());
            sender.send_to(&packet, addresses.iter().find(|e| e.label == "eye-osc").unwrap().local_addr).unwrap();
            let mut packet = [0u8; 84]; packet[..3].copy_from_slice(&[b'M', 1, 12]);
            packet[36..40].copy_from_slice(&(1. - value).to_le_bytes());
            packet[48..52].copy_from_slice(&(1. - value).to_le_bytes());
            sender.send_to(&packet, addresses.iter().find(|e| e.label == "eye-htc").unwrap().local_addr).unwrap();
            system.tick_with_udp(&mut world, &mut queue, &udp.clone());
            assert_eq!(udp.diagnostics().iter().map(|e| e.stats.received_packets).sum::<u64>(), before + 2);
            assert!((world.get_component_by_id_as::<VRChatOSCEyeTrackingComponent>(osc).unwrap().closure_sample.left.unwrap() - value).abs() < 1e-6);
            assert!((world.get_component_by_id_as::<HTCEyeTrackingComponent>(htc).unwrap().closure_sample.left.unwrap() - value).abs() < 1e-6);
            previous_count = before + 2;
            old = udp.clone();
        }
    }

    #[test]
    fn generic_tracker_materializes_default_source_components() {
        let mut world = World::default();
        let selector = world.add_component(XREyeTrackingComponent::on());
        let mut system = XREyeTrackingSystem::default();
        system.register_component(&world, selector);

        system.ensure_generic_sources(&mut world);

        let children = world.children_of(selector);
        assert_eq!(children.len(), 3);
        assert!(children.iter().any(|child| {
            world
                .get_component_by_id_as::<VRChatOSCEyeTrackingComponent>(*child)
                .is_some()
        }));
        assert!(children.iter().any(|child| {
            world
                .get_component_by_id_as::<HTCEyeTrackingComponent>(*child)
                .is_some()
        }));
        assert!(children.iter().any(|child| {
            world
                .get_component_by_id_as::<MediaPipeEyeTrackingComponent>(*child)
                .is_some()
        }));
        assert_eq!(system.vrchat_osc_sources.len(), 1);
        assert_eq!(system.htc_sources.len(), 1);
        assert_eq!(system.mediapipe_sources.len(), 1);

        system.ensure_generic_sources(&mut world);
        assert_eq!(world.children_of(selector).len(), 3);
        assert_eq!(system.vrchat_osc_sources.len(), 1);
        assert_eq!(system.htc_sources.len(), 1);
        assert_eq!(system.mediapipe_sources.len(), 1);
    }

    #[test]
    fn registration_classifies_each_kind_and_is_idempotent() {
        let mut world = World::default();
        let selector = world.add_component(XREyeTrackingComponent::on());
        let osc = world.add_component(VRChatOSCEyeTrackingComponent::on());
        let htc = world.add_component(HTCEyeTrackingComponent::on());
        let mediapipe = world.add_component(MediaPipeEyeTrackingComponent::on());
        let mut system = XREyeTrackingSystem::default();

        for id in [selector, osc, htc, mediapipe] {
            system.register_component(&world, id);
            system.register_component(&world, id);
        }

        assert_eq!(system.selectors, HashSet::from([selector]));
        assert_eq!(system.vrchat_osc_sources, HashSet::from([osc]));
        assert_eq!(system.htc_sources, HashSet::from([htc]));
        assert_eq!(system.mediapipe_sources, HashSet::from([mediapipe]));
    }

    #[test]
    fn initialized_authored_tree_registers_and_subtree_removal_unregisters() {
        let mut world = World::default();
        let selector = world.add_component(XREyeTrackingComponent::on());
        let osc = world.add_component(VRChatOSCEyeTrackingComponent::on());
        let htc = world.add_component(HTCEyeTrackingComponent::on());
        let mediapipe = world.add_component(MediaPipeEyeTrackingComponent::on());
        world.add_child(selector, osc).unwrap();
        world.add_child(selector, htc).unwrap();
        world.add_child(selector, mediapipe).unwrap();

        let mut systems = SystemWorld::default();
        let mut visuals = VisualWorld::default();
        let mut render_assets = RenderAssets::new();
        let mut queue = CommandQueue::new();
        world.init_component_tree(selector, &mut queue);
        queue.flush(&mut world, &mut systems, &mut visuals, &mut render_assets);

        assert_eq!(systems.xr_eye_tracking.selectors, HashSet::from([selector]));
        assert_eq!(
            systems.xr_eye_tracking.vrchat_osc_sources,
            HashSet::from([osc])
        );
        assert_eq!(systems.xr_eye_tracking.htc_sources, HashSet::from([htc]));
        assert_eq!(
            systems.xr_eye_tracking.mediapipe_sources,
            HashSet::from([mediapipe])
        );

        systems.remove_subtree_immediate(&mut world, &mut visuals, selector);
        assert!(systems.xr_eye_tracking.selectors.is_empty());
        assert!(systems.xr_eye_tracking.vrchat_osc_sources.is_empty());
        assert!(systems.xr_eye_tracking.htc_sources.is_empty());
        assert!(systems.xr_eye_tracking.mediapipe_sources.is_empty());
    }

    #[test]
    fn removal_clears_registration_and_transport_state() {
        let mut world = World::default();
        let osc = world.add_component(VRChatOSCEyeTrackingComponent::on());
        let htc = world.add_component(HTCEyeTrackingComponent::on());
        let mut system = XREyeTrackingSystem::default();
        system.register_component(&world, osc);
        system.register_component(&world, htc);
        system
            .standard_samples
            .insert(osc, StandardEyeSample::default());
        system.failed_standard_binds.insert(osc);
        system.failed_htc_binds.insert(htc);

        system.component_removed(osc);
        system.component_removed(htc);
        system.component_removed(osc);

        assert!(!system.vrchat_osc_sources.contains(&osc));
        assert!(!system.htc_sources.contains(&htc));
        assert!(!system.sockets.contains_key(&osc));
        assert!(!system.htc_sockets.contains_key(&htc));
        assert!(!system.standard_samples.contains_key(&osc));
        assert!(!system.failed_standard_binds.contains(&osc));
        assert!(!system.failed_htc_binds.contains(&htc));
    }

    #[test]
    fn stale_registration_is_pruned_without_discovering_unregistered_components() {
        let mut world = World::default();
        let stale = world.add_component(VRChatOSCEyeTrackingComponent::on());
        let unregistered = world.add_component(HTCEyeTrackingComponent::on());
        let mut system = XREyeTrackingSystem::default();
        system.register_component(&world, stale);
        world.remove_component_subtree(stale).unwrap();

        system.prune_stale_registrations(&world);

        assert!(!system.vrchat_osc_sources.contains(&stale));
        assert!(!system.htc_sources.contains(&unregistered));
    }

    #[test]
    fn generic_tracker_uses_priority_independently_per_channel() {
        let mut world = World::default();
        let selector = world.add_component(
            XREyeTrackingComponent::on()
                .with_priority(vec![EyeTrackingSource::Htc, EyeTrackingSource::VrChatOsc]),
        );
        let osc = world.add_component(VRChatOSCEyeTrackingComponent::on());
        let htc = world.add_component(HTCEyeTrackingComponent::on());
        world.add_child(selector, osc).unwrap();
        world.add_child(selector, htc).unwrap();
        world
            .get_component_by_id_as_mut::<VRChatOSCEyeTrackingComponent>(osc)
            .unwrap()
            .closure_sample = EyeClosureSample {
            left: Some(0.4),
            right: Some(0.4),
            sequence: 3,
        };
        world
            .get_component_by_id_as_mut::<HTCEyeTrackingComponent>(htc)
            .unwrap()
            .gaze_sample = EyeGazeSample {
            left: Some([1.0, 0.0, 0.0]),
            right: Some([1.0, 0.0, 0.0]),
            sequence: 2,
        };

        let mut system = XREyeTrackingSystem::default();
        system.register_component(&world, selector);
        system.register_component(&world, osc);
        system.register_component(&world, htc);
        system.resolve_generic_trackers(&mut world);

        let selected = world
            .get_component_by_id_as::<XREyeTrackingComponent>(selector)
            .unwrap();
        assert_eq!(selected.gaze_source, Some(EyeTrackingSource::Htc));
        assert_eq!(selected.closure_source, Some(EyeTrackingSource::VrChatOsc));
        assert_eq!(selected.closure_sample.left, Some(0.4));
    }

    #[test]
    fn standard_sample_prefers_individual_and_falls_back_to_combined() {
        let sample = standard_gaze_sample(
            StandardEyeSample {
                combined_look: Some([0.0, 0.0, -2.0]),
                left_look: Some([3.0, 0.0, 0.0]),
                ..StandardEyeSample::default()
            },
            9,
        )
        .expect("usable gaze");
        assert_eq!(sample.sequence, 9);
        assert_eq!(sample.left, Some([1.0, 0.0, 0.0]));
        assert_eq!(sample.right, Some([0.0, 0.0, -1.0]));

        let invalid_individual = standard_gaze_sample(
            StandardEyeSample {
                combined_look: Some([0.0, 0.0, -1.0]),
                left_look: Some([0.0, 0.0, 0.0]),
                ..StandardEyeSample::default()
            },
            10,
        )
        .expect("combined fallback");
        assert_eq!(invalid_individual.left, Some([0.0, 0.0, -1.0]));

        let unilateral = standard_gaze_sample(
            StandardEyeSample {
                left_look: Some([0.0, 1.0, 0.0]),
                ..StandardEyeSample::default()
            },
            11,
        )
        .expect("unilateral fallback");
        assert_eq!(unilateral.left, Some([0.0, 1.0, 0.0]));
        assert_eq!(unilateral.right, unilateral.left);
    }

    #[test]
    fn closure_normalization_duplicates_combined_and_preserves_htc_per_eye() {
        assert_eq!(
            standard_closure_sample(Some(0.7), 4)
                .map(|sample| { (sample.left, sample.right, sample.sequence) }),
            Some((Some(0.7), Some(0.7), 4))
        );

        let left = HtcEye {
            look: None,
            position: None,
            openness: Some(0.25),
            wide: None,
            squeeze: None,
            pupil_diameter: None,
        };
        let right = HtcEye {
            openness: Some(0.8),
            ..left.clone()
        };
        let closure = htc_closure_sample(&left, &right, 5).expect("HTC closure");
        assert_eq!(closure.left, Some(0.75));
        assert!((closure.right.unwrap() - 0.2).abs() < 1e-6);
        assert_eq!(closure.sequence, 5);

        let unilateral = htc_closure_sample(
            &left,
            &HtcEye {
                openness: None,
                ..right
            },
            6,
        )
        .expect("unilateral HTC closure fallback");
        assert_eq!(unilateral.left, Some(0.75));
        assert_eq!(unilateral.right, unilateral.left);
    }

    #[test]
    fn invalid_or_zero_gaze_is_not_a_pose_sample() {
        assert_eq!(normalized_look([0.0, 0.0, 0.0]), None);
        assert_eq!(normalized_look([f32::NAN, 0.0, -1.0]), None);
        assert!(
            htc_gaze_sample(
                &HtcEye {
                    look: Some([0.0, 0.0, 0.0]),
                    position: None,
                    openness: None,
                    wide: None,
                    squeeze: None,
                    pupil_diameter: None
                },
                &HtcEye {
                    look: None,
                    position: None,
                    openness: None,
                    wide: None,
                    squeeze: None,
                    pupil_diameter: None
                },
                1,
            )
            .is_none()
        );
    }

    #[test]
    fn decodes_alvr_mittens_v1_packet_layout() {
        let mut packet = [0u8; 84];
        packet[0] = b'M';
        packet[1] = 1;
        packet[2] = 0xff;
        let mut put = |offset: usize, values: &[f32]| {
            for (index, value) in values.iter().enumerate() {
                packet[offset + index * 4..offset + index * 4 + 4]
                    .copy_from_slice(&value.to_le_bytes());
            }
        };
        put(4, &[0.0, 0.0, 0.0, 1.0]);
        put(20, &[0.0, 1.0, 0.0, 0.0]);
        put(36, &[0.25, 0.5, 0.75]);
        put(48, &[0.8, 0.6, 0.4]);
        put(60, &[3.1, 3.2]);
        put(68, &[0.1, 0.2, 0.3, 0.4]);

        let (left, right) = decode_htc(&packet).expect("ALVR packet");
        assert_eq!(left.look, Some([0.0, 0.0, -1.0]));
        let right_look = right.look.expect("right gaze");
        assert!(right_look[0].abs() < 1e-6);
        assert!(right_look[1].abs() < 1e-6);
        assert!((right_look[2] - 1.0).abs() < 1e-6);
        assert_eq!(left.openness, Some(0.25));
        assert_eq!(left.wide, Some(0.5));
        assert_eq!(left.squeeze, Some(0.75));
        assert_eq!(right.openness, Some(0.8));
        assert_eq!(left.pupil_diameter, Some(3.1));
        assert_eq!(right.pupil_diameter, Some(3.2));
        assert_eq!(left.position, Some([0.1, 0.2]));
        assert_eq!(right.position, Some([0.3, 0.4]));

        let gaze = htc_gaze_sample(&left, &right, 12).expect("per-eye HTC gaze");
        assert_ne!(gaze.left, gaze.right);
        let closure = htc_closure_sample(&left, &right, 13).expect("per-eye HTC closure");
        assert_eq!(closure.left, Some(0.75));
        assert!((closure.right.unwrap() - 0.2).abs() < 1e-6);
    }
}
