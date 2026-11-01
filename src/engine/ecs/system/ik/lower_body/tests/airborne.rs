use super::*;
#[test]
fn unsupported_legs_trail_world_motion_sideways_and_recover_on_landing() {
    let mut f = Fixture::walking(None, 0.2);
    f.world
        .get_component_by_id_as_mut::<CollidableComponent>(f.floor_collider)
        .unwrap()
        .enabled = false;
    for _ in 0..40 {
        f.travel(0.015, 0., 1. / 60.);
    }
    for foot in f.feet() {
        assert_eq!(foot.phase, FootPhase::Airborne);
        assert!(foot.support.is_none());
        assert!(
            foot.target_sole[0] < foot.base_sole[0] - 0.05,
            "feet must trail world X motion: {foot:?}"
        );
        assert!((foot.target_sole[2] - foot.base_sole[2]).abs() < 0.001);
    }
    f.assert_pose();
    for _ in 0..100 {
        f.travel(0., -0.015, 1. / 60.);
    }
    for foot in f.feet() {
        assert!(
            foot.target_sole[2] > foot.base_sole[2] + 0.05,
            "trail must follow a changed direction"
        );
        assert!((foot.target_sole[0] - foot.base_sole[0]).abs() < 0.01);
    }
    f.world
        .get_component_by_id_as_mut::<CollidableComponent>(f.floor_collider)
        .unwrap()
        .enabled = true;
    f.step();
    assert!(
        f.feet()
            .iter()
            .all(|foot| foot.support.is_some() && foot.phase != FootPhase::Airborne)
    );
    f.assert_pose();
}
#[test]
fn explicit_flight_disables_planting_even_while_touching_a_floor() {
    let mut f = Fixture::walking(None, 0.2);
    let mut input =
        crate::engine::ecs::component::InputXRGamepadComponent::new().enable_flying(true);
    input.flying = true;
    let gamepad = f.world.add_component(input);
    f.world.add_child(f.input, gamepad).unwrap();
    for _ in 0..25 {
        f.travel(0., 0.02, 1. / 60.);
    }
    assert!(
        f.feet()
            .iter()
            .all(|foot| foot.support.is_none() && foot.phase == FootPhase::Airborne)
    );
    assert!(
        f.feet()
            .iter()
            .all(|foot| foot.target_sole[2] < foot.base_sole[2] - 0.05)
    );
    f.world
        .get_component_by_id_as_mut::<crate::engine::ecs::component::InputXRGamepadComponent>(
            gamepad,
        )
        .unwrap()
        .flying = false;
    f.step();
    assert!(f.feet().iter().all(|foot| foot.support.is_some()));
    f.assert_pose();
}
