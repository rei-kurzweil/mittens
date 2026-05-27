use crate::engine::ecs::component::{ActionComponent, KeyframeComponent};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};

/// Called when an `ActionComponent` is registered (currently during component init).
///
/// Policy:
/// - If the action is under a `KeyframeComponent` ancestor, do **not** auto-fire.
/// - Otherwise, auto-fire by emitting the stored `IntentValue`.
pub(crate) fn register_action(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    component: ComponentId,
) {
    let Some(action) = world.get_component_by_id_as::<ActionComponent>(component) else {
        return;
    };

    // ActionComponents are declarative: they auto-fire at init unless they live under a keyframe.
    let mut cur = component;
    while let Some(parent) = world.parent_of(cur) {
        if world
            .get_component_by_id_as::<KeyframeComponent>(parent)
            .is_some()
        {
            return;
        }
        cur = parent;
    }

    // Avoid accidental recursion if someone constructs an invalid action payload.
    if matches!(action.signal, IntentValue::RegisterAction { .. }) {
        return;
    }

    emit.push_intent_now(component, action.signal.clone());
}
