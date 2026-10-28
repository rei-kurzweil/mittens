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
}
