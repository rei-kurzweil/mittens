use super::*;
use crate::engine::ecs::system::VelocitySystem;
use crate::engine::ecs::{
    RxWorld,
    component::{GravityMode, TransformComponent},
};
fn child<T: crate::engine::ecs::component::Component>(
    w: &mut World,
    p: ComponentId,
    c: T,
) -> ComponentId {
    let id = w.add_component(c);
    w.add_child(p, id).unwrap();
    id
}
#[test]
fn auto_inherits_live_values_and_off_blocks_without_resetting_velocity() {
    let mut world = World::default();
    let base = world.add_component(GravityComponent::new().with_coefficient(0.4));
    let override_id = child(&mut world, base, GravityComponent::auto());
    let velocity = child(&mut world, override_id, VelocityComponent::new());
    child(&mut world, velocity, TransformComponent::new());
    let mut physics = VelocitySystem::default();
    let mut emit = RxWorld::default();
    assert_eq!(provider(&world, velocity), Some(base));
    physics.step(&mut world, &mut emit);
    let first = world
        .get_component_by_id_as::<VelocityComponent>(velocity)
        .unwrap()
        .linear_local_mps;
    assert!((first[1] + 9.81 * 0.4 / 120.).abs() < 1e-6);
    world
        .get_component_by_id_as_mut::<GravityComponent>(override_id)
        .unwrap()
        .set_mode(GravityMode::Off);
    physics.step(&mut world, &mut emit);
    assert_eq!(
        world
            .get_component_by_id_as::<VelocityComponent>(velocity)
            .unwrap()
            .linear_local_mps,
        first,
        "changing gravity does not erase momentum"
    );
    world
        .get_component_by_id_as_mut::<GravityComponent>(base)
        .unwrap()
        .set_coefficient(-0.25)
        .unwrap();
    world
        .get_component_by_id_as_mut::<GravityComponent>(override_id)
        .unwrap()
        .set_mode(GravityMode::Auto);
    physics.step(&mut world, &mut emit);
    let next = world
        .get_component_by_id_as::<VelocityComponent>(velocity)
        .unwrap()
        .linear_local_mps;
    assert!(
        (next[1] - first[1] - 9.81 * 0.25 / 120.).abs() < 1e-6,
        "Auto must use the current inherited coefficient"
    );
}
#[test]
fn independent_body_local_off_overrides_do_not_affect_siblings_or_each_other() {
    let mut world = World::default();
    let base = world.add_component(GravityComponent::new().with_coefficient(0.6));
    let body = child(&mut world, base, VelocityComponent::new());
    child(&mut world, body, TransformComponent::new());
    let sibling = child(&mut world, base, VelocityComponent::new());
    child(&mut world, sibling, TransformComponent::new());
    let first = child(&mut world, body, GravityComponent::off());
    let second = child(&mut world, body, GravityComponent::off());
    assert_eq!(provider(&world, body), Some(first));
    assert_eq!(provider(&world, sibling), Some(base));
    world
        .get_component_by_id_as_mut::<GravityComponent>(first)
        .unwrap()
        .set_mode(GravityMode::Auto);
    assert_eq!(provider(&world, body), Some(second));
    world
        .get_component_by_id_as_mut::<GravityComponent>(second)
        .unwrap()
        .set_mode(GravityMode::Auto);
    assert_eq!(provider(&world, body), Some(base));
    world
        .get_component_by_id_as_mut::<GravityComponent>(base)
        .unwrap()
        .set_enabled(false);
    assert!(
        !world
            .get_component_by_id_as::<GravityComponent>(provider(&world, body).unwrap())
            .unwrap()
            .enabled
    );
    // A local Auto override does not cross an intervening Velocity boundary.
    let nested = child(&mut world, body, VelocityComponent::new());
    child(&mut world, nested, GravityComponent::auto());
    assert_eq!(provider(&world, nested), None);
}
