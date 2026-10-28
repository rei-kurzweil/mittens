use crate::engine::Universe;
use crate::engine::ecs::component::{TextComponent, TransformComponent, VelocityComponent};
use crate::engine::ecs::{ComponentId, EventSignal, World};
use crate::utils::math::{quat_rotate_vec3, vec3_len, vec3_sub};

fn fixture() -> Universe {
    let mut scene = Universe::new(World::default());
    scene.load_mms_source_at_path(r#"
        import { pacing_and_waiting_around } from "../assets/components/characters/lumae/behavior/pacing_and_waiting_around.mms"
        let body = T.position(0, 2, 0).scale(0.8, 0.8, 0.8) { name = "pacing_body" }
        let controller = pacing_and_waiting_around(body, {
            speed = 0.4 wait_seconds = 0.2 turn_seconds = 0.5 walk_seconds = 1.0 ease_seconds = 0.2
        })
        T.position(3, 0, 5).rotation(0, 1.57079632679, 0) { controller }
        let status = Text { name = "pacing_status" "waiting" }
        status
        on_global("FrameTick", fn(event) { status.set_text(controller.status().phase) })
        on_global("DataEvent", fn(event) {
            if event == "Start" { controller.start() }
            if event == "Stop" { controller.stop() }
        })
    "#, "examples/_lumae-pacing-test.mms").unwrap();
    scene.update(0.0, &Default::default());
    scene
}

fn named(scene: &Universe, name: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(name))
        .unwrap()
}
fn motion(scene: &Universe) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| {
            scene
                .world
                .get_component_by_id_as::<VelocityComponent>(id)
                .is_some()
        })
        .unwrap()
}
fn position(scene: &Universe) -> [f32; 3] {
    scene
        .world
        .get_component_by_id_as::<TransformComponent>(named(scene, "pacing_body"))
        .unwrap()
        .transform
        .matrix_world[3][..3]
        .try_into()
        .unwrap()
}
fn phase(scene: &Universe) -> &str {
    &scene
        .world
        .get_component_by_id_as::<TextComponent>(named(scene, "pacing_status"))
        .unwrap()
        .text
}
fn event(scene: &mut Universe, name: &str) {
    scene.systems.rx.push_event(
        ComponentId::default(),
        EventSignal::DataEvent {
            name: name.into(),
            payload: None,
        },
    );
    scene.update(0.0, &Default::default());
}
fn tick(scene: &mut Universe, dt: f32) {
    scene.update(dt, &Default::default());
}

#[test]
fn lumae_pacing_waits_turns_walks_and_returns_without_accumulating_speed() {
    let mut scene = fixture();
    let initial = position(&scene);
    for _ in 0..120 {
        tick(&mut scene, 1.0 / 60.0);
    }
    assert_eq!(
        position(&scene),
        initial,
        "constructing must not start motion"
    );
    event(&mut scene, "Start");
    for _ in 0..9 {
        tick(&mut scene, 1.0 / 60.0);
    }
    assert_eq!(phase(&scene), "waiting");
    assert_eq!(position(&scene), initial);
    let mut saw_turn = false;
    let mut saw_walk = false;
    let mut min_x = initial[0];
    for _ in 0..210 {
        tick(&mut scene, 1.0 / 60.0);
        saw_turn |= phase(&scene) == "turning";
        saw_walk |= phase(&scene) == "walking";
        min_x = min_x.min(position(&scene)[0]);
        let velocity = scene
            .world
            .get_component_by_id_as::<VelocityComponent>(motion(&scene))
            .unwrap();
        assert!(
            vec3_len(velocity.linear_local_mps) <= 0.40001,
            "velocity accumulated: {velocity:?}"
        );
        assert!((position(&scene)[1] - initial[1]).abs() < 1e-5);
    }
    assert!(saw_turn && saw_walk);
    assert!(
        min_x < initial[0] - 0.25,
        "must walk along turned forward under a rotated parent"
    );
    assert!(
        (position(&scene)[0] - initial[0]).abs() < 0.03,
        "alternating half-turns should pace back: {:?}",
        position(&scene)
    );
    assert!((position(&scene)[2] - initial[2]).abs() < 0.02);
    let body = scene
        .world
        .get_component_by_id_as::<TransformComponent>(named(&scene, "pacing_body"))
        .unwrap();
    assert_eq!(body.transform.scale, [0.8; 3]);
}

#[test]
fn lumae_pacing_stop_preserves_vertical_velocity_and_root_pose() {
    let mut scene = fixture();
    event(&mut scene, "Start");
    for _ in 0..70 {
        tick(&mut scene, 1.0 / 60.0);
    }
    assert_eq!(phase(&scene), "walking");
    let id = motion(&scene);
    scene
        .world
        .get_component_by_id_as_mut::<VelocityComponent>(id)
        .unwrap()
        .linear_local_mps[1] = -0.5;
    let body = named(&scene, "pacing_body");
    let before = scene
        .world
        .get_component_by_id_as::<TransformComponent>(body)
        .unwrap()
        .trs();
    event(&mut scene, "Stop");
    let velocity = scene
        .world
        .get_component_by_id_as::<VelocityComponent>(id)
        .unwrap();
    assert!(velocity.linear_local_mps[0].abs() < 1e-5 && velocity.linear_local_mps[2].abs() < 1e-5);
    assert_eq!(velocity.linear_local_mps[1], -0.5);
    assert_eq!(velocity.angular_local_radps, [0.0; 3]);
    let after = scene
        .world
        .get_component_by_id_as::<TransformComponent>(body)
        .unwrap()
        .trs();
    assert_eq!(before, after, "stopping must not snap/reset the root pose");
    let stopped = position(&scene);
    for _ in 0..30 {
        tick(&mut scene, 1.0 / 60.0);
    }
    let end = position(&scene);
    assert!((end[0] - stopped[0]).abs() < 1e-5 && (end[2] - stopped[2]).abs() < 1e-5);
    assert!(end[1] < stopped[1] - 0.24, "vertical motion remains active");
}

#[test]
fn lumae_pacing_turn_is_smooth_and_consistent_at_30_60_and_120_hz() {
    for hz in [30, 60, 120] {
        let dt = 1.0 / hz as f32;
        let mut scene = fixture();
        event(&mut scene, "Start");
        while phase(&scene) == "waiting" {
            tick(&mut scene, dt);
        }
        let mut rates = Vec::new();
        let mut turn_frames = 0;
        while phase(&scene) == "turning" {
            tick(&mut scene, dt);
            rates.push(
                scene
                    .world
                    .get_component_by_id_as::<VelocityComponent>(motion(&scene))
                    .unwrap()
                    .angular_local_radps[1],
            );
            turn_frames += 1;
            assert!(turn_frames < hz, "turn did not complete at {hz} Hz");
        }
        tick(&mut scene, dt); // Integrate the last queued rate; MMS runs after native update.
        let rotation = scene
            .world
            .get_component_by_id_as::<TransformComponent>(named(&scene, "pacing_body"))
            .unwrap()
            .transform
            .rotation;
        let forward = quat_rotate_vec3(rotation, [0.0, 0.0, 1.0]);
        assert!(
            vec3_len(vec3_sub(forward, [0.0, 0.0, -1.0])) < 0.002,
            "turn drift at {hz} Hz: {forward:?}"
        );
        let peak = rates.iter().copied().fold(0.0, f32::max);
        assert!(peak > 8.0 && peak < 12.0);
        assert!(rates[0] < peak * 0.1, "turn must ease in");
        assert!(
            rates.last().unwrap().abs() < peak * 0.1,
            "turn must ease out"
        );
    }
}

#[test]
fn lumae_pacing_repeated_start_and_hitch_do_not_restart_or_fast_forward_a_turn() {
    let mut scene = fixture();
    event(&mut scene, "Start");
    while phase(&scene) != "turning" {
        tick(&mut scene, 1.0 / 60.0);
    }
    for _ in 0..5 {
        tick(&mut scene, 1.0 / 60.0);
    }
    event(&mut scene, "Start");
    assert_eq!(phase(&scene), "turning", "start should be idempotent");
    tick(&mut scene, 0.5);
    assert_eq!(
        phase(&scene),
        "turning",
        "hitch must not fast-forward the schedule"
    );
    assert_eq!(
        scene
            .world
            .get_component_by_id_as::<VelocityComponent>(motion(&scene))
            .unwrap()
            .angular_local_radps,
        [0.0; 3]
    );
    for _ in 0..30 {
        tick(&mut scene, 1.0 / 60.0);
    }
    assert_eq!(phase(&scene), "walking");
}
