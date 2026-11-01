//! Resolve effective body-local and inherited gravity, independent of locomotion.
use crate::engine::ecs::component::{GravityComponent, VelocityComponent};
use crate::engine::ecs::{ComponentId, World};
pub(super) fn provider(world: &World, velocity_id: ComponentId) -> Option<ComponentId> {
    world.get_component_by_id_as::<VelocityComponent>(velocity_id)?;
    // Exact-body overrides are ordinary Gravity components. Multiple
    // independent Off overrides compose: releasing one to Auto cannot
    // accidentally re-enable gravity while another still blocks it.
    let mut local = None;
    for &child in world.children_of(velocity_id) {
        if let Some(gravity) = world.get_component_by_id_as::<GravityComponent>(child) {
            if gravity.inherit {
                continue;
            }
            if !gravity.enabled {
                return Some(child);
            }
            local.get_or_insert(child);
        }
    }
    if local.is_some() {
        return local;
    }
    let mut current = world.parent_of(velocity_id);
    while let Some(id) = current {
        if world
            .get_component_by_id_as::<VelocityComponent>(id)
            .is_some()
        {
            break;
        }
        if world
            .get_component_by_id_as::<GravityComponent>(id)
            .is_some_and(|gravity| !gravity.inherit)
        {
            return Some(id);
        }
        current = world.parent_of(id);
    }
    None
}

#[cfg(test)]
#[path = "gravity_scope_tests.rs"]
mod tests;
