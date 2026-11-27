//! A rider standing on a moving deck follows it (tilting with it),
//! collides with its CollisionContext, and leaves level, with the deck's speed.
use super::rocket_motion_tests::{TestClock, advance};
use crate::engine::Universe;
use crate::engine::ecs::component::VelocityComponent;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::{ComponentId, EventSignal, SignalEmitter, World};
use std::sync::Arc;

fn named(scene: &Universe, label: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(label))
        .unwrap_or_else(|| panic!("missing {label}"))
}

fn position(scene: &Universe, label: &str) -> [f32; 3] {
    TransformSystem::world_position(&scene.world, named(scene, label)).unwrap()
}

fn event(scene: &mut Universe, name: &str) {
    scene.command_queue.push_event(
        named(scene, "carry_test_events"),
        EventSignal::DataEvent {
            name: name.into(),
            payload: None,
        },
    );
}

fn step(scene: &mut Universe, clock: &TestClock, frames: usize) {
    for _ in 0..frames {
        advance(scene, clock);
    }
}

#[test]
fn a_rider_follows_a_moving_deck_and_leaves_with_its_speed() {
    let source = r#"
        let deck_floor = T.position(0.0, -0.1, 0.0).scale(6.0, 0.2, 4.0) {
            Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.85) {} }
        }
        let deck = CollisionContext { name = "carry_test_deck" deck_floor }
        let board = Zone.cube([3.0, 0.6, 2.0]) { name = "carry_test_board" }
        let stay = Zone.cube([3.6, 2.0, 2.6]) { name = "carry_test_stay" }
        let board_frame = T.position(0.0, 0.6, 0.0) { board }
        let stay_frame = T.position(0.0, 1.5, 0.0) { stay }
        let carrier = Mountable.carry().entry_zone(stay).auto_mount_zone(board).context(deck) {}
        let platform = T.position(0.0, 1.0, 0.0) {
            name = "carry_test_platform"
            deck board_frame stay_frame carrier
        }
        let platform_motion = Velocity { name = "carry_test_platform_velocity" platform }
        platform_motion

        let rider_root = T.position(0.5, 1.35, 0.0) {
            name = "carry_test_rider"
            Zone.sphere(0.3) { Collidable.slide() {} }
        }
        let rider_velocity = Velocity { name = "carry_test_rider_velocity" rider_root }
        let rider_gravity = Gravity { rider_velocity }
        rider_gravity
        let rider = Rider.anchor(rider_root).movement_root(rider_root) {}
        rider_root.attach(rider)

        let events = T { name = "carry_test_events" }
        events
        on(events, "DataEvent", fn(event) {
            if event == "Go" { platform_motion.translate([2.0, 0.0, 0.0]) }
            if event == "Tilt" { platform_motion.rotate([0.2, 0.0, 0.0]) }
            if event == "Level" { platform_motion.rotate([-0.2, 0.0, 0.0]) }
            if event == "Jump" { rider_velocity.translate([0.0, 3.0, 8.0]) }
        })
    "#;
    let clock = Arc::new(TestClock::default());
    let mut scene = Universe::new(World::default());
    scene.systems.clock.set_driver(clock.clone());
    scene
        .load_mms_source_at_path(source, "examples/_vehicle_carry_test.mms")
        .unwrap();

    // The deck's floor lives only in its CollisionContext; boarding points
    // the rider at it, so the rider lands on it instead of falling through.
    step(&mut scene, &clock, 30);
    let rider = named(&scene, "carry_test_rider");
    let deck = named(&scene, "carry_test_deck");
    assert_eq!(scene.systems.static_contact.outbound_context(rider), Some(deck));
    let landed = position(&scene, "carry_test_rider");
    assert!((landed[1] - 1.3).abs() < 0.05, "standing on the deck: {landed:?}");

    // Two seconds at 2 m/s: the rider keeps its spot on the deck.
    event(&mut scene, "Go");
    step(&mut scene, &clock, 240);
    let platform = position(&scene, "carry_test_platform");
    let carried = position(&scene, "carry_test_rider");
    assert!((platform[0] - 4.0).abs() < 0.1, "platform moved: {platform:?}");
    assert!(
        (carried[0] - platform[0] - 0.5).abs() < 0.05 && (carried[1] - 1.3).abs() < 0.05,
        "rider kept its spot: {carried:?} on {platform:?}"
    );

    // The deck pitches about 11 degrees while moving: the rider keeps its
    // spot in the deck's own coordinates and tilts with it.
    event(&mut scene, "Tilt");
    step(&mut scene, &clock, 120);
    event(&mut scene, "Level");
    step(&mut scene, &clock, 2);
    let platform_world =
        TransformSystem::world_model(&scene.world, named(&scene, "carry_test_platform")).unwrap();
    let carried = position(&scene, "carry_test_rider");
    let on_deck = crate::utils::math::mat4_mul_vec4(
        crate::utils::math::mat4_inverse(platform_world).unwrap(),
        [carried[0], carried[1], carried[2], 1.0],
    );
    assert!(
        (on_deck[0] - 0.5).abs() < 0.1 && (on_deck[1] - 0.3).abs() < 0.05 && on_deck[2].abs() < 0.1,
        "rider kept its spot on the tilted deck: {on_deck:?}"
    );
    let pitch = |label: &str| {
        let model = TransformSystem::world_model(&scene.world, named(&scene, label)).unwrap();
        model[1][2].atan2(model[1][1])
    };
    let deck_pitch = pitch("carry_test_platform");
    assert!(deck_pitch.abs() > 0.15, "the deck tilted: {deck_pitch}");
    assert!(
        (pitch("carry_test_rider") - deck_pitch).abs() < 0.02,
        "the rider tilts with it"
    );

    // Jumping off the side ends the carry with the deck's speed kept.
    event(&mut scene, "Jump");
    step(&mut scene, &clock, 60);
    assert_eq!(scene.systems.static_contact.outbound_context(rider), None);
    let velocity = scene
        .world
        .get_component_by_id_as::<VelocityComponent>(named(&scene, "carry_test_rider_velocity"))
        .unwrap()
        .linear_local_mps;
    assert!((velocity[0] - 2.0).abs() < 0.2, "kept the deck's speed: {velocity:?}");
    let rotation = scene
        .world
        .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(rider)
        .unwrap()
        .transform
        .rotation;
    assert!(
        rotation[0].abs() < 1e-4 && rotation[2].abs() < 1e-4,
        "stepping off levels the rider: {rotation:?}"
    );
}
