//! The burst rocket controller fires three rockets 0.4 s apart, alternating muzzles.
use super::rocket_motion_tests::{TestClock, advance};
use crate::engine::Universe;
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

fn flights(scene: &Universe) -> Vec<ComponentId> {
    scene
        .world
        .all_components()
        .filter(|&id| scene.world.component_label(id) == Some("sta_rocket_flight"))
        .collect()
}

fn event(scene: &mut Universe, name: &str) {
    scene.command_queue.push_event(
        named(scene, "burst_test_events"),
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
fn burst_fires_three_rockets_alternating_muzzles_and_ignores_presses_mid_burst() {
    let source = r#"
        import { burst_rocket_weapon_controller } from "../assets/components/weapons/burst_rocket_weapon_controller.mms"
        Clock.bpm(120.0)
        let left = T.position(-2.0, 1.0, 0.0) { name = "burst_test_left" }
        let right = T.position(2.0, 1.0, 0.0) { name = "burst_test_right" }
        left
        right
        let gate = { open = true }
        let rockets = burst_rocket_weapon_controller({ name = "burst_test" muzzles = [left, right]
            can_fire = fn() { return gate.open } })
        rockets
        let events = T { name = "burst_test_events" }
        events
        on(events, "DataEvent", fn(event) {
            if event == "Fire" { rockets.fire_burst() }
            if event == "Close" { gate.open = false }
        })
    "#;
    let clock = Arc::new(TestClock::default());
    let mut scene = Universe::new(World::default());
    scene.systems.clock.set_driver(clock.clone());
    scene
        .load_mms_source_at_path(source, "examples/_burst_rocket_test.mms")
        .unwrap();
    step(&mut scene, &clock, 4);
    assert!(flights(&scene).is_empty());

    event(&mut scene, "Fire");
    step(&mut scene, &clock, 12);
    let first = flights(&scene);
    assert_eq!(first.len(), 1, "one rocket at the start of the burst");
    let x = TransformSystem::world_position(&scene.world, first[0]).unwrap()[0];
    assert!(x < -1.0, "the first rocket leaves the left muzzle: {x}");

    // Pressing again mid-burst neither restarts nor adds a burst.
    event(&mut scene, "Fire");
    step(&mut scene, &clock, 42);
    let second = flights(&scene);
    assert_eq!(second.len(), 2, "the second rocket 0.4 s later");
    let new = second.iter().find(|id| !first.contains(id)).unwrap();
    let x = TransformSystem::world_position(&scene.world, *new).unwrap()[0];
    assert!(x > 1.0, "the second rocket leaves the right muzzle: {x}");

    step(&mut scene, &clock, 48);
    assert_eq!(flights(&scene).len(), 3, "the third rocket 0.8 s after the first");
    step(&mut scene, &clock, 48);
    assert_eq!(flights(&scene).len(), 3, "a burst is three rockets");

    // The gate is checked when a burst starts.
    event(&mut scene, "Close");
    event(&mut scene, "Fire");
    step(&mut scene, &clock, 12);
    assert_eq!(flights(&scene).len(), 3);
}
