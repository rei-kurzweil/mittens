use mittens_engine::engine::{
    Universe,
    ecs::{
        World,
        component::*,
        system::{TransformSystem, external_xr_frame::ExternalXrFrame},
    },
};

#[derive(Default)]
struct Upload(u32);
impl mittens_engine::engine::graphics::MeshUploader for Upload {
    fn upload_mesh(
        &mut self,
        _: &mittens_engine::engine::graphics::CpuMesh,
    ) -> Result<mittens_engine::engine::graphics::MeshHandle, Box<dyn std::error::Error>> {
        self.0 += 1;
        Ok(mittens_engine::engine::graphics::MeshHandle(self.0))
    }
}
impl mittens_engine::engine::graphics::TextureUploader for Upload {
    fn upload_texture_rgba8(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
    ) -> Result<mittens_engine::engine::graphics::TextureHandle, Box<dyn std::error::Error>> {
        self.0 += 1;
        Ok(mittens_engine::engine::graphics::TextureHandle(self.0))
    }
    fn upload_texture_bc7(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
        _: bool,
    ) -> Result<mittens_engine::engine::graphics::TextureHandle, Box<dyn std::error::Error>> {
        self.0 += 1;
        Ok(mittens_engine::engine::graphics::TextureHandle(self.0))
    }
}

#[test]
fn ship_xr_avatar_camera_stays_inside_the_bridge_after_initialization() {
    let mut scene = Universe::new(World::default());
    let view = openxr::View {
        pose: openxr::Posef {
            orientation: openxr::Quaternionf {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            position: openxr::Vector3f {
                x: 0.0,
                y: 1.65,
                z: 0.0,
            },
        },
        fov: openxr::Fovf {
            angle_left: -0.7,
            angle_right: 0.7,
            angle_up: 0.7,
            angle_down: -0.7,
        },
    };
    scene.systems.xr.set_external_frame(Some(ExternalXrFrame {
        views: Some([view, view]),
        ..Default::default()
    }));
    // Exercise the example's exact rig against a bridge-height floor fixture;
    // unrelated shops, weapon audio and scenery are covered by the scene tests.
    let example = include_str!("../examples/ship_interior.mms");
    let start = example.find("if xr_view_enabled {").unwrap();
    let end = start
        + example[start..]
            .find("\n    if desktop_view_enabled {")
            .unwrap();
    let source = format!(
        r#"
        let xr_view_enabled = true
        let view_far_distance = 1000.0
        let xr_avatar_enabled = true
        import {{ bisket_humanoid_bone_map }} from "../assets/components/humanoid_bone_maps/bisket.mms"
        import {{ ship_avatar_material }} from "../assets/components/materials/ship_anime_materials.mms"
        import {{ vroid_arm_ik_bend_plane_prototype }} from "../assets/components/arm_ik/vroid.mms"
        T.position(0,-0.1,-6).scale(12,0.2,12) {{
            Zone.cube([0.5,0.5,0.5]) {{ Collidable.static() }}
        }}
        {}
    "#,
        &example[start..end]
    );
    scene
        .load_mms_source_at_path(&source, "examples/_ship_xr_startup_test.mms")
        .unwrap();
    let mut upload = Upload::default();
    for _ in 0..90 {
        scene.update(1.0 / 90.0, &Default::default());
        scene.systems.prepare_render(
            &mut scene.world,
            &mut scene.visuals,
            &mut scene.render_assets,
            &mut upload,
            &mut scene.command_queue,
        );
    }
    let root = scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some("ship_player_root"))
        .unwrap();
    let gravity = scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some("ship_player_gravity"))
        .unwrap();
    let ready = scene
        .world
        .get_component_by_id_as::<GravityComponent>(gravity)
        .unwrap()
        .enabled;
    let camera = scene.systems.camera.active_xr_camera_component().unwrap();
    let eye = TransformSystem::world_position(&scene.world, camera).unwrap();
    eprintln!(
        "root {:?}, eye {eye:?}, ready {}",
        TransformSystem::world_position(&scene.world, root),
        ready
    );
    assert!(ready);
    let views = &scene
        .visuals
        .visual_camera(mittens_engine::engine::graphics::CameraTarget::Xr)
        .unwrap()
        .eyes;
    let camera_config = scene
        .world
        .get_component_by_id_as::<CameraXRComponent>(camera)
        .unwrap();
    assert_eq!(camera_config.z_far, 1000.0);
    for view in views {
        let far = view.proj[3][2] / (1.0 + view.proj[2][2]);
        assert!(
            (far - 1000.0).abs() < 5.0,
            "XR projection ignores authored far plane: {far}"
        );
    }
    assert_eq!(views.len(), 2, "the XR rig must publish both eye views");
    for view in views {
        let y = view.transform.matrix_world[3][1];
        assert!(
            y > 0.3 && y < 3.2,
            "rendered eye must be inside the bridge: {y}"
        );
    }
    assert!(eye[0].abs() < 5.8 && eye[2] > -11.8 && eye[2] < 0.0);
    assert!(
        eye[1] > 0.3 && eye[1] < 3.2,
        "XR eye must be above the floor and below the ceiling: {eye:?}"
    );
    // Do not exit while native material compilation workers are still active.
    let definitions: Vec<_> = scene
        .world
        .all_components()
        .filter_map(|id| {
            let registry = &scene.visuals.materials;
            registry
                .definition(registry.source_definition(id)?)?
                .custom
                .clone()
        })
        .collect();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while definitions.iter().any(|d| d.compiled.get().is_none()) {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
