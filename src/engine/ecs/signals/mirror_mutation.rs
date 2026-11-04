use crate::engine::ecs::{ComponentId, World, component::MirrorComponent, system::SystemWorld};
use crate::engine::graphics::VisualWorld;

// MMS callbacks can run after the frame's mirror tick. Publish an actual
// enabled-state change before drawing so a paused mirror cannot capture once
// more with stale metadata. Unchanged values do no extra work.
pub(super) fn apply(
    systems: &mut SystemWorld,
    world: &mut World,
    visuals: &mut VisualWorld,
    id: ComponentId,
    enabled: bool,
) {
    let Some(mirror) = world.get_component_by_id_as_mut::<MirrorComponent>(id) else {
        return;
    };
    if mirror.enabled == enabled {
        return;
    }
    mirror.enabled = enabled;
    systems.mirror.refresh_captures(world, visuals);
    for texture in systems.mirror.take_pending_texture_registrations() {
        systems.register_texture(world, visuals, texture);
    }
}
