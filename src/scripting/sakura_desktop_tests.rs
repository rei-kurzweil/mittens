use crate::engine::Universe;
use crate::engine::ecs::component::{
    AvatarControlComponent, Camera3DComponent, CameraXRComponent, InputXRComponent,
    TransformComponent,
};
use crate::engine::ecs::{ComponentId, World};

fn named(scene: &Universe, name: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(name))
        .unwrap()
}

#[test]
fn sakura_desktop_first_person_avatar_initializes_without_xr() {
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(
            include_str!("../../examples/sakura-drone-conversion-desktop.mms"),
            "examples/sakura-drone-conversion-desktop.mms",
        )
        .unwrap();
    assert_eq!(
        scene
            .world
            .all_components()
            .filter(|&id| scene
                .world
                .get_component_by_id_as::<Camera3DComponent>(id)
                .is_some())
            .count(),
        1
    );
    assert!(!scene.world.all_components().any(|id| {
        scene
            .world
            .get_component_by_id_as::<CameraXRComponent>(id)
            .is_some()
            || scene
                .world
                .get_component_by_id_as::<InputXRComponent>(id)
                .is_some()
    }));
    for _ in 0..12 {
        scene.update(1.0 / 60.0, &Default::default());
    }
    let avatar = scene
        .world
        .get_component_by_id_as::<AvatarControlComponent>(named(
            &scene,
            "sakura_desktop_avatar_control",
        ))
        .unwrap();
    assert!(avatar.model_root_id.is_some());
    assert!(avatar.capsule_ready);
    let slot = named(&scene, "sakura_desktop_camera");
    let head = avatar.displaced_head.expect("AVC head binding");
    assert_eq!(
        scene.world.parent_of(slot),
        avatar.splice_camera_bone,
        "camera must use the calibrated mapped eye anchor"
    );
    assert_eq!(scene.world.component_label(head), Some("J_Bip_C_Head"));
    assert_eq!(
        scene.world.parent_of(head),
        avatar.head_mount,
        "desktop and XR must share the head splice"
    );
    let head_parent = scene.world.parent_of(head).unwrap();
    assert_eq!(
        scene
            .world
            .get_component_by_id_as::<TransformComponent>(head)
            .unwrap()
            .transform
            .translation,
        [0.0; 3]
    );

    assert_eq!(
        scene.world.parent_of(named(&scene, "sakura_desktop_c3d")),
        Some(slot)
    );
    let camera = scene
        .world
        .get_component_by_id_as::<TransformComponent>(named(&scene, "sakura_desktop_camera"))
        .unwrap()
        .transform
        .matrix_world;
    assert!(camera.iter().flatten().all(|v| v.is_finite()));
    assert!(
        (camera[3][0] + 18.0).abs() < 0.1 && (camera[3][2] + 1.4).abs() < 0.3,
        "camera wandered: {:?}",
        camera[3]
    );
    assert!(
        (26.5..28.0).contains(&camera[3][1]),
        "camera height: {:?}",
        camera[3]
    );
    assert!(
        scene
            .world
            .component_label(named(&scene, "sakura_machine_pistol"))
            .is_some()
    );
    // Real-scene look-down regression: camera rotation alone is insufficient.
    // Check actual neck flexion, geometric closure, return to neutral and drift.
    use crate::engine::ecs::component::{BoneRestPoseComponent, HumanoidSlot};
    use crate::engine::ecs::system::ik::head_neck_body_constraint::HeadNeckBodyConstraintStatus;
    use crate::engine::user_input::{Key, NamedKey};
    let avc = named(&scene, "sakura_desktop_avatar_control");
    let avatar = scene
        .world
        .get_component_by_id_as::<AvatarControlComponent>(avc)
        .unwrap();
    let root = avatar.model_root_id.unwrap();
    let report = scene
        .systems
        .humanoid_bone_map
        .report(avatar.humanoid_map_gltf.unwrap())
        .unwrap();
    let neck = report.target(HumanoidSlot::Neck).unwrap().component;
    let neck_rest = *scene
        .world
        .children_of(neck)
        .iter()
        .find_map(|&id| {
            scene
                .world
                .get_component_by_id_as::<BoneRestPoseComponent>(id)
        })
        .unwrap();
    let head_rest = *scene
        .world
        .children_of(head)
        .iter()
        .find_map(|&id| {
            scene
                .world
                .get_component_by_id_as::<BoneRestPoseComponent>(id)
        })
        .unwrap();
    let mut down = crate::engine::user_input::InputState::default();
    down.keys_down.insert(Key::Named(NamedKey::ArrowDown));
    down.keys_pressed.insert(Key::Named(NamedKey::ArrowDown));
    let root_before = scene
        .world
        .get_component_by_id_as::<TransformComponent>(root)
        .unwrap()
        .transform
        .matrix_world[3];
    let head_before = scene
        .world
        .get_component_by_id_as::<TransformComponent>(head)
        .unwrap()
        .transform
        .matrix_world[3];
    for _ in 0..60 {
        scene.update(1.0 / 60.0, &down);
        down.keys_pressed.clear();
    }
    assert_eq!(
        scene.systems.head_neck_body_constraint.status(avc),
        HeadNeckBodyConstraintStatus::Ready
    );
    let neck_pose = &scene
        .world
        .get_component_by_id_as::<TransformComponent>(neck)
        .unwrap()
        .transform;
    let dot = neck_pose
        .rotation
        .iter()
        .zip(neck_rest.rotation)
        .map(|(a, b)| a * b)
        .sum::<f32>()
        .abs()
        .clamp(0.0, 1.0);
    let bend = 2.0 * dot.acos();
    assert!(
        bend > 0.1 && bend < 0.46,
        "look-down must bend the neck within its limits: {bend}"
    );
    let expected = crate::utils::math::mat4_mul_vec4(
        neck_pose.matrix_world,
        [
            head_rest.translation[0],
            head_rest.translation[1],
            head_rest.translation[2],
            1.0,
        ],
    );
    let head_world = scene
        .world
        .get_component_by_id_as::<TransformComponent>(head)
        .unwrap()
        .transform
        .matrix_world;
    for axis in 0..3 {
        assert!(
            (expected[axis] - head_world[3][axis]).abs() < 0.0001,
            "neck/head disconnected"
        );
    }
    let root_after = scene
        .world
        .get_component_by_id_as::<TransformComponent>(root)
        .unwrap()
        .transform
        .matrix_world[3];
    assert!(
        (root_before[0] - root_after[0]).abs() < 0.001
            && (root_before[2] - root_after[2]).abs() < 0.001,
        "look alone moved the body anchor"
    );
    let relative_shift: [f32; 3] = std::array::from_fn(|axis| {
        (head_world[3][axis] - root_after[axis]) - (head_before[axis] - root_before[axis])
    });
    assert!(
        crate::utils::math::vec3_len(relative_shift) > 0.01,
        "head must translate independently of the body on pitch"
    );
    let camera_down = scene
        .world
        .get_component_by_id_as::<TransformComponent>(named(&scene, "sakura_desktop_camera"))
        .unwrap()
        .transform
        .matrix_world;
    assert!(-camera_down[2][1] < -0.4, "C3D must actually look down");
    // Probe actual skinned shirt/body surfaces through the authored C3D,
    // including the near clip plane; endpoint closure alone misses this bug.
    use crate::engine::ecs::component::GLTFComponent;
    use crate::engine::ecs::system::avatar_control_system::first_person_test_support::body_hit;
    let gltf = scene
        .world
        .get_component_by_id_as::<GLTFComponent>(named(&scene, "sakura_desktop_mitten"))
        .unwrap();
    let document = gltf::Gltf::open(&gltf.uri).unwrap();
    let nodes: Vec<_> = document
        .nodes()
        .map(|node| {
            gltf.spawned_node_transforms
                .iter()
                .copied()
                .find(|&id| scene.world.component_label(id) == node.name())
                .unwrap_or_default()
        })
        .collect();
    let near = scene
        .world
        .all_components()
        .find_map(|id| scene.world.get_component_by_id_as::<Camera3DComponent>(id))
        .unwrap()
        .z_near;
    let mut hits = 0;
    for y in [0.0, -0.2, -0.4, -0.6] {
        for x in [-0.1, 0.0, 0.1] {
            if let Some((distance, facing)) =
                body_hit(&document, &scene.world, &nodes, camera_down, [x, y, -1.0])
            {
                hits += 1;
                assert!(
                    facing < 0.0,
                    "first-person camera is inside a body surface: distance={distance}, facing={facing}"
                );
                assert!(
                    distance >= near,
                    "shirt/chest surface is clipped: distance={distance}, near={}",
                    near
                );
            }
        }
    }
    assert!(hits > 0, "look-down probe must exercise the real body mesh");
    let driver = named(&scene, "sakura_desktop_driver");
    let base_rotation = scene
        .world
        .get_component_by_id_as::<TransformComponent>(driver)
        .unwrap()
        .transform
        .rotation;
    for roll in [-35.0_f32, 35.0] {
        let rotation = crate::utils::math::quat_mul(
            base_rotation,
            crate::utils::math::quat_from_axis_angle([0.0, 0.0, -1.0], roll.to_radians()),
        );
        let t = &mut scene
            .world
            .get_component_by_id_as_mut::<TransformComponent>(driver)
            .unwrap()
            .transform;
        t.rotation = rotation;
        t.recompute_model();
        scene
            .systems
            .transform_changed(&mut scene.world, &mut scene.visuals, driver);
        scene.update(1.0 / 60.0, &Default::default());
        let view = scene
            .world
            .get_component_by_id_as::<TransformComponent>(named(&scene, "sakura_desktop_camera"))
            .unwrap()
            .transform
            .matrix_world;
        let mut roll_hits = 0;
        for y in [0.0, -0.2, -0.4, -0.6] {
            for x in [-0.1, 0.0, 0.1] {
                if let Some((distance, facing)) =
                    body_hit(&document, &scene.world, &nodes, view, [x, y, -1.0])
                {
                    roll_hits += 1;
                    assert!(
                        facing < 0.0 && distance >= near,
                        "pitch/roll body view invalid: roll={roll}, distance={distance}, facing={facing}, near={near}"
                    );
                }
            }
        }
        assert!(roll_hits > 0, "roll probe must hit body surfaces");
    }
    let t = &mut scene
        .world
        .get_component_by_id_as_mut::<TransformComponent>(driver)
        .unwrap()
        .transform;
    t.rotation = base_rotation;
    t.recompute_model();
    scene
        .systems
        .transform_changed(&mut scene.world, &mut scene.visuals, driver);
    let mut up = crate::engine::user_input::InputState::default();
    up.keys_down.insert(Key::Named(NamedKey::ArrowUp));
    up.keys_pressed.insert(Key::Named(NamedKey::ArrowUp));
    for _ in 0..60 {
        scene.update(1.0 / 60.0, &up);
        up.keys_pressed.clear();
    }
    let neutral = scene
        .world
        .get_component_by_id_as::<TransformComponent>(neck)
        .unwrap()
        .transform
        .rotation;
    assert!(
        neutral
            .iter()
            .zip(neck_rest.rotation)
            .map(|(a, b)| a * b)
            .sum::<f32>()
            .abs()
            > 0.9999,
        "neck must return to neutral"
    );

    // Exercise the real desktop input path after initialization, including look.
    let before = [camera[3][0], camera[3][1], camera[3][2]];
    let before_forward = [camera[2][0], camera[2][1], camera[2][2]];
    let mut input = crate::engine::user_input::InputState::default();
    input.keys_down.insert(Key::Character("w".into()));
    input.keys_down.insert(Key::Named(NamedKey::ArrowUp));
    input.keys_pressed.insert(Key::Named(NamedKey::ArrowUp));
    for _ in 0..45 {
        scene.update(1.0 / 60.0, &input);
        input.keys_pressed.clear();
    }
    let pose = &scene
        .world
        .get_component_by_id_as::<TransformComponent>(named(&scene, "sakura_desktop_camera"))
        .unwrap()
        .transform
        .matrix_world;
    assert!(pose.iter().flatten().all(|v| v.is_finite()));
    assert!(
        (pose[3][0] - before[0]).hypot(pose[3][2] - before[2]) > 0.2,
        "WASD did not move camera"
    );
    let forward = [pose[2][0], pose[2][1], pose[2][2]];
    assert!(
        crate::utils::math::vec3_len(crate::utils::math::vec3_sub(forward, before_forward)) > 0.1,
        "desktop look did not rotate the camera"
    );
    assert_eq!(scene.world.parent_of(head), Some(head_parent));
    let avc = named(&scene, "sakura_desktop_avatar_control");
    assert!(
        scene
            .systems
            .lower_body_ik
            .walking(avc)
            .unwrap()
            .steps_started
            .iter()
            .sum::<u64>()
            > 0,
        "desktop walking did not start"
    );
    // Turning native neck bending off must keep the spliced head target live
    // without accumulating body movement.
    scene
        .world
        .get_component_by_id_as_mut::<AvatarControlComponent>(avc)
        .unwrap()
        .head_neck_body_enabled = false;
    for _ in 0..10 {
        scene.update(1.0 / 60.0, &Default::default());
    }
    let camera_id = named(&scene, "sakura_desktop_camera");
    let stable = scene
        .world
        .get_component_by_id_as::<TransformComponent>(camera_id)
        .unwrap()
        .transform
        .matrix_world;
    for _ in 0..30 {
        scene.update(1.0 / 60.0, &Default::default());
    }
    let final_pose = scene
        .world
        .get_component_by_id_as::<TransformComponent>(camera_id)
        .unwrap()
        .transform
        .matrix_world;
    assert!(
        (final_pose[3][0] - stable[3][0]).abs() < 0.001
            && (final_pose[3][2] - stable[3][2]).abs() < 0.001,
        "legacy desktop follow accumulated movement"
    );
    assert!(
        crate::utils::math::vec3_len(crate::utils::math::vec3_sub(
            [final_pose[2][0], final_pose[2][1], final_pose[2][2]],
            forward
        )) < 0.001,
        "legacy fallback lost the look target"
    );
}
