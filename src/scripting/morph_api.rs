//! Instance-scoped morph weights for authored animations and telemetry.
use crate::engine::ecs::component::{GLTFComponent, MorphTargetMapComponent};
use crate::engine::ecs::{ComponentId, World};
use meow_meow_script as mms;

pub(super) fn morph_weight(
    world: &crate::engine::ecs::World,
    id: ComponentId,
    slot: &str,
) -> Result<f32, String> {
    let model = world
        .get_component_by_id_as::<crate::engine::ecs::component::GLTFComponent>(id)
        .ok_or("receiver is not a GLTF instance")?;
    if !model.spawned {
        return Ok(0.);
    }
    let label = world
        .children_of(id)
        .iter()
        .find_map(|id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::MorphTargetMapComponent>(
                    *id,
                )
                .and_then(|map| map.slots().get(slot))
        })
        .ok_or("unknown morph slot")?;
    Ok(model
        .morph_targets
        .iter()
        .filter(|info| info.label.as_ref() == Some(label))
        .filter_map(|info| model.morph_factors.get(&info.key))
        .map(|state| state.effective())
        .fold(0., f32::max))
}

/// Script ownership is independent of tracker drivers and received bindings.
/// null releases the script value without changing imported/base defaults.
pub(super) fn set_weight(
    world: &mut World,
    id: ComponentId,
    slot: &str,
    value: Option<f32>,
) -> Result<(), String> {
    if value.is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v)) {
        return Err("morph weight must be finite and in [0,1]".into());
    }
    let label = world
        .children_of(id)
        .iter()
        .find_map(|id| {
            world
                .get_component_by_id_as::<MorphTargetMapComponent>(*id)
                .and_then(|map| map.slots().get(slot))
        })
        .cloned()
        .ok_or("unknown morph slot")?;
    let model = world
        .get_component_by_id_as_mut::<GLTFComponent>(id)
        .ok_or("receiver is not a GLTF instance")?;
    if !model.spawned {
        return Err("morph target is not initialized".into());
    }
    let mut found = false;
    for info in &model.morph_targets {
        if info.label.as_ref() == Some(&label) {
            if let Some(factor) = model.morph_factors.get_mut(&info.key) {
                factor.script = value;
                found = true;
            }
        }
    }
    if found {
        Ok(())
    } else {
        Err("mapped morph target was not imported".into())
    }
}

pub(super) fn invoke(
    world: &mut World,
    id: ComponentId,
    method: &str,
    args: &[mms::Value],
) -> Result<mms::Value, String> {
    use mms::Value as V;
    match (method, args) {
        ("morph_weight", [V::String(slot)]) => {
            morph_weight(world, id, slot).map(|v| V::Number(v as f64))
        }
        ("set_morph_weight", [V::String(slot), V::Number(value)]) => {
            set_weight(world, id, slot, Some(*value as f32))?;
            Ok(V::Null)
        }
        ("set_morph_weight", [V::String(slot), V::Null]) => {
            set_weight(world, id, slot, None)?;
            Ok(V::Null)
        }
        _ => Err("expected morph_weight(slot) or set_morph_weight(slot, weight_or_null)".into()),
    }
}

pub(super) fn configure(
    component: &mut mms::ComponentBuilder<'_, super::runtime_config::MittensBinding>,
    canonical: &'static str,
) {
    use mms::{ValueSignature as S, ValueType as T};
    super::runtime_config::host_method(
        component,
        canonical,
        "morph_weight",
        S::new(vec![T::String], T::F32),
    );
    super::runtime_config::host_method(
        component,
        canonical,
        "set_morph_weight",
        S::new(vec![T::String, T::Any], T::Null),
    );
}
