use super::super::rig::uniform;
use super::*;

impl Fixture {
    fn walking(asset: Option<&str>, drop: f32) -> Self {
        let mut f = Self::new(asset);
        f.world
            .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
            .unwrap()
            .walking_enabled = AvatarControlComponent::new().walking_enabled;
        f.drop_body(drop);
        f
    }

    fn travel(&mut self, x: f32, z: f32, dt: f32) {
        for id in [self.root, self.capsule] {
            let t = &mut self
                .world
                .get_component_by_id_as_mut::<TransformComponent>(id)
                .unwrap()
                .transform;
            t.translation[0] += x;
            t.translation[2] += z;
            t.recompute_model();
        }
        self.propagate();
        self.step_with_dt(dt);
    }

    fn assert_contacts(&self) {
        self.assert_pose();
        assert!(
            self.feet().iter().all(|f| f.target_error < 0.0002),
            "{:?}",
            self.feet()
        );
        assert!(
            self.feet()
                .iter()
                .filter(|f| f.phase == FootPhase::Swing)
                .count()
                <= 1
        );
        for i in 0..2 {
            let foot = self.feet()[i];
            let rotation = uniform(matrix(&self.world, self.joints[i][2]).unwrap())
                .unwrap()
                .rotation_quat_xyzw;
            let dot = rotation
                .iter()
                .zip(foot.target_rotation)
                .map(|(a, b)| a * b)
                .sum::<f32>()
                .abs();
            assert!(dot > 0.99999);
            assert_eq!(foot.support, Some(self.floor_zone));
            assert!(foot.residual < 0.0002);
        }
    }
}

#[test]
fn planted_feet_hold_world_pose_while_body_translates_turns_and_crouches() {
    for asset in [
        None,
        Some("assets/models/rei(mu).glb"),
        Some("assets/models/bisket.glb"),
    ] {
        let mut f = Fixture::walking(asset, 0.2);
        let contacts = f.feet();
        let tracked = matrix(&f.world, f.driven).unwrap();
        f.travel(0.02, 0.02, 1.0 / 60.0);
        let t = &mut f
            .world
            .get_component_by_id_as_mut::<TransformComponent>(f.root)
            .unwrap()
            .transform;
        t.rotation = quat_rotation_y(0.1);
        t.recompute_model();
        f.drop_body(0.35);
        for _ in 0..20 {
            f.step();
        }
        f.assert_contacts();
        for (a, b) in f.feet().into_iter().zip(contacts) {
            assert_eq!(a.phase, FootPhase::Planted);
            close(a.target_sole, b.target_sole);
            close(a.solved_sole, b.solved_sole);
            assert!(a.knee_flexion > b.knee_flexion);
        }
        assert_eq!(matrix(&f.world, f.driven).unwrap(), tracked);
        assert_eq!(
            f.systems
                .lower_body_ik
                .walking(f.avatar)
                .unwrap()
                .steps_started,
            [0, 0]
        );
    }
}

#[test]
fn step_lifts_lands_and_finishes_after_stopping_without_idle_chatter() {
    let mut f = Fixture::walking(None, 0.2);
    f.travel(0.0, 0.12, 1.0 / 60.0);
    let side = f
        .systems
        .lower_body_ik
        .walking(f.avatar)
        .unwrap()
        .swing_side
        .expect("step must start");
    assert_eq!(f.feet()[side].phase, FootPhase::Swing);
    f.step_with_dt(0.055);
    f.assert_contacts();
    assert!(
        f.feet()[side].solved_sole[1] > 0.015,
        "swing was clamped to floor"
    );
    let body = matrix(&f.world, f.root).unwrap();
    for frame in 0..120 {
        f.travel(if frame % 2 == 0 { 0.002 } else { -0.002 }, 0.0, 1.0 / 60.0);
        f.assert_contacts();
    }
    assert!(f.feet().iter().all(|f| f.phase == FootPhase::Planted));
    let settled = f.feet();
    let count = f
        .systems
        .lower_body_ik
        .walking(f.avatar)
        .unwrap()
        .steps_started;
    assert!(count.iter().sum::<u64>() <= 6);
    for _ in 0..120 {
        f.step();
    }
    assert_eq!(
        f.systems
            .lower_body_ik
            .walking(f.avatar)
            .unwrap()
            .steps_started,
        count
    );
    for (a, b) in f.feet().into_iter().zip(settled) {
        close(a.solved_sole, b.solved_sole);
    }
    assert_eq!(matrix(&f.world, f.root).unwrap(), body);
}

#[test]
fn walking_anticipates_start_and_swings_forward_while_stance_holds() {
    for direction in [1.0, -1.0] {
        let mut f = Fixture::walking(None, 0.2);
        let planted = f.feet();
        let mut distance = 0.0;
        let side = loop {
            f.travel(0.0, direction * 0.01, 1.0 / 60.0);
            distance += 0.01;
            let gait = f.systems.lower_body_ik.walking(f.avatar).unwrap();
            if let Some(side) = gait.swing_side {
                break side;
            }
            assert!(distance < 0.1, "first step waited for feet to trail");
        };
        assert!(distance < 0.32 * 0.32);
        // Releasing movement during lift still carries the airborne foot
        // forward, while the other foot remains at its original contact.
        f.step_with_dt(0.06);
        f.assert_contacts();
        assert_eq!(f.feet()[side].phase, FootPhase::Swing);
        assert!(f.feet()[side].target_sole[1] > 0.01);
        assert!(direction * (f.feet()[side].target_sole[2] - planted[side].target_sole[2]) > 0.025);
        close(
            f.feet()[1 - side].target_sole,
            planted[1 - side].target_sole,
        );
    }
}

#[test]
fn walking_acceleration_advances_an_existing_swing_without_moving_stance() {
    let mut steady = Fixture::walking(None, 0.2);
    let mut accelerating = Fixture::walking(None, 0.2);
    let side = loop {
        for f in [&mut steady, &mut accelerating] {
            f.travel(0.0, 0.4 / 60.0, 1.0 / 60.0);
        }
        if let Some(side) = steady
            .systems
            .lower_body_ik
            .walking(steady.avatar)
            .unwrap()
            .swing_side
        {
            break side;
        }
    };
    let stance = accelerating.feet()[1 - side].target_sole;
    for _ in 0..3 {
        steady.travel(0.0, 0.4 / 60.0, 1.0 / 60.0);
        accelerating.travel(0.0, 1.8 / 60.0, 1.0 / 60.0);
        close(accelerating.feet()[1 - side].target_sole, stance);
    }
    let steady_gait = steady.systems.lower_body_ik.walking(steady.avatar).unwrap();
    let faster_gait = accelerating
        .systems
        .lower_body_ik
        .walking(accelerating.avatar)
        .unwrap();
    assert_eq!(faster_gait.swing_side, Some(side));
    assert_eq!(faster_gait.steps_started, steady_gait.steps_started);
    assert!(faster_gait.swing_progress.unwrap() > steady_gait.swing_progress.unwrap() + 0.05);
    assert!(accelerating.feet()[side].target_sole[2] > steady.feet()[side].target_sole[2]);
}

#[test]
fn walking_speed_increases_step_length_and_cadence_without_accumulating_lag() {
    for dt in [1.0 / 30.0, 1.0 / 60.0, 1.0 / 120.0] {
        for direction in [[0.0, 1.0], [0.0, -1.0], [0.6, 0.8]] {
            let mut samples = Vec::new();
            for speed in [0.25, 0.65, 1.2] {
                let mut f = Fixture::walking(None, 0.25);
                let mut contacts = f.feet();
                let mut counts = [0, 0];
                let mut landed = 0;
                let mut travel = 0.0;
                for _ in 0..(1.8 / dt) as usize {
                    f.travel(direction[0] * speed * dt, direction[1] * speed * dt, dt);
                    let feet = f.feet();
                    let gait = f.systems.lower_body_ik.walking(f.avatar).unwrap();
                    for side in 0..2 {
                        assert!(
                            gait.steps_started[side] >= counts[side],
                            "gait reset at {speed}"
                        );
                        let foot = feet[side];
                        let lag = (foot.base_sole[0] - foot.target_sole[0]) * direction[0]
                            + (foot.base_sole[2] - foot.target_sole[2]) * direction[1];
                        assert!(lag < 0.4, "feet trailed by {lag} at {speed}, dt={dt}");
                        if foot.phase == FootPhase::Planted {
                            let delta = vec3_sub(foot.target_sole, contacts[side].target_sole);
                            let length = (delta[0] * delta[0] + delta[2] * delta[2]).sqrt();
                            assert!(length <= 0.3201, "step exceeded configured stride");
                            if length > 0.001 {
                                travel += length;
                                landed += 1;
                                contacts[side] = foot;
                            }
                        }
                    }
                    counts = gait.steps_started;
                }
                assert!(landed >= 2);
                samples.push((counts.iter().sum::<u64>(), travel / landed as f32));
            }
            assert!(
                samples[1].0 > samples[0].0 && samples[2].0 > samples[1].0,
                "{samples:?}"
            );
            assert!(
                samples[1].1 > samples[0].1 && samples[2].1 > samples[1].1,
                "{samples:?}"
            );
        }
    }
}

#[test]
fn walking_gamepad_pace_keeps_up_across_headset_frame_rates_and_uneven_frames() {
    let schedules: &[&[f32]] = &[
        &[1.0 / 30.0],
        &[1.0 / 45.0],
        &[1.0 / 60.0],
        &[1.0 / 90.0],
        &[1.0 / 120.0],
        &[
            1.0 / 45.0,
            1.0 / 90.0,
            1.0 / 30.0,
            1.0 / 120.0,
            1.0 / 60.0,
            0.05,
        ],
    ];
    for asset in ["assets/models/rei(mu).glb", "assets/models/bisket.glb"] {
        for drop in [0.0, 0.2] {
            for direction in [[0.0, 1.0], [0.0, -1.0], [0.6, 0.8], [1.0, 0.0]] {
                let mut samples = Vec::new();
                for &schedule in schedules {
                    let mut f = Fixture::walking(Some(asset), drop);
                    f.world
                        .get_component_by_id_as_mut::<ZoneComponent>(f.floor_zone)
                        .unwrap()
                        .shape =
                        crate::engine::ecs::component::CollisionShape::cube_half_extents([
                            10.0, 0.05, 10.0,
                        ]);
                    let mut elapsed = 0.0_f32;
                    let mut frame = 0;
                    let mut counts = [0, 0];
                    let mut lag_sum = 0.0;
                    while elapsed < 3.0 {
                        let dt = schedule[frame % schedule.len()].min(3.0 - elapsed);
                        f.travel(direction[0] * 1.4 * dt, direction[1] * 1.4 * dt, dt);
                        f.assert_pose();
                        let gait = f.systems.lower_body_ik.walking(f.avatar).unwrap();
                        assert!(
                            gait.steps_started
                                .iter()
                                .zip(counts)
                                .all(|(n, old)| *n >= old),
                            "gait reset: {asset} drop={drop} direction={direction:?} schedule={schedule:?}"
                        );
                        counts = gait.steps_started;
                        let lag = f
                            .feet()
                            .iter()
                            .map(|foot| {
                                (foot.base_sole[0] - foot.target_sole[0]) * direction[0]
                                    + (foot.base_sole[2] - foot.target_sole[2]) * direction[1]
                            })
                            .fold(0.0_f32, f32::max);
                        assert!(
                            lag < 0.35,
                            "feet trailed by {lag}: {asset} drop={drop} direction={direction:?} schedule={schedule:?}"
                        );
                        if elapsed >= 1.0 {
                            lag_sum += lag * dt;
                        }
                        elapsed += dt;
                        frame += 1;
                    }
                    samples.push((counts.iter().sum::<u64>(), lag_sum / 2.0));
                }
                let min_steps = samples.iter().map(|s| s.0).min().unwrap();
                let max_steps = samples.iter().map(|s| s.0).max().unwrap();
                let min_lag = samples.iter().map(|s| s.1).fold(f32::INFINITY, f32::min);
                let max_lag = samples.iter().map(|s| s.1).fold(0.0_f32, f32::max);
                assert!(
                    max_steps - min_steps <= 2,
                    "cadence varies with frame rate: {samples:?}"
                );
                assert!(
                    max_lag - min_lag < 0.04,
                    "foot lag varies with frame rate: {samples:?}"
                );
            }
        }
    }
}

#[test]
fn walking_turn_in_place_steps_then_holds_the_new_heading() {
    let mut f = Fixture::walking(None, 0.2);
    let t = &mut f
        .world
        .get_component_by_id_as_mut::<TransformComponent>(f.root)
        .unwrap()
        .transform;
    t.rotation = quat_rotation_y(0.6);
    t.recompute_model();
    f.propagate();
    for _ in 0..90 {
        f.step();
        f.assert_contacts();
    }
    assert_eq!(
        f.systems
            .lower_body_ik
            .walking(f.avatar)
            .unwrap()
            .steps_started,
        [1, 1]
    );
    assert!(f.feet().iter().all(|f| f.phase == FootPhase::Planted));
    let goals = f.feet();
    for _ in 0..90 {
        f.step();
    }
    assert_eq!(
        f.systems
            .lower_body_ik
            .walking(f.avatar)
            .unwrap()
            .steps_started,
        [1, 1]
    );
    for (a, b) in f.feet().into_iter().zip(goals) {
        close(a.solved_sole, b.solved_sole);
    }
}

#[test]
fn walking_compatible_map_refresh_preserves_contacts_and_active_swing() {
    let mut f = Fixture::walking(Some("assets/models/bisket.glb"), 0.2);
    f.travel(0.0, 0.12, 1.0 / 60.0);
    f.step();
    let before = f.systems.lower_body_ik.walking(f.avatar).unwrap();
    assert!(before.swing_side.is_some());
    let contacts = f.feet();
    let gltf = f
        .world
        .all_components()
        .find(|&id| {
            f.world
                .get_component_by_id_as::<GLTFComponent>(id)
                .is_some()
        })
        .unwrap();
    for _ in 0..2 {
        f.systems
            .humanoid_bone_map
            .resolve_gltf(&mut f.world, gltf, true);
        let generation = f.systems.humanoid_bone_map.report(gltf).unwrap().generation;
        let config = f
            .world
            .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
            .unwrap();
        config.humanoid_map_gltf = Some(gltf);
        config.humanoid_map_generation = generation;
        f.step();
        f.assert_contacts();
    }
    let after = f.systems.lower_body_ik.walking(f.avatar).unwrap();
    assert_eq!(after.steps_started, before.steps_started);
    assert_eq!(after.swing_side, before.swing_side);
    assert!(after.swing_progress.unwrap() > before.swing_progress.unwrap());
    let stance = 1 - before.swing_side.unwrap();
    close(f.feet()[stance].target_sole, contacts[stance].target_sole);
}

#[test]
fn walking_alternates_and_preserves_stance_at_multiple_frame_rates_and_directions() {
    for dt in [1.0 / 30.0, 1.0 / 60.0, 1.0 / 120.0] {
        for direction in [[0.0, 1.0], [0.0, -1.0], [1.0, 0.0], [-1.0, 0.0], [0.6, 0.8]] {
            let mut f = Fixture::walking(None, 0.2);
            let mut previous = f.feet();
            let mut counts = [0, 0];
            let mut last_step = None;
            let ticks = (2.0 / dt) as usize;
            for _ in 0..ticks {
                f.travel(direction[0] * 0.45 * dt, direction[1] * 0.45 * dt, dt);
                f.assert_contacts();
                let next = f
                    .systems
                    .lower_body_ik
                    .walking(f.avatar)
                    .unwrap()
                    .steps_started;
                for side in 0..2 {
                    if next[side] > counts[side] {
                        assert_ne!(last_step, Some(side), "steps did not alternate at dt={dt}");
                        last_step = Some(side);
                    }
                    let current = f.feet()[side];
                    if current.phase == FootPhase::Planted
                        && previous[side].phase == FootPhase::Planted
                    {
                        close(current.target_sole, previous[side].target_sole);
                        close(current.solved_sole, previous[side].solved_sole);
                    }
                }
                counts = next;
                previous = f.feet();
            }
            assert!(
                counts.iter().all(|n| *n >= 2),
                "{direction:?} dt={dt}: {counts:?}"
            );
        }
    }
}

#[test]
fn walking_real_rigs_support_scale_yaw_and_reversal() {
    for asset in ["assets/models/rei(mu).glb", "assets/models/bisket.glb"] {
        let mut f = Fixture::new(Some(asset));
        let scene = &mut f
            .world
            .get_component_by_id_as_mut::<TransformComponent>(f.scene)
            .unwrap()
            .transform;
        scene.scale = [1.4; 3];
        scene.rotation = quat_rotation_y(0.6);
        scene.recompute_model();
        f.world
            .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
            .unwrap()
            .walking_enabled = true;
        f.drop_body(0.2);
        for direction in [1.0, -1.0, 1.0] {
            for _ in 0..100 {
                f.travel(0.0, direction * 0.006, 1.0 / 60.0);
                f.assert_contacts();
            }
        }
        let counts = f
            .systems
            .lower_body_ik
            .walking(f.avatar)
            .unwrap()
            .steps_started;
        assert!(counts.iter().all(|n| *n >= 2));
    }
}

#[test]
fn walking_releases_contacts_on_support_tracking_opt_out_and_teleport() {
    let mut f = Fixture::walking(None, 0.2);
    f.travel(0.0, 0.12, 1.0 / 60.0);
    f.world
        .get_component_by_id_as_mut::<CollidableComponent>(f.floor_collider)
        .unwrap()
        .enabled = false;
    f.step();
    assert_eq!(
        f.systems.lower_body_ik.status(f.avatar),
        LowerBodyStatus::NoSupport
    );
    assert!(f.feet().iter().all(|f| f.phase == FootPhase::Rest));
    assert_eq!(
        f.systems
            .lower_body_ik
            .walking(f.avatar)
            .unwrap()
            .steps_started,
        [0, 0]
    );
    f.world
        .get_component_by_id_as_mut::<CollidableComponent>(f.floor_collider)
        .unwrap()
        .enabled = true;
    f.step();
    f.assert_contacts();
    f.world
        .get_component_by_id_as_mut::<InputXRComponent>(f.input)
        .unwrap()
        .pose_valid = false;
    f.step();
    assert!(f.systems.lower_body_ik.feet(f.avatar).is_none());
    f.travel(0.2, 0.0, 1.0 / 60.0);
    f.world
        .get_component_by_id_as_mut::<InputXRComponent>(f.input)
        .unwrap()
        .pose_valid = true;
    f.step();
    f.assert_contacts();
    let before = f.feet();
    f.travel(1.5, 0.0, 1.0 / 60.0);
    f.assert_contacts();
    assert!(f.feet()[0].target_sole[0] - before[0].target_sole[0] > 1.4);
    assert_eq!(
        f.systems
            .lower_body_ik
            .walking(f.avatar)
            .unwrap()
            .steps_started,
        [0, 0]
    );
    f.world
        .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
        .unwrap()
        .walking_enabled = false;
    f.step();
    assert!(f.feet().iter().all(|f| f.phase == FootPhase::Rest));
    f.assert_pose();
}

#[test]
fn walking_handles_bad_frame_times_and_reports_unreachable_upright_targets() {
    let mut f = Fixture::walking(None, 0.0);
    f.travel(0.0, 0.15, 1.0 / 60.0);
    assert!(f.feet().iter().all(|f| f.target_error.is_finite()));
    assert!(f.feet().iter().any(|f| f.target_error > 0.001));
    assert_eq!(
        f.systems.lower_body_ik.status(f.avatar),
        LowerBodyStatus::Limited
    );
    // No stretching or tracked-origin adjustment to conceal a reach limit.
    f.assert_pose();
    for dt in [0.0, f32::NAN, 0.5] {
        f.step_with_dt(dt);
        assert!(f.feet().iter().all(|f| f.phase == FootPhase::Rest));
        f.assert_pose();
        f.step();
        assert!(f.feet().iter().all(|f| f.phase == FootPhase::Planted));
    }
}

#[test]
fn walking_mms_configuration_and_validation() {
    use crate::scripting::runner::RuntimeSpecSession;
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut queue = CommandQueue::new();
    let (_session, _) = RuntimeSpecSession::start(
        "AVC.walking_stride(0.25).walking_step_duration(0.3).walking_step_height(0.06)",
        &mut world,
        &mut rx,
        None,
        &mut queue,
    )
    .unwrap();
    let config = world
        .all_components()
        .find_map(|id| world.get_component_by_id_as::<AvatarControlComponent>(id))
        .unwrap();
    assert!(config.walking_enabled);
    assert_eq!(config.walking_stride, 0.25);
    assert_eq!(config.walking_step_duration, 0.3);
    assert_eq!(config.walking_step_height, 0.06);
    use crate::engine::ecs::component::Component;
    let saved = crate::scripting::unparser::unparse_component(&config.to_mms_ast(&world));
    assert!(
        !saved.contains("walking_enabled"),
        "default walking needs no authored flag"
    );
    let mut restored = World::default();
    let (_restored_session, _) =
        RuntimeSpecSession::start(&saved, &mut restored, &mut rx, None, &mut queue).unwrap();
    let got = restored
        .all_components()
        .find_map(|id| restored.get_component_by_id_as::<AvatarControlComponent>(id))
        .unwrap();
    assert!(got.walking_enabled);
    assert_eq!(
        (
            got.walking_stride,
            got.walking_step_duration,
            got.walking_step_height
        ),
        (0.25, 0.3, 0.06)
    );
    assert!(AvatarControlComponent::new().walking_enabled);
    let disabled = AvatarControlComponent::new().with_walking_enabled(false);
    let saved = crate::scripting::unparser::unparse_component(&disabled.to_mms_ast(&world));
    assert!(saved.contains("walking_enabled(false)"));
    let mut opted_out = World::default();
    let (_disabled_session, _) =
        RuntimeSpecSession::start(&saved, &mut opted_out, &mut rx, None, &mut queue).unwrap();
    assert!(
        !opted_out
            .all_components()
            .find_map(|id| opted_out.get_component_by_id_as::<AvatarControlComponent>(id))
            .unwrap()
            .walking_enabled
    );
    for invalid in [f32::NAN, f32::INFINITY, -0.1, 0.0, 2.0] {
        assert!(
            AvatarControlComponent::new()
                .with_walking_stride(invalid)
                .is_err()
        );
        assert!(
            AvatarControlComponent::new()
                .with_walking_step_duration(invalid)
                .is_err()
        );
    }
    for invalid in [f32::NAN, f32::INFINITY, -0.1, 0.4] {
        assert!(
            AvatarControlComponent::new()
                .with_walking_step_height(invalid)
                .is_err()
        );
    }
}

#[test]
fn walking_full_avatar_stage_keeps_contacts_through_map_refresh_and_head_follow() {
    for asset in ["assets/models/rei(mu).glb", "assets/models/bisket.glb"] {
        let mut f = Fixture::new(Some(asset));
        f.systems.register_avatar_control(f.avatar);
        f.world
            .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
            .unwrap()
            .walking_enabled = true;
        let mut last_counts = [0, 0];
        let mut previous: Option<[FootState; 2]> = None;
        for frame in 0..160 {
            let x = frame as f32 * 0.004;
            for (id, height) in [(f.driven, 1.35), (f.capsule, 1.0 + f.base_height)] {
                let t = &mut f
                    .world
                    .get_component_by_id_as_mut::<TransformComponent>(id)
                    .unwrap()
                    .transform;
                t.translation[0] = x;
                t.translation[1] = height;
                t.recompute_model();
            }
            f.propagate();
            let tracked = matrix(&f.world, f.driven).unwrap();
            let capsule = matrix(&f.world, f.capsule).unwrap();
            f.systems.tick(
                &mut f.world,
                &mut f.visuals,
                &mut f.assets,
                &Default::default(),
                &mut f.queue,
                1.0 / 60.0,
            );
            f.assert_contacts();
            assert_eq!(matrix(&f.world, f.driven).unwrap(), tracked);
            assert_eq!(matrix(&f.world, f.capsule).unwrap(), capsule);
            let counts = f
                .systems
                .lower_body_ik
                .walking(f.avatar)
                .unwrap()
                .steps_started;
            assert!(
                counts.iter().zip(last_counts).all(|(a, b)| *a >= b),
                "gait reset on map refresh"
            );
            if let Some(before) = previous {
                for (a, b) in f.feet().into_iter().zip(before) {
                    if a.phase == FootPhase::Planted && b.phase == FootPhase::Planted {
                        close(a.solved_sole, b.solved_sole);
                    }
                }
            }
            last_counts = counts;
            previous = Some(f.feet());
        }
        assert!(
            last_counts.iter().all(|n| *n >= 2),
            "{asset}: {last_counts:?}"
        );
    }
}
