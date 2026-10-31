use super::*;
use crate::engine::ecs::component::{Component, WalkingIkComponent};
use crate::scripting::runner::RuntimeSpecSession;

fn profile(asset: Option<&str>, step_scale: Option<f32>) -> (f32, f32, f32) {
    let mut f = Fixture::walking(asset, 0.2);
    if let Some(scale) = step_scale {
        let config = f
            .world
            .add_component(WalkingIkComponent::new().with_step_scale(scale).unwrap());
        f.world.add_child(f.avatar, config).unwrap();
    }
    f.travel(0.0, 0.12, 1.0 / 60.0);
    let side = f
        .systems
        .lower_body_ik
        .walking(f.avatar)
        .unwrap()
        .swing_side
        .unwrap();
    let from = f.feet()[side].target_sole;
    let stance = f.feet()[1 - side].target_sole;
    let mut peak_height = 0.0_f32;
    let mut forward_travel = 0.0_f32;
    let mut thigh_angle = 0.0_f32;
    for _ in 0..100 {
        f.step_with_dt(0.005);
        f.assert_contacts();
        let foot = f.feet()[side];
        peak_height = peak_height.max(foot.target_sole[1] - f.base_height);
        forward_travel = forward_travel.max(foot.target_sole[2] - from[2]);
        let hip = matrix(&f.world, f.joints[side][0]).unwrap()[3];
        let knee = matrix(&f.world, f.joints[side][1]).unwrap()[3];
        thigh_angle = thigh_angle.max((knee[2] - hip[2]).atan2(hip[1] - knee[1]));
        if foot.phase != FootPhase::Swing {
            return (peak_height, forward_travel, thigh_angle);
        }
        close(f.feet()[1 - side].target_sole, stance);
    }
    panic!("step did not finish");
}

#[test]
fn walking_step_scale_increases_reach_clearance_and_forward_knee_swing() {
    for asset in [
        None,
        Some("assets/models/mitten.glb"),
        Some("assets/models/bisket.glb"),
    ] {
        let base = profile(asset, Some(1.0));
        let larger = profile(asset, Some(1.3));
        let automatic = profile(asset, None);
        assert!(
            (larger.0 / base.0 - 1.3).abs() < 0.01,
            "clearance: {base:?} -> {larger:?}"
        );
        assert!(
            (larger.1 / base.1 - 1.3).abs() < 0.01,
            "reach: {base:?} -> {larger:?}"
        );
        assert!(
            larger.2 > base.2 + 0.02,
            "knee swing: {base:?} -> {larger:?}"
        );
        assert_eq!(
            automatic, larger,
            "default tuning must apply without adding a component"
        );
    }
}

#[test]
fn walking_component_mms_round_trip_and_validation() {
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut queue = CommandQueue::new();
    let (_session, _) = RuntimeSpecSession::start(
        "AVC.walking_stride(0.9) { WalkingIK.stride(0.4) { enabled(false) step_duration(0.3) step_height(0.08) step_scale(1.3) } }",
        &mut world, &mut rx, None, &mut queue,
    ).unwrap();
    let avatar = world
        .all_components()
        .find(|&id| {
            world
                .get_component_by_id_as::<AvatarControlComponent>(id)
                .is_some()
        })
        .unwrap();
    let config_id = world.children_of(avatar)[0];
    let expected = *world
        .get_component_by_id_as::<WalkingIkComponent>(config_id)
        .unwrap();
    assert_eq!(
        expected,
        WalkingIkComponent {
            enabled: false,
            stride: 0.4,
            step_duration: 0.3,
            step_height: 0.08,
            step_scale: 1.3,
        }
    );
    let tree = crate::scripting::component_registry::subtree_to_ce_ast(&world, avatar).unwrap();
    let saved = crate::scripting::unparser::unparse_component(&tree);
    let mut restored = World::default();
    let (_restored, _) =
        RuntimeSpecSession::start(&saved, &mut restored, &mut rx, None, &mut queue).unwrap();
    let restored_avatar = restored
        .all_components()
        .find(|&id| {
            restored
                .get_component_by_id_as::<AvatarControlComponent>(id)
                .is_some()
        })
        .unwrap();
    let restored_config = restored
        .children_of(restored_avatar)
        .iter()
        .find_map(|&id| restored.get_component_by_id_as::<WalkingIkComponent>(id))
        .unwrap();
    assert_eq!(*restored_config, expected);
    for invalid in [
        "stride(0)",
        "stride(2)",
        "step_duration(0.01)",
        "step_height(-0.1)",
        "step_scale(0)",
        "step_scale(3)",
        "unknown(1)",
    ] {
        let source = format!("WalkingIK {{ {invalid} }}");
        assert!(
            RuntimeSpecSession::start(
                &source,
                &mut World::default(),
                &mut RxWorld::default(),
                None,
                &mut CommandQueue::new()
            )
            .is_err(),
            "{source}"
        );
    }
    for invalid in [f32::NAN, f32::INFINITY] {
        assert!(WalkingIkComponent::new().with_stride(invalid).is_err());
        assert!(
            WalkingIkComponent::new()
                .with_step_duration(invalid)
                .is_err()
        );
        assert!(WalkingIkComponent::new().with_step_height(invalid).is_err());
        assert!(WalkingIkComponent::new().with_step_scale(invalid).is_err());
    }
    let saved_default = crate::scripting::unparser::unparse_component(
        &WalkingIkComponent::default().to_mms_ast(&world),
    );
    assert!(saved_default.contains("step_scale"));
}

#[test]
fn walking_component_scope_precedence_opt_out_and_invalid_runtime_settings() {
    let mut f = Fixture::walking(None, 0.2);
    let nested = f
        .world
        .add_component(WalkingIkComponent::new().with_enabled(false));
    f.world.add_child(f.root, nested).unwrap();
    f.travel(0.0, 0.12, 1.0 / 60.0);
    assert!(
        f.feet().iter().any(|foot| foot.phase == FootPhase::Swing),
        "nested settings must not configure the AVC"
    );
    let settings = f
        .world
        .add_component(WalkingIkComponent::new().with_enabled(false));
    f.world.add_child(f.avatar, settings).unwrap();
    f.step();
    assert!(f.feet().iter().all(|foot| foot.phase == FootPhase::Rest));
    assert!(
        f.feet().iter().all(|foot| foot.knee_flexion > 0.1),
        "crouch remains active"
    );
    f.world
        .get_component_by_id_as_mut::<WalkingIkComponent>(settings)
        .unwrap()
        .enabled = true;
    // Child tuning replaces the old AVC distances, including invalid old data.
    f.world
        .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
        .unwrap()
        .walking_stride = f32::NAN;
    f.step();
    f.travel(0.0, 0.12, 1.0 / 60.0);
    assert!(f.feet().iter().any(|foot| foot.phase == FootPhase::Swing));
    f.world
        .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
        .unwrap()
        .walking_enabled = false;
    f.step();
    assert!(f.feet().iter().all(|foot| foot.phase == FootPhase::Rest));
    f.world
        .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
        .unwrap()
        .walking_enabled = true;
    f.world
        .get_component_by_id_as_mut::<WalkingIkComponent>(settings)
        .unwrap()
        .step_scale = f32::NAN;
    f.step();
    assert_eq!(
        f.systems.lower_body_ik.status(f.avatar),
        LowerBodyStatus::UnsupportedRig("invalid walking configuration")
    );
    f.world
        .get_component_by_id_as_mut::<WalkingIkComponent>(settings)
        .unwrap()
        .step_scale = 1.3;
    let duplicate = f.world.add_component(WalkingIkComponent::new());
    f.world.add_child(f.avatar, duplicate).unwrap();
    f.step();
    assert_eq!(
        f.systems.lower_body_ik.status(f.avatar),
        LowerBodyStatus::UnsupportedRig("multiple WalkingIK components on one AVC")
    );
}
