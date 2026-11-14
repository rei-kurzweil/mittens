use super::*;

fn notifications(queue: &mut crate::engine::ecs::CommandQueue) -> Vec<crate::engine::ecs::Signal> {
    let mut rx = RxWorld::default();
    queue.drain_into_rx(&mut rx);
    rx.drain_ready_events()
        .into_iter()
        .filter(|signal| {
            matches!(
                signal.event,
                Some(EventSignal::PointerGrabbed { .. } | EventSignal::PointerReleased { .. })
            )
        })
        .collect()
}

#[test]
fn pointer_notifications_follow_successful_acquisition_and_actual_release() {
    let mut world = World::default();
    let driver = world.add_component(TransformComponent::new());
    let pointer = world.add_component(PointerComponent::new());
    world.add_child(driver, pointer).unwrap();
    let original = world.add_component(TransformComponent::new());
    let target = world.add_component(TransformComponent::new());
    world.add_child(original, target).unwrap();
    let renderable =
        world.add_component(crate::engine::ecs::component::RenderableComponent::cube());
    world.add_child(target, renderable).unwrap();
    let marker = world.add_component(GrabbableComponent::new());
    world.add_child(target, marker).unwrap();
    let mut system = GrabbableSystem::default();
    let assets = RenderAssets::new();
    let mut queue = crate::engine::ecs::CommandQueue::new();
    // No parent transform: acquisition fails and must not announce success.
    let invalid = world.add_component(PointerComponent::new());
    system.start(
        &mut world,
        &assets,
        &mut queue,
        invalid,
        target,
        renderable,
        [0.; 3],
        [0., 0., -1.],
    );
    assert!(notifications(&mut queue).is_empty());
    system.start(
        &mut world,
        &assets,
        &mut queue,
        pointer,
        target,
        renderable,
        [0.; 3],
        [0., 0., -1.],
    );
    let events = notifications(&mut queue);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].scope, pointer);
    assert!(
        matches!(events[0].event, Some(EventSignal::PointerGrabbed { pointer: p, target: t }) if p == pointer && t == target)
    );
    // Repeated input and another pointer cannot acquire the same local hold.
    system.start(
        &mut world,
        &assets,
        &mut queue,
        pointer,
        target,
        renderable,
        [0.; 3],
        [0., 0., -1.],
    );
    system.start(
        &mut world,
        &assets,
        &mut queue,
        invalid,
        target,
        renderable,
        [0.; 3],
        [0., 0., -1.],
    );
    assert!(notifications(&mut queue).is_empty());
    system.release(&mut world, &mut queue, pointer);
    system.release(&mut world, &mut queue, pointer);
    let events = notifications(&mut queue);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].scope, pointer);
    assert!(
        matches!(events[0].event, Some(EventSignal::PointerReleased { pointer: p, target: t }) if p == pointer && t == target)
    );
    assert_eq!(world.parent_of(target), Some(original));
    // MMS takeover uses existing reparent invalidation, not a new release API.
    system.start(
        &mut world,
        &assets,
        &mut queue,
        pointer,
        target,
        renderable,
        [0.; 3],
        [0., 0., -1.],
    );
    notifications(&mut queue);
    world.add_child(original, target).unwrap();
    system.tick(&mut world, &assets, &mut queue, 1. / 60.);
    let events = notifications(&mut queue);
    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0].event,
        Some(EventSignal::PointerReleased { .. })
    ));
    system.tick(&mut world, &assets, &mut queue, 1. / 60.);
    assert!(notifications(&mut queue).is_empty());
}
