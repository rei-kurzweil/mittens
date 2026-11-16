//! Amplitude-driven mouth opening.
use super::*;

/// Drive AVC's named amplitude fallback without touching the primary morph
/// driver. A future/live viseme driver therefore has deterministic priority.
pub(super) fn update_amplitude_mouth_open(
    avc_id: ComponentId,
    world: &mut World,
    humanoid_maps: &mut HumanoidBoneMapSystem,
    dt_sec: f32,
) {
    let (authored, cached, floor, ceiling, amount, smoothing, previous) = {
        let Some(avc) = world.get_component_by_id_as::<AvatarControlComponent>(avc_id) else {
            return;
        };
        (
            avc.mouth_open_amplitude.clone(),
            avc.resolved_mouth_open_amplitude,
            avc.mouth_open_rms_floor,
            avc.mouth_open_rms_ceiling,
            avc.mouth_open_amount,
            avc.mouth_open_smoothing,
            avc.mouth_open_weight,
        )
    };
    let Some(authored) = authored else { return };
    let source = cached
        .filter(|&id| is_level_provider(world, id))
        .or_else(|| {
            resolve_component_ref(world, &authored, Some(avc_id), QueryRootMode::WorldRoot)
        });
    let source = source.filter(|&id| is_level_provider(world, id));
    let target = source
        .and_then(|id| retained_level_sample(world, id))
        .map_or(0.0, |sample| {
            ((sample.rms - floor) / (ceiling - floor)).clamp(0.0, 1.0) * amount
        });
    let alpha = if smoothing <= 0.0 {
        1.0
    } else {
        1.0 - (-smoothing * dt_sec.max(0.0)).exp()
    };
    let weight = previous + (target - previous) * alpha;
    if let Some(avc) = world.get_component_by_id_as_mut::<AvatarControlComponent>(avc_id) {
        avc.resolved_mouth_open_amplitude = source;
        avc.mouth_open_weight = if weight.is_finite() {
            weight.clamp(0.0, 1.0)
        } else {
            0.0
        };
    }

    let Some(gltf_id) = humanoid_maps.request_for_avc(world, avc_id) else {
        return;
    };
    let label = world.children_of(gltf_id).iter().copied().find_map(|id| {
        world
            .get_component_by_id_as::<crate::engine::ecs::component::MorphTargetMapComponent>(id)
            .and_then(|map| map.slots().get("viseme_aa").cloned())
    });
    let matched_keys: Vec<_> = label.as_deref().map_or_else(Vec::new, |label| {
        world
            .get_component_by_id_as::<GLTFComponent>(gltf_id)
            .map(|gltf| {
                gltf.morph_targets
                    .iter()
                    .filter(|info| info.label.as_deref() == Some(label))
                    .map(|info| info.key)
                    .collect()
            })
            .unwrap_or_default()
    });
    if let Some(gltf) = world.get_component_by_id_as_mut::<GLTFComponent>(gltf_id) {
        for state in gltf.morph_factors.values_mut() {
            state.amplitude_mouth_open = None;
        }
        for key in &matched_keys {
            if let Some(state) = gltf.morph_factors.get_mut(key) {
                state.amplitude_mouth_open = Some(weight.clamp(0.0, 1.0));
            }
        }
    }
    if matched_keys.is_empty() {
        let should_log = world
            .get_component_by_id_as::<AvatarControlComponent>(avc_id)
            .is_some_and(|avc| !avc.mouth_open_missing_slot_diagnosed);
        if should_log {
            eprintln!(
                "[AVC][mouth_open_from_amplitude] avc={avc_id:?} missing or unresolved viseme_aa morph slot"
            );
            if let Some(avc) = world.get_component_by_id_as_mut::<AvatarControlComponent>(avc_id) {
                avc.mouth_open_missing_slot_diagnosed = true;
            }
        }
    }
}
