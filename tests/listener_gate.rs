use mittens_engine::engine::ecs::{RxWorld, SignalKind, World, component::TransformComponent};

fn noop_handler(_: &mut World, _: &mut dyn mittens_engine::engine::ecs::SignalEmitter, _: &mittens_engine::engine::ecs::Signal) {}

#[test]
fn scoped_listener_gate_follows_event_scope_ancestry() {
    let mut world = World::default();
    let root = world.add_component(TransformComponent::new());
    let child = world.add_component(TransformComponent::new());
    let unrelated = world.add_component(TransformComponent::new());
    world.set_parent(child, Some(root)).unwrap();

    let rx = RxWorld::default();
    assert!(!rx.listener_gate(SignalKind::ProjectileHit).any());

    let mut rx = rx;
    rx.add_handler_closure(SignalKind::ProjectileHit, root, |_, _, _| {});
    let gate = rx.listener_gate(SignalKind::ProjectileHit);

    assert!(gate.any());
    assert!(gate.allows(&world, root));
    assert!(gate.allows(&world, child));
    assert!(!gate.allows(&world, unrelated));
}

#[test]
fn global_and_any_listeners_open_the_gate_for_all_scopes() {
    let mut world = World::default();
    let first = world.add_component(TransformComponent::new());
    let second = world.add_component(TransformComponent::new());
    let mut rx = RxWorld::default();

    rx.add_global_handler(SignalKind::ProjectileHit, noop_handler);
    let gate = rx.listener_gate(SignalKind::ProjectileHit);
    assert!(gate.allows(&world, first));
    assert!(gate.allows(&world, second));

    let mut rx = RxWorld::default();
    rx.add_handler_closure(SignalKind::Any, first, |_, _, _| {});
    let gate = rx.listener_gate(SignalKind::ProjectileHit);
    assert!(gate.allows(&world, first));
    assert!(!gate.allows(&world, second));
}

#[test]
fn bypassed_event_bindings_do_not_keep_the_listener_gate_open() {
    let mut world = World::default();
    let root = world.add_component(TransformComponent::new());
    let child = world.add_component(TransformComponent::new());
    world.set_parent(child, Some(root)).unwrap();

    let mut rx = RxWorld::default();
    rx.add_event_binding(SignalKind::ProjectileHit, Some(root), 7, None, None, |_, _, _| {});
    assert!(rx.listener_gate(SignalKind::ProjectileHit).allows(&world, child));

    assert!(rx.bypass_event_binding(7, true));
    assert!(!rx.listener_gate(SignalKind::ProjectileHit).allows(&world, child));

    assert!(rx.bypass_event_binding(7, false));
    assert!(rx.listener_gate(SignalKind::ProjectileHit).allows(&world, child));
}
