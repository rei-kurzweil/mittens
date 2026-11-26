use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

use mittens_engine::engine::{
    Universe,
    ecs::{
        ComponentId, EventSignal, SignalEmitter, SignalKind, World, component::TransformComponent,
        system::ClockDriver,
    },
};

#[derive(Default)]
struct TestClock(AtomicU64);

impl ClockDriver for TestClock {
    fn name(&self) -> &'static str {
        "mitten-test"
    }

    fn time_now_sec(&self) -> f64 {
        self.0.load(Ordering::Relaxed) as f64 / 60.0
    }
}

fn advance(scene: &mut Universe, clock: &TestClock, frames: usize) {
    for _ in 0..frames {
        clock.0.fetch_add(1, Ordering::Relaxed);
        scene.update(1.0 / 60.0, &Default::default());
    }
}

fn named(scene: &Universe, name: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(name))
        .unwrap_or_else(|| panic!("missing {name}"))
}

#[test]
fn mitten_enables_frame_callbacks_only_for_active_behaviors() {
    let mut scene = Universe::new(World::default());
    scene
        .systems
        .xr
        .set_external_frame(Some(Default::default()));
    scene
        .load_mms_source_at_path(
            r#"
            import { practice_target } from "../assets/components/target_practice.mms"
            import { mitten_antagonist } from "../assets/components/characters/target_combat.mms"
            import { mitten_route } from "../assets/components/characters/mitten_equipment.mms"
            Clock.bpm(120.0)
            let player = T.position(0.0, 1.0, -20.0) {}
            player
            let target = practice_target({
                prefix = "fixture" avatar = "mitten" use_xr = false use_desktop = false
                position = [0.0, 0.0, 0.0] yaw = 0.0 strafe = 0.0 player = player
                combat = mitten_antagonist
                retreat = fn(here, threat) { return mitten_route(here, [[here[0] + 0.6, 0.0, here[2]]]) }
            })
            target.root
            let controls = T { name = "fixture_controls" }
            controls
            on(controls, "DataEvent", fn(event) {
                if event == "Hit" { target.pistol_hit({ renderable = { player_hit_zone = target.npc.hit_zone } }) }
                if event == "Remove" { target.remove() }
            })
            "#,
            "examples/_mitten_activity_test.mms",
        )
        .unwrap();
    let clock = Arc::new(TestClock::default());
    scene.systems.clock.set_driver(clock.clone());
    advance(&mut scene, &clock, 8);

    let has_frame_tick =
        |scene: &Universe| scene.systems.rx.has_global_handlers(SignalKind::FrameTick);
    assert!(!has_frame_tick(&scene), "initial wait");
    let shots = Arc::new(Mutex::new(Vec::new()));
    let captured = shots.clone();
    let shot_clock = clock.clone();
    let emitter = named(&scene, "fixture_pistol_laser_completion");
    scene
        .systems
        .rx
        .add_handler_closure(SignalKind::DataEvent, emitter, move |_, _, signal| {
            if let Some(EventSignal::DataEvent { name, .. }) = &signal.event
                && name == "ShotFired"
            {
                captured.lock().unwrap().push(shot_clock.time_now_sec());
            }
        });

    advance(&mut scene, &clock, 100);
    assert!(has_frame_tick(&scene), "aiming before the first shot");
    advance(&mut scene, &clock, 45);
    assert!(!has_frame_tick(&scene), "between single shots");
    advance(&mut scene, &clock, 510);
    assert!(has_frame_tick(&scene), "aiming during the first burst");
    advance(&mut scene, &clock, 185);
    assert!(!has_frame_tick(&scene), "automatic cooldown");
    let first_burst = shots.lock().unwrap().clone();
    assert_eq!(first_burst.iter().filter(|&&t| t < 10.3).count(), 5);
    assert!(first_burst.len() >= 30);

    let controls = named(&scene, "fixture_controls");
    let signal = |name: &str| EventSignal::DataEvent {
        name: name.into(),
        payload: None,
    };
    scene.command_queue.push_event(controls, signal("Hit"));
    advance(&mut scene, &clock, 3);
    assert!(
        has_frame_tick(&scene),
        "retreat during the automatic cooldown"
    );
    advance(&mut scene, &clock, 40);
    assert!(!has_frame_tick(&scene), "waiting in cover after arrival");

    advance(&mut scene, &clock, 267);
    let fired_again = shots.lock().unwrap()[first_burst.len()];
    let gap = fired_again - first_burst.last().unwrap();
    assert!((5.0..5.3).contains(&gap), "automatic firing pause: {gap}");
    scene.command_queue.push_event(controls, signal("Remove"));
    advance(&mut scene, &clock, 3);
    assert!(!has_frame_tick(&scene), "removal during a burst");
    let total = shots.lock().unwrap().len();
    advance(&mut scene, &clock, 360);
    assert_eq!(
        shots.lock().unwrap().len(),
        total,
        "removed timers do not fire"
    );
}

#[test]
fn mitten_restores_the_seated_pose_each_time_she_boards() {
    let mut scene = Universe::new(World::default());
    scene
        .systems
        .xr
        .set_external_frame(Some(Default::default()));
    scene
        .load_mms_source_at_path(
            r#"
            import { practice_target } from "../assets/components/target_practice.mms"
            Clock.bpm(120.0)
            T.position(0.0, -0.5, 0.0) { Zone.cube([20.0, 0.5, 20.0]) { Collidable.static() {} } }
            let chassis = T { name = "fixture_chassis" }
            chassis
            let target = practice_target({
                prefix = "fixture" avatar = "mitten" use_xr = false use_desktop = false
                position = [-0.33, 0.20, 0.30] yaw = 0.0 strafe = 0.0 seated = true
                vehicle = { chassis = chassis seat = [-0.33, 0.20, 0.30] door = [-1.95, 0.0, -0.25] }
            })
            chassis.attach(target.root)
            let controls = T { name = "fixture_seat_controls" }
            controls
            let truck = { x = 0.0 }
            on(controls, "DataEvent", fn(event) {
                for shot in range(10) {
                    target.pistol_hit({ renderable = { player_hit_zone = target.npc.hit_zone } })
                }
                truck.x = truck.x + 3.0
                chassis.update_transform([truck.x, 0.0, 0.0])
            })
            "#,
            "examples/_mitten_seat_test.mms",
        )
        .unwrap();
    let clock = Arc::new(TestClock::default());
    scene.systems.clock.set_driver(clock.clone());
    advance(&mut scene, &clock, 8);

    let chassis = named(&scene, "fixture_chassis");
    let frame = named(&scene, "fixture_frame");
    let controls = named(&scene, "fixture_seat_controls");
    let joints = [
        "J_Bip_L_UpperLeg",
        "J_Bip_L_LowerLeg",
        "J_Bip_L_Foot",
        "J_Bip_R_UpperLeg",
        "J_Bip_R_LowerLeg",
        "J_Bip_R_Foot",
    ];
    let leg_pose = |scene: &Universe| {
        joints.map(|joint| {
            scene
                .world
                .get_component_by_id_as::<TransformComponent>(named(scene, joint))
                .unwrap()
                .transform
                .rotation
        })
    };
    let matches = |a: [f32; 4], b: [f32; 4]| {
        a.into_iter().zip(b).map(|(a, b)| a * b).sum::<f32>().abs() > 0.999
    };
    let seated = leg_pose(&scene);
    assert_eq!(scene.world.parent_of(frame), Some(chassis));
    for cycle in 1..=2 {
        scene.command_queue.push_event(
            controls,
            EventSignal::DataEvent {
                name: "Down".into(),
                payload: None,
            },
        );
        advance(&mut scene, &clock, 30);
        assert_ne!(scene.world.parent_of(frame), Some(chassis));
        advance(&mut scene, &clock, 350);
        assert_ne!(
            scene.world.parent_of(frame),
            Some(chassis),
            "walking back to the door"
        );
        assert!(
            leg_pose(&scene)
                .into_iter()
                .zip(seated)
                .any(|(a, b)| !matches(a, b)),
            "cycle {cycle} should leave the seated pose while out of the cab"
        );
        advance(&mut scene, &clock, 100);
        assert_eq!(
            scene.world.parent_of(frame),
            Some(chassis),
            "reboard {cycle}"
        );
        for ((joint, a), b) in joints.into_iter().zip(leg_pose(&scene)).zip(seated) {
            assert!(
                matches(a, b),
                "{joint} should match the startup seat pose after reboard {cycle}"
            );
        }
    }
}
