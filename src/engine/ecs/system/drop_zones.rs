use crate::engine::ecs::component::ZoneComponent;
use crate::engine::ecs::system::{TransformSystem, zone_query};
use crate::engine::ecs::{ComponentId, EventSignal, World};
use crate::utils::math::{vec3_dot, vec3_sub};

pub(super) fn drop_event(
    world: &World,
    pointer: ComponentId,
    target: ComponentId,
    attachment_index: Option<i32>,
) -> Option<EventSignal> {
    let mut receivers = world.drop_zone_ids().peekable();
    receivers.peek()?;
    let origin = TransformSystem::world_position(world, target)?;
    let probes = zone_query::zones_in_subtree(world, target, None);

    receivers
        .filter_map(|zone| {
            let receiver = world.get_component_by_id_as::<ZoneComponent>(zone)?;
            let limit = receiver.drop_index?;
            if !receiver.enabled
                || !world.is_initialized(zone)
                || attachment_index.is_some_and(|index| index > limit)
            {
                return None;
            }
            let frame = zone_query::resolve_zone_frame(world, zone, receiver).ok()?;
            if within(world, zone, target) || within(world, frame, target) {
                return None;
            }
            let other_zone = if probes.is_empty() {
                let relation = zone_query::classify_point(world, zone, origin).ok()?;
                if relation == zone_query::ZoneRelation::Outside {
                    return None;
                }
                None
            } else {
                Some(*probes.iter().find(|&&probe| {
                    zone_query::overlap_zones(world, zone, probe).is_ok_and(|hit| hit.intersects)
                })?)
            };
            let offset = vec3_sub(TransformSystem::world_position(world, frame)?, origin);
            Some((vec3_dot(offset, offset), zone, other_zone))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)))
        .map(|(_, zone, other_zone)| EventSignal::ZoneDropped {
            zone,
            other_zone,
            pointer,
            target,
        })
}

fn within(world: &World, mut component: ComponentId, root: ComponentId) -> bool {
    loop {
        if component == root {
            return true;
        }
        let Some(parent) = world.parent_of(component) else {
            return false;
        };
        component = parent;
    }
}
