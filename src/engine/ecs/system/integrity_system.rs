//! Applies the frame's damage impacts to Integrity components: any part, cut
//! or not, loses integrity when an impact sphere reaches it, and each crossed
//! threshold emits one DataEvent on the component. Runs only on frames with
//! impacts, after the destruction system has drained them.
use crate::engine::ecs::component::{DamageComponent, IntegrityComponent};
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::system::pointer_system::nearest_ancestor_transform;
use crate::engine::ecs::{EventSignal, SignalEmitter, World};

pub(crate) fn apply_damage(
    world: &mut World,
    impacts: &[DamageComponent],
    emit: &mut dyn SignalEmitter,
) {
    if impacts.is_empty() {
        return;
    }
    let parts: Vec<_> = world
        .all_components()
        .filter(|&id| {
            world
                .get_component_by_id_as::<IntegrityComponent>(id)
                .is_some_and(|integrity| integrity.enabled)
        })
        .collect();
    for part in parts {
        let Some(center) = world
            .parent_of(part)
            .and_then(|parent| nearest_ancestor_transform(world, parent))
            .and_then(|frame| TransformSystem::world_position(world, frame))
        else {
            continue;
        };
        let reach = world
            .get_component_by_id_as::<IntegrityComponent>(part)
            .map(|integrity| integrity.reach)
            .unwrap_or(0.0);
        let mut crossed = Vec::new();
        for impact in impacts {
            let distance = (0..3)
                .map(|i| (impact.position[i] - center[i]).powi(2))
                .sum::<f32>()
                .sqrt();
            if distance > reach + impact.radius {
                continue;
            }
            if let Some(integrity) = world.get_component_by_id_as_mut::<IntegrityComponent>(part)
            {
                crossed.extend(integrity.damage(impact.amount));
            }
        }
        for name in crossed {
            emit.push_event(part, EventSignal::DataEvent { name, payload: None });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::CommandQueue;
    use crate::engine::ecs::component::TransformComponent;

    #[test]
    fn impacts_in_reach_lower_integrity_and_cross_each_threshold_once() {
        let mut world = World::default();
        let frame = world.add_component(TransformComponent::new().with_position(1.0, 0.0, 0.0));
        let transform = world
            .get_component_by_id_as_mut::<TransformComponent>(frame)
            .unwrap();
        transform.transform.matrix_world = transform.transform.model;
        let mut integrity = IntegrityComponent::new();
        integrity.reach = 0.5;
        integrity.thresholds = vec![("smoking".into(), 0.5, false), ("wrecked".into(), 0.0, false)];
        let part = world.add_component(integrity);
        world.add_child(frame, part).unwrap();
        let hit = |x: f32, amount: f32| DamageComponent {
            position: [x, 0.0, 0.0],
            radius: 0.3,
            amount,
            blast: None,
        };
        let mut emit = CommandQueue::new();
        let fraction = |world: &World| {
            world
                .get_component_by_id_as::<IntegrityComponent>(part)
                .unwrap()
                .fraction()
        };
        // Out of reach: nothing happens.
        apply_damage(&mut world, &[hit(5.0, 60.0)], &mut emit);
        assert_eq!(fraction(&world), 1.0);
        // Two hits in reach: past half once, then to zero.
        apply_damage(&mut world, &[hit(1.2, 60.0)], &mut emit);
        assert!((fraction(&world) - 0.4).abs() < 1e-6);
        apply_damage(&mut world, &[hit(1.2, 60.0), hit(0.9, 10.0)], &mut emit);
        assert_eq!(fraction(&world), 0.0);
        let mut rx = crate::engine::ecs::RxWorld::default();
        emit.drain_into_rx(&mut rx);
        let names: Vec<String> = rx
            .drain_ready_events()
            .into_iter()
            .filter_map(|signal| match signal.event {
                Some(EventSignal::DataEvent { name, .. }) => Some(name),
                _ => None,
            })
            .collect();
        assert_eq!(names, vec!["smoking".to_string(), "wrecked".to_string()]);
    }
}
