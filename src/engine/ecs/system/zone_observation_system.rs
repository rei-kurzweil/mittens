use std::collections::{BTreeMap, HashMap};

use super::static_contact_system::movement_target;
use super::{AttachmentSystem, zone_query};
use crate::engine::ecs::component::{CollidableComponent, CollidableMode, ZoneComponent};
use crate::engine::ecs::{ComponentId, EventSignal, SignalEmitter, World};

/// Opt-in overlap observation with downward capsule/box crossing detection. Sensors never apply contact response.
#[derive(Debug, Default)]
pub struct ZoneObservationSystem {
    previous_centers: HashMap<ComponentId, [f32; 3]>,
    overlaps: BTreeMap<(ComponentId, ComponentId), (ComponentId, ComponentId, [f32; 3])>,
}

impl ZoneObservationSystem {
    /// A teleport is a discontinuity, not a sweep through intervening sensors.
    pub(crate) fn forget_target(&mut self, world: &World, target: ComponentId) {
        self.previous_centers.retain(|zone, _| {
            !world
                .children_of(*zone)
                .iter()
                .any(|id| movement_target(world, *id, *zone) == Some(target))
        });
    }

    pub fn tick(&mut self, world: &World, mounts: &AttachmentSystem, emit: &mut dyn SignalEmitter) {
        let sensors: Vec<_> = world
            .all_components()
            .filter(|id| {
                world
                    .get_component_by_id_as::<ZoneComponent>(*id)
                    .is_some_and(|zone| zone.enabled && zone.events_enabled)
            })
            .collect();
        if sensors.is_empty() {
            self.overlaps.clear();
            self.previous_centers.clear();
            return;
        }
        let movers: Vec<_> = world
            .all_components()
            .filter_map(|id| {
                let collidable = world.get_component_by_id_as::<CollidableComponent>(id)?;
                if !collidable.enabled || collidable.mode != CollidableMode::Slide {
                    return None;
                }
                let zone = world.parent_of(id)?;
                if !world
                    .get_component_by_id_as::<ZoneComponent>(zone)
                    .is_some_and(|z| z.enabled)
                {
                    return None;
                }
                let target = movement_target(world, id, zone)?;
                (!mounts.is_movement_root_mounted(target)).then_some((id, zone, target))
            })
            .collect();
        let mut centers = HashMap::new();
        for &(_, zone, _) in &movers {
            if let Some(center) = world
                .get_component_by_id_as::<ZoneComponent>(zone)
                .and_then(|z| zone_query::resolve_zone_frame(world, zone, z).ok())
                .and_then(|frame| super::TransformSystem::world_position(world, frame))
            {
                centers.insert(zone, center);
            }
        }
        let mut current = BTreeMap::new();
        for sensor in sensors {
            for &(collidable, other_zone, movement_target) in &movers {
                if sensor == other_zone {
                    continue;
                }
                let overlaps = zone_query::overlap_zones(world, sensor, other_zone)
                    .is_ok_and(|overlap| overlap.intersects);
                let crossed = self
                    .previous_centers
                    .get(&other_zone)
                    .is_some_and(|previous| {
                        zone_query::sweep_capsule_floor(world, other_zone, sensor, *previous)
                            .is_ok_and(|hit| hit.is_some())
                    });
                if overlaps || crossed {
                    let key = (sensor, other_zone);
                    let Some(center) = world
                        .get_component_by_id_as::<ZoneComponent>(other_zone)
                        .and_then(|z| zone_query::resolve_zone_frame(world, other_zone, z).ok())
                        .and_then(|frame| super::TransformSystem::world_position(world, frame))
                    else {
                        continue;
                    };
                    let Some(target_position) =
                        super::TransformSystem::world_position(world, movement_target)
                    else {
                        continue;
                    };
                    let movement_target_offset =
                        std::array::from_fn(|i| target_position[i] - center[i]);
                    current.insert(key, (collidable, movement_target, movement_target_offset));
                    if !self.overlaps.contains_key(&key) {
                        emit.push_event(
                            sensor,
                            EventSignal::ZoneEntered {
                                zone: sensor,
                                other_zone,
                                collidable,
                                movement_target,
                                movement_target_offset,
                            },
                        );
                    }
                }
            }
        }
        // Disabled or removed participants simply retire their overlap. Exit
        // callbacks require live, enabled participants; stale handles are not delivered.
        for (&(zone, other_zone), &(collidable, movement_target, movement_target_offset)) in
            &self.overlaps
        {
            if !current.contains_key(&(zone, other_zone))
                && world
                    .get_component_by_id_as::<ZoneComponent>(zone)
                    .is_some_and(|z| z.enabled && z.events_enabled)
                && world
                    .get_component_by_id_as::<ZoneComponent>(other_zone)
                    .is_some_and(|z| z.enabled)
                && world
                    .get_component_by_id_as::<CollidableComponent>(collidable)
                    .is_some_and(|c| c.enabled)
                && world.get_component_record(movement_target).is_some()
            {
                emit.push_event(
                    zone,
                    EventSignal::ZoneExited {
                        zone,
                        other_zone,
                        collidable,
                        movement_target,
                        movement_target_offset,
                    },
                );
            }
        }
        self.overlaps = current;
        self.previous_centers = centers;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::TransformComponent;
    use crate::engine::ecs::{CommandQueue, RxWorld};

    #[test]
    fn observation_is_opt_in_and_emits_one_enter_exit_per_overlap() {
        let mut world = World::default();
        let sensor_frame = world.add_component(TransformComponent::new());
        let sensor = world.add_component(ZoneComponent::cube([2.0; 3]));
        world.add_child(sensor_frame, sensor).unwrap();
        let frame = world.add_component(TransformComponent::new());
        let mover = world.add_component(ZoneComponent::capsule_y(0.2, 0.3));
        let collidable = world.add_component(CollidableComponent::slide());
        world.add_child(frame, mover).unwrap();
        world.add_child(mover, collidable).unwrap();
        let mut system = ZoneObservationSystem::default();
        let mounts = AttachmentSystem::default();
        let mut queue = CommandQueue::new();
        let mut rx = RxWorld::default();
        system.tick(&world, &mounts, &mut queue);
        queue.drain_into_rx(&mut rx);
        assert!(rx.drain_ready_events().is_empty());
        world
            .get_component_by_id_as_mut::<ZoneComponent>(sensor)
            .unwrap()
            .events_enabled = true;
        system.tick(&world, &mounts, &mut queue);
        system.tick(&world, &mounts, &mut queue);
        queue.drain_into_rx(&mut rx);
        let events = rx.drain_ready_events();
        assert_eq!(events.len(), 1);
        assert!(
            matches!(events[0].event, Some(EventSignal::ZoneEntered { zone, other_zone, movement_target, .. }) if zone == sensor && other_zone == mover && movement_target == frame)
        );
        let t = world
            .get_component_by_id_as_mut::<TransformComponent>(frame)
            .unwrap();
        t.transform.matrix_world[3][0] = 5.0;
        system.tick(&world, &mounts, &mut queue);
        system.tick(&world, &mounts, &mut queue);
        queue.drain_into_rx(&mut rx);
        let events = rx.drain_ready_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0].event,
            Some(EventSignal::ZoneExited { .. })
        ));
        world
            .get_component_by_id_as_mut::<TransformComponent>(frame)
            .unwrap()
            .transform
            .matrix_world[3][0] = 0.0;
        system.tick(&world, &mounts, &mut queue);
        queue.drain_into_rx(&mut rx);
        assert!(matches!(
            rx.drain_ready_events()[0].event,
            Some(EventSignal::ZoneEntered { .. })
        ));
        world
            .get_component_by_id_as_mut::<ZoneComponent>(sensor)
            .unwrap()
            .enabled = false;
        system.tick(&world, &mounts, &mut queue);
        queue.drain_into_rx(&mut rx);
        assert!(rx.drain_ready_events().is_empty());
        world
            .get_component_by_id_as_mut::<ZoneComponent>(sensor)
            .unwrap()
            .enabled = true;
        system.tick(&world, &mounts, &mut queue);
        queue.drain_into_rx(&mut rx);
        rx.drain_ready_events();
        system.forget_target(&world, frame);
        world
            .get_component_by_id_as_mut::<TransformComponent>(frame)
            .unwrap()
            .transform
            .matrix_world[3][1] = -100.0;
        system.tick(&world, &mounts, &mut queue);
        queue.drain_into_rx(&mut rx);
        let events = rx.drain_ready_events();
        assert_eq!(events.len(), 1);
        assert!(
            matches!(events[0].event, Some(EventSignal::ZoneExited { .. })),
            "teleport must not sweep across the sensor"
        );
    }
}
