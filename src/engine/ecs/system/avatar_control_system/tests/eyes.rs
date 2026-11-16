use super::*;

#[test]
fn direct_child_eye_trackers_choose_newest_source_per_eye() {
    let mut world = World::default();
    let avc = world.add_component(AvatarControlComponent::new());
    let standard = world.add_component(XREyeTrackingComponent::on());
    let htc = world.add_component(XREyeTrackingHtcComponent::on());
    let descendant = world.add_component(XREyeTrackingComponent::on());
    let _ = world.add_child(avc, standard);
    let _ = world.add_child(avc, htc);
    let _ = world.add_child(standard, descendant);
    world
        .get_component_by_id_as_mut::<XREyeTrackingComponent>(standard)
        .unwrap()
        .gaze_sample = EyeGazeSample {
        left: Some([1.0, 0.0, 0.0]),
        right: Some([0.0, 1.0, 0.0]),
        sequence: 2,
    };
    world
        .get_component_by_id_as_mut::<XREyeTrackingHtcComponent>(htc)
        .unwrap()
        .gaze_sample = EyeGazeSample {
        left: Some([0.0, 0.0, -1.0]),
        right: None,
        sequence: 3,
    };
    world
        .get_component_by_id_as_mut::<XREyeTrackingComponent>(descendant)
        .unwrap()
        .gaze_sample = EyeGazeSample {
        left: Some([0.0, -1.0, 0.0]),
        right: Some([0.0, -1.0, 0.0]),
        sequence: 99,
    };
    assert_eq!(
        newest_direct_eye_gaze(&mut world, avc),
        (
            Some(ResolvedEyeGaze {
                direction: [0.0, 0.0, -1.0],
                compensation: HeadRotationCompensation::Off,
                rotation_limits: None,
                sequence: 3,
            }),
            Some(ResolvedEyeGaze {
                direction: [0.0, 1.0, 0.0],
                compensation: HeadRotationCompensation::Off,
                rotation_limits: None,
                sequence: 2,
            }),
        )
    );
}

#[test]
fn disabled_newer_gaze_does_not_suppress_an_enabled_direct_tracker() {
    let mut world = World::default();
    let avc = world.add_component(AvatarControlComponent::new());
    let enabled = world
        .add_component(XREyeTrackingComponent::on().with_enable_pupil_direction_tracking(true));
    let disabled = world.add_component(
        XREyeTrackingHtcComponent::on().with_enable_pupil_direction_tracking(false),
    );
    world.add_child(avc, enabled).unwrap();
    world.add_child(avc, disabled).unwrap();
    world
        .get_component_by_id_as_mut::<XREyeTrackingComponent>(enabled)
        .unwrap()
        .gaze_sample = EyeGazeSample {
        left: Some([0.0, 0.0, -1.0]),
        right: Some([0.0, 0.0, -1.0]),
        sequence: 1,
    };
    world
        .get_component_by_id_as_mut::<XREyeTrackingHtcComponent>(disabled)
        .unwrap()
        .gaze_sample = EyeGazeSample {
        left: Some([1.0, 0.0, 0.0]),
        right: Some([1.0, 0.0, 0.0]),
        sequence: 2,
    };

    let (left, right) = newest_direct_eye_gaze(&mut world, avc);
    assert_eq!(left.unwrap().sequence, 1);
    assert_eq!(right.unwrap().sequence, 1);
}

#[test]
fn blink_routing_chooses_each_eye_and_drives_its_morph_independently() {
    let mut world = World::default();
    let avc = world.add_component(AvatarControlComponent::new());
    let standard = world.add_component(
        XREyeTrackingComponent::on().with_enable_pupil_direction_tracking(false),
    );
    let htc = world.add_component(
        XREyeTrackingHtcComponent::on().with_enable_pupil_direction_tracking(false),
    );
    let descendant = world.add_component(XREyeTrackingComponent::on());
    world.add_child(avc, standard).unwrap();
    world.add_child(avc, htc).unwrap();
    world.add_child(standard, descendant).unwrap();

    world
        .get_component_by_id_as_mut::<XREyeTrackingComponent>(standard)
        .unwrap()
        .closure_sample = EyeClosureSample {
        left: Some(0.2),
        right: Some(0.3),
        sequence: 2,
    };
    world
        .get_component_by_id_as_mut::<XREyeTrackingHtcComponent>(htc)
        .unwrap()
        .closure_sample = EyeClosureSample {
        left: Some(0.8),
        right: None,
        sequence: 3,
    };
    world
        .get_component_by_id_as_mut::<XREyeTrackingComponent>(descendant)
        .unwrap()
        .closure_sample = EyeClosureSample {
        left: Some(1.0),
        right: Some(1.0),
        sequence: 99,
    };

    let closure = newest_direct_eye_closure(&world, avc);
    assert_eq!(closure, (Some(0.8), Some(0.3)));

    let left_key = MorphTargetKey {
        node_index: 0,
        primitive_index: 0,
        target_index: 0,
    };
    let right_key = MorphTargetKey {
        target_index: 1,
        ..left_key
    };
    let mut gltf = GLTFComponent::new("test.glb");
    gltf.morph_targets = vec![
        MorphTargetInfo {
            key: left_key,
            label: Some("BlinkLeft".into()),
            base_factor: 0.1,
        },
        MorphTargetInfo {
            key: right_key,
            label: Some("BlinkRight".into()),
            base_factor: 0.15,
        },
    ];
    gltf.morph_factors.insert(
        left_key,
        MorphFactorState {
            external: None,
            script: None,
            idle_blink: None,
            base: 0.1,
            driver: None,
            amplitude_mouth_open: None,
        },
    );
    gltf.morph_factors.insert(
        right_key,
        MorphFactorState {
            external: None,
            script: None,
            idle_blink: None,
            base: 0.15,
            driver: None,
            amplitude_mouth_open: None,
        },
    );
    let gltf_id = world.add_component(gltf);
    let map = world.add_component(
        MorphTargetMapComponent::new()
            .with_slot("left_eye_blink", "BlinkLeft")
            .with_slot("right_eye_blink", "BlinkRight"),
    );
    world.add_child(gltf_id, map).unwrap();

    apply_eye_blink_drivers(&mut world, gltf_id, closure);
    let gltf = world
        .get_component_by_id_as::<GLTFComponent>(gltf_id)
        .unwrap();
    assert_eq!(gltf.morph_factors[&left_key].driver, Some(0.8));
    assert_eq!(gltf.morph_factors[&right_key].driver, Some(0.3));
    assert_eq!(gltf.morph_factors[&left_key].base, 0.1);
    assert_eq!(gltf.morph_factors[&right_key].base, 0.15);
    apply_eye_blink_drivers(&mut world, gltf_id, (None, None));
    let gltf = world
        .get_component_by_id_as::<GLTFComponent>(gltf_id)
        .unwrap();
    assert_eq!(gltf.morph_factors[&left_key].driver, None);
    assert_eq!(gltf.morph_factors[&right_key].driver, None);
    assert_eq!(gltf.morph_factors[&left_key].effective(), 0.1);
    assert_eq!(gltf.morph_factors[&right_key].effective(), 0.15);
}

#[test]
fn shared_rotation_limits_clamp_yaw_and_pitch() {
    let limits = EyeRotationLimits::from_array([0.2, 0.3, 0.4, 0.5]);
    let gaze = clamp_eye_gaze_rotation(ResolvedEyeGaze {
        direction: [0.8, 0.8, -1.0],
        compensation: HeadRotationCompensation::Off,
        rotation_limits: Some(limits),
        sequence: 1,
    });
    assert!((gaze.direction[0].atan2(-gaze.direction[2]) - 0.3).abs() < 1e-5);
    assert!(
        (gaze.direction[1]
            .atan2((gaze.direction[0].powi(2) + gaze.direction[2].powi(2)).sqrt())
            - 0.4)
            .abs()
            < 1e-5
    );
}

#[test]
fn per_eye_rotation_limits_are_independent() {
    let mut world = World::default();
    let avc = world.add_component(AvatarControlComponent::new());
    let tracker = world.add_component(
        XREyeTrackingComponent::on()
            .with_rotation_limits_per_eye([0.1, 0.2, 0.3, 0.4], [0.5, 0.6, 0.7, 0.8]),
    );
    world.add_child(avc, tracker).unwrap();
    world
        .get_component_by_id_as_mut::<XREyeTrackingComponent>(tracker)
        .unwrap()
        .gaze_sample = EyeGazeSample {
        left: Some([1.0, 0.0, -1.0]),
        right: Some([1.0, 0.0, -1.0]),
        sequence: 1,
    };
    let (left, right) = newest_direct_eye_gaze(&mut world, avc);
    let left = clamp_eye_gaze_rotation(left.unwrap());
    let right = clamp_eye_gaze_rotation(right.unwrap());
    assert!((left.direction[0].atan2(-left.direction[2]) - 0.2).abs() < 1e-5);
    assert!((right.direction[0].atan2(-right.direction[2]) - 0.6).abs() < 1e-5);
}

#[test]
fn shared_and_per_eye_rotation_limits_use_tighter_cap() {
    let gaze = clamp_eye_gaze_rotation(ResolvedEyeGaze {
        direction: [1.0, 0.0, -1.0],
        compensation: HeadRotationCompensation::Off,
        rotation_limits: crate::engine::ecs::component::combined_eye_rotation_limits(
            Some(EyeRotationLimits::from_array([0.5, 0.5, 0.5, 0.5])),
            Some(EyeRotationLimits::from_array([0.3, 0.2, 0.3, 0.3])),
        ),
        sequence: 1,
    });
    assert!((gaze.direction[0].atan2(-gaze.direction[2]) - 0.2).abs() < 1e-5);
}

#[test]
fn eye_tracking_is_rest_relative_and_restores_when_inactive() {
    let mut world = World::default();
    let avc = world.add_component(AvatarControlComponent::new());
    let eye = world.add_component(TransformComponent::new());
    let rest = [0.0, 0.5, 0.0, 0.866_025_4];
    let rest_pose = world.add_component(BoneRestPoseComponent::new([0.0; 3], rest, [1.0; 3]));
    let _ = world.add_child(eye, rest_pose);
    let mut emitted = RecordingEmitter::default();
    update_one_eye_tracking(
        avc,
        &mut world,
        &mut emitted,
        true,
        Some(eye),
        Some(ResolvedEyeGaze {
            direction: [1.0, 0.0, 0.0],
            compensation: HeadRotationCompensation::Off,
            rotation_limits: None,
            sequence: 1,
        }),
    );
    let expected = quat_mul(shortest_arc_quat([0.0, 0.0, -1.0], [1.0, 0.0, 0.0]), rest);
    assert_eq!(emitted.0, vec![expected]);
    update_one_eye_tracking(avc, &mut world, &mut emitted, true, Some(eye), None);
    assert_eq!(emitted.0.last(), Some(&rest));
}

#[test]
fn cancel_head_rotation_converts_world_gaze_to_eye_parent_basis() {
    let mut world = World::default();
    let parent = world.add_component(TransformComponent::new().with_rotation_quat([
        0.0,
        0.707_106_77,
        0.0,
        0.707_106_77,
    ]));
    let parent_transform = world
        .get_component_by_id_as_mut::<TransformComponent>(parent)
        .unwrap();
    parent_transform.transform.matrix_world = parent_transform.transform.model;
    let eye = world.add_component(TransformComponent::new());
    world.add_child(parent, eye).unwrap();

    let local = gaze_in_eye_parent_basis(
        &world,
        eye,
        ResolvedEyeGaze {
            direction: [1.0, 0.0, 0.0],
            compensation: HeadRotationCompensation::CancelHeadRotation,
            rotation_limits: None,
            sequence: 1,
        },
    );
    assert!((local[0]).abs() < 1e-5);
    assert!((local[1]).abs() < 1e-5);
    assert!((local[2] - 1.0).abs() < 1e-5);
}

#[test]
fn freeze_head_motion_policy_holds_last_gaze_during_a_rapid_turn() {
    let mut world = World::default();
    let avc = world.add_component(
        AvatarControlComponent::new()
            .with_head_motion_gaze_policy(HeadMotionGazePolicy::Freeze),
    );
    let parent = world.add_component(TransformComponent::new());
    let eye = world.add_component(TransformComponent::new());
    world.add_child(parent, eye).unwrap();
    let old = ResolvedEyeGaze {
        direction: [0.25, 0.0, -1.0],
        compensation: HeadRotationCompensation::Off,
        rotation_limits: None,
        sequence: 1,
    };
    assert_eq!(
        apply_head_motion_gaze_policy(
            avc,
            &mut world,
            Some(eye),
            None,
            Some(old),
            None,
            1.0 / 60.0,
        )
        .0,
        Some(old),
    );

    let rotated = TransformComponent::new()
        .with_rotation_quat([0.0, 0.707_106_77, 0.0, 0.707_106_77])
        .transform
        .model;
    world
        .get_component_by_id_as_mut::<TransformComponent>(parent)
        .unwrap()
        .transform
        .matrix_world = rotated;
    let incoming = ResolvedEyeGaze {
        direction: [-0.5, 0.0, -1.0],
        compensation: HeadRotationCompensation::Off,
        rotation_limits: None,
        sequence: 2,
    };
    let frozen = apply_head_motion_gaze_policy(
        avc,
        &mut world,
        Some(eye),
        None,
        Some(incoming),
        None,
        1.0 / 60.0,
    )
    .0
    .unwrap();
    assert_eq!(frozen.direction, old.direction);
    assert_eq!(frozen.compensation, HeadRotationCompensation::Off);
    assert_eq!(frozen.sequence, incoming.sequence);
}

#[test]
fn freeze_policy_holds_the_already_limited_gaze() {
    let mut world = World::default();
    let avc = world.add_component(
        AvatarControlComponent::new()
            .with_head_motion_gaze_policy(HeadMotionGazePolicy::Freeze),
    );
    let parent = world.add_component(TransformComponent::new());
    let eye = world.add_component(TransformComponent::new());
    world.add_child(parent, eye).unwrap();
    let limits = Some(EyeRotationLimits::from_array([0.2; 4]));
    let limited = clamp_eye_gaze_rotation(ResolvedEyeGaze {
        direction: [1.0, 0.0, -1.0],
        compensation: HeadRotationCompensation::Off,
        rotation_limits: limits,
        sequence: 1,
    });
    apply_head_motion_gaze_policy(
        avc,
        &mut world,
        Some(eye),
        None,
        Some(limited),
        None,
        1.0 / 60.0,
    );
    let rotated = TransformComponent::new()
        .with_rotation_quat([0.0, 0.707_106_77, 0.0, 0.707_106_77])
        .transform
        .model;
    world
        .get_component_by_id_as_mut::<TransformComponent>(parent)
        .unwrap()
        .transform
        .matrix_world = rotated;
    let incoming = clamp_eye_gaze_rotation(ResolvedEyeGaze {
        direction: [-1.0, 0.0, -1.0],
        compensation: HeadRotationCompensation::Off,
        rotation_limits: limits,
        sequence: 2,
    });
    let frozen = apply_head_motion_gaze_policy(
        avc,
        &mut world,
        Some(eye),
        None,
        Some(incoming),
        None,
        1.0 / 60.0,
    )
    .0
    .unwrap();
    assert_eq!(frozen.direction, limited.direction);
    assert!((frozen.direction[0].atan2(-frozen.direction[2]) - 0.2).abs() < 1e-5);
}
