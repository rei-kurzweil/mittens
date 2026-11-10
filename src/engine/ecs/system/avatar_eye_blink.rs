//! Normalized eyelid closure and opt-in provider-independent idle blinking.
use super::*;
#[cfg(test)]
#[path = "avatar_eye_blink_tests.rs"]
mod tests;
/// The newest normalized closure for each eye owns its mapped blink target
/// while present. The base value is never overwritten, so removing/loss of a
/// tracker restores editor and imported defaults automatically.
pub(super) fn update_eye_blink(
    avc_id: ComponentId,
    world: &mut World,
    humanoid_maps: &mut HumanoidBoneMapSystem,
    dt_sec: f32,
) {
    let closure = newest_direct_eye_closure(world, avc_id);
    let Some(gltf_id) = humanoid_maps.request_for_avc(world, avc_id) else {
        return;
    };
    let (closure, fallback) =
        if let Some(avatar) = world.get_component_by_id_as_mut::<AvatarControlComponent>(avc_id) {
            let raw = [closure.0, closure.1];
            let retained = avatar.eye_blink_state.update(raw, dt_sec);
            let fallback = avatar.idle_blink.then(|| {
                avatar
                    .eye_blink_state
                    .amount(avatar.blink_interval, avatar.blink_duration)
            });
            (
                if avatar.idle_blink {
                    (retained[0], retained[1])
                } else {
                    closure
                },
                fallback,
            )
        } else {
            (closure, None)
        };
    apply_factors(world, gltf_id, closure, fallback);
}

pub(super) fn newest_direct_eye_closure(
    world: &World,
    avc_id: ComponentId,
) -> (Option<f32>, Option<f32>) {
    let mut left = None::<(u64, f32)>;
    let mut right = None::<(u64, f32)>;
    for id in world.children_of(avc_id).iter().copied() {
        let sample = world
            .get_component_by_id_as::<XREyeTrackingComponent>(id)
            .map(|tracker| tracker.closure_sample)
            .or_else(|| {
                world
                    .get_component_by_id_as::<VRChatOSCEyeTrackingComponent>(id)
                    .map(|tracker| tracker.closure_sample)
            })
            .or_else(|| {
                world
                    .get_component_by_id_as::<XREyeTrackingHtcComponent>(id)
                    .map(|tracker| tracker.closure_sample)
            });
        let Some(sample) = sample else { continue };
        if let Some(value) = sample.left {
            if left.is_none_or(|(sequence, _)| sample.sequence > sequence) {
                left = Some((sample.sequence, value));
            }
        }
        if let Some(value) = sample.right {
            if right.is_none_or(|(sequence, _)| sample.sequence > sequence) {
                right = Some((sample.sequence, value));
            }
        }
    }
    (left.map(|(_, value)| value), right.map(|(_, value)| value))
}

#[cfg(test)]
pub(super) fn apply_eye_blink_drivers(
    world: &mut World,
    gltf_id: ComponentId,
    closure: (Option<f32>, Option<f32>),
) {
    apply_factors(world, gltf_id, closure, None);
}
fn apply_factors(
    world: &mut World,
    gltf_id: ComponentId,
    closure: (Option<f32>, Option<f32>),
    fallback: Option<f32>,
) {
    let labels = world.children_of(gltf_id).iter().copied().find_map(|id| {
        world
            .get_component_by_id_as::<crate::engine::ecs::component::MorphTargetMapComponent>(id)
            .map(|map| map.slots().clone())
    });
    let Some(labels) = labels else { return };
    let Some(gltf) =
        world.get_component_by_id_as_mut::<crate::engine::ecs::component::GLTFComponent>(gltf_id)
    else {
        return;
    };
    for (channel, closure) in [
        ("left_eye_blink", closure.0),
        ("right_eye_blink", closure.1),
    ] {
        let Some(label) = labels.get(channel) else {
            continue;
        };
        for info in gltf
            .morph_targets
            .iter()
            .filter(|info| info.label.as_deref() == Some(label.as_str()))
        {
            if let Some(state) = gltf.morph_factors.get_mut(&info.key) {
                state.driver = closure;
                state.idle_blink = fallback;
            }
        }
    }
}
