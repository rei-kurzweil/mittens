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
    let slot = named(&scene, "sakura_desktop_camera_slot");
    let head = scene.world.parent_of(slot).unwrap();
    assert_eq!(scene.world.component_label(head), Some("J_Bip_C_Head"));
    assert_ne!(
        scene.world.parent_of(head),
        avatar.head_mount,
        "desktop must retain the imported neck/head attachment"
    );
    let head_parent = scene.world.parent_of(head).unwrap();
    assert_eq!(
        scene.world.component_label(head_parent),
        Some("J_Bip_C_Neck")
    );

    assert_eq!(
        scene
            .world
            .parent_of(named(&scene, "sakura_desktop_camera")),
        Some(slot)
    );
    let camera = &scene
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
    // Exercise the real desktop input path after initialization, including look.
    use crate::engine::user_input::{Key, NamedKey};
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
    // Turning native neck bending off must keep desktop look live without
    // feeding the attached head pose back into body placement each frame.
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
