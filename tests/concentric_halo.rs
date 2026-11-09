use mittens_engine::engine::{
    Universe,
    ecs::{
        ComponentId, World,
        component::{
            ArmatureComponent, BoneRestPoseComponent, RenderableComponent, TransformComponent,
        },
        system::TransformSystem,
    },
    graphics::{CpuMesh, MeshHandle, MeshUploader, TextureHandle, TextureUploader},
};

#[derive(Default)]
struct Upload(u32);
impl MeshUploader for Upload {
    fn upload_mesh(&mut self, _: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        self.0 += 1;
        Ok(MeshHandle(self.0))
    }
}
impl TextureUploader for Upload {
    fn upload_texture_rgba8(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        self.0 += 1;
        Ok(TextureHandle(self.0))
    }
    fn upload_texture_bc7(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
        _: bool,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        self.0 += 1;
        Ok(TextureHandle(self.0))
    }
}
fn named(scene: &Universe, label: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(label))
        .unwrap_or_else(|| panic!("missing {label}"))
}
fn wait_shaders(scene: &Universe) {
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
        assert!(
            std::time::Instant::now() < deadline,
            "shader compilation timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    for definition in definitions {
        assert!(
            definition.compiled.get().unwrap().is_ok(),
            "{:?}",
            definition.compiled.get()
        );
    }
}
fn prepare(scene: &mut Universe, upload: &mut Upload) {
    scene.systems.prepare_render(
        &mut scene.world,
        &mut scene.visuals,
        &mut scene.render_assets,
        upload,
        &mut scene.command_queue,
    );
}
fn fixture() -> Universe {
    fixture_options("null")
}
fn fixture_options(options: &str) -> Universe {
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(
            r#"
        import {concentric_halo} from "../assets/components/accessories/concentric_halo.mms"
        T.position(0,1.7,0) {name="head" concentric_halo("test_halo",null)}
    "#
            .replace("\"test_halo\",null", &format!("\"test_halo\",{options}"))
            .as_str(),
            "examples/_halo_test.mms",
        )
        .unwrap();
    scene.update(0., &Default::default());
    wait_shaders(&scene);
    scene
}

#[test]
fn halo_is_a_rigid_spring_chain_with_single_layer_translucent_custom_material() {
    let mut scene = fixture();
    let rig = named(&scene, "test_halo_rig");
    let config = scene
        .world
        .get_component_by_id_as::<ArmatureComponent>(rig)
        .unwrap();
    assert!(config.rest_pose_only && !config.automatic_weights);
    assert!(config.status.starts_with("bound:"), "{}", config.status);
    assert_eq!(scene.systems.rigging.binding_count(), 1);
    let snapshot = scene.systems.secondary_motion.bound_snapshot(&scene.world);
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].segments.len(), 3);
    let mut upload = Upload::default();
    prepare(&mut scene, &mut upload);
    scene.visuals.prepare_draw_cache();
    assert!(scene.visuals.opaque_stream().1.is_empty());
    assert_eq!(scene.visuals.transparent_single_stream().1.len(), 2);
    for index in 0..2 {
        let joint = named(&scene, &format!("test_halo_ring_{index}"));
        assert_eq!(
            scene
                .world
                .children_of(joint)
                .iter()
                .filter(|&&id| scene
                    .world
                    .get_component_by_id_as::<BoneRestPoseComponent>(id)
                    .is_some())
                .count(),
            1
        );
        let render = named(&scene, &format!("test_halo_mesh_{index}"));
        let component = scene
            .world
            .get_component_by_id_as::<RenderableComponent>(render)
            .unwrap();
        let cpu = scene
            .render_assets
            .cpu_mesh(component.renderable.mesh)
            .unwrap();
        assert!(cpu.joints0.is_none() && cpu.weights0.is_none());
        assert_eq!(cpu.indices_u32.len(), 64 * 4 * 6);
        let instance = scene
            .visuals
            .instance(component.get_handle().unwrap())
            .unwrap();
        assert_eq!(instance.bones_count, 0);
        assert_eq!(instance.deformed_count, 0);
        assert!(!instance.multiple_layers && !instance.transparent_cutout);
        assert_eq!(
            instance.emissive, 0.,
            "no opaque emission-extraction flag on the blended surface"
        );
        assert!((instance.color[3] * instance.opacity - 0.7).abs() < 1e-6);
        if index == 0 {
            assert_eq!(&instance.color[..3], &[1., 0.16, 0.22]);
        }
        let material = scene
            .visuals
            .materials
            .instance(instance.material_instance.unwrap())
            .unwrap();
        let definition = scene
            .visuals
            .materials
            .definition(material.definition)
            .unwrap();
        assert_eq!(
            definition.custom.as_ref().unwrap().fragment_uri,
            "assets/shaders/halo.frag"
        );
    }
    let uploads = upload.0;
    for _ in 0..10 {
        scene.update(1. / 60., &Default::default());
        prepare(&mut scene, &mut upload);
    }
    assert_eq!(
        uploads, upload.0,
        "spring pose updates must not rebuild mesh/pipeline resources"
    );
}

#[test]
fn upper_halo_rings_lag_more_and_authored_rest_poses_do_not_change() {
    let mut scene = fixture();
    let head = named(&scene, "head");
    for _ in 0..10 {
        scene.update(1. / 60., &Default::default());
    }
    let mut max_lag = [0.0_f32; 2];
    let mut max_stretch = 0.0_f32;
    for step in 0..45 {
        let x = step as f32 * 0.008;
        let t = scene
            .world
            .get_component_by_id_as_mut::<TransformComponent>(head)
            .unwrap();
        t.transform.translation[0] = x;
        t.transform.recompute_model();
        scene
            .systems
            .transform_changed(&mut scene.world, &mut scene.visuals, head);
        scene.update(1. / 60., &Default::default());
        let root =
            TransformSystem::world_position(&scene.world, named(&scene, "test_halo_joint_root"))
                .unwrap();
        let first =
            TransformSystem::world_position(&scene.world, named(&scene, "test_halo_ring_0"))
                .unwrap();
        let distance = root
            .into_iter()
            .zip(first)
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f32>()
            .sqrt();
        max_stretch = max_stretch.max((distance - 0.26).abs());
        for index in 0..2 {
            let joint = named(&scene, &format!("test_halo_ring_{index}"));
            let position = TransformSystem::world_position(&scene.world, joint).unwrap();
            max_lag[index] = max_lag[index].max(x - position[0]);
            let rest = scene
                .world
                .children_of(joint)
                .iter()
                .find_map(|&id| {
                    scene
                        .world
                        .get_component_by_id_as::<BoneRestPoseComponent>(id)
                })
                .unwrap();
            assert_eq!(rest.rotation, [0., 0., 0., 1.]);
            assert!((rest.translation[1] - if index == 0 { 0.26 } else { 0.10 }).abs() < 1e-6);
        }
    }
    eprintln!("halo ring maximum lag in meters: {max_lag:?}");
    assert!(
        max_stretch > 0.01,
        "joint spacing must be free to stretch: {max_stretch}"
    );
    assert!(max_lag[1] > 0.02, "top must visibly lag: {max_lag:?}");
    assert!(
        max_lag[1] > max_lag[0],
        "lag accumulates upward: {max_lag:?}"
    );
    let initial_x = scene
        .world
        .get_component_by_id_as::<TransformComponent>(head)
        .unwrap()
        .translation()[0];
    for _ in 0..240 {
        scene.update(1. / 60., &Default::default());
    }
    let top =
        TransformSystem::world_position(&scene.world, named(&scene, "test_halo_ring_1")).unwrap();
    assert!(
        (top[0] - initial_x).abs() < 0.02,
        "settle toward upright without gravity sag: {top:?}"
    );
    // The captured target follows the head's live orientation, not world up.
    let t = scene
        .world
        .get_component_by_id_as_mut::<TransformComponent>(head)
        .unwrap();
    t.transform.rotation = [0., 0., (0.15_f32).sin(), (0.15_f32).cos()];
    t.transform.recompute_model();
    scene
        .systems
        .transform_changed(&mut scene.world, &mut scene.visuals, head);
    for _ in 0..240 {
        scene.update(1. / 60., &Default::default());
    }
    let top =
        TransformSystem::world_position(&scene.world, named(&scene, "test_halo_ring_1")).unwrap();
    let expected = [
        initial_x - (0.36 * std::f32::consts::FRAC_1_SQRT_2) * (0.3_f32).sin(),
        1.7 + (0.36 * std::f32::consts::FRAC_1_SQRT_2) * (0.3_f32).cos(),
    ];
    assert!(
        (top[0] - expected[0]).abs() < 0.02 && (top[1] - expected[1]).abs() < 0.02,
        "settled chain follows the head's orientation: {top:?}, expected {expected:?}"
    );
}

#[test]
fn response_scale_controls_translation_lag_and_basis_is_configurable() {
    let mut slow = fixture();
    let mut fast = fixture_options("{response_scale=1.0 position=[0,0,0] rotation=[0,0,0]}");
    let basis = fast
        .world
        .get_component_by_id_as::<TransformComponent>(named(&fast, "test_halo"))
        .unwrap();
    assert_eq!(basis.transform.translation, [0.; 3]);
    assert_eq!(basis.transform.rotation, [0., 0., 0., 1.]);
    let default_basis = slow
        .world
        .get_component_by_id_as::<TransformComponent>(named(&slow, "test_halo"))
        .unwrap();
    assert!(
        (default_basis.transform.rotation[0] - (-std::f32::consts::FRAC_PI_8).sin()).abs() < 1e-5
    );
    assert_eq!(default_basis.transform.translation, [0., 0., -0.10]);
    let mut lags = Vec::new();
    for scene in [&mut slow, &mut fast] {
        let head = named(scene, "head");
        let t = scene
            .world
            .get_component_by_id_as_mut::<TransformComponent>(head)
            .unwrap();
        t.transform.translation[0] = 0.3;
        t.transform.recompute_model();
        scene
            .systems
            .transform_changed(&mut scene.world, &mut scene.visuals, head);
        for _ in 0..6 {
            scene.update(1. / 60., &Default::default());
        }
        let ring = TransformSystem::world_position(&scene.world, named(scene, "test_halo_ring_0"))
            .unwrap();
        lags.push(0.3 - ring[0]);
    }
    assert!(
        lags[0] > lags[1] + 0.02,
        "lower response scale produces more lag: {lags:?}"
    );
}

#[test]
fn halo_demo_scene_loads_and_captures_its_rigid_chain() {
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(include_str!("../examples/halo.mms"), "examples/halo.mms")
        .unwrap();
    wait_shaders(&scene);
    for _ in 0..5 {
        scene.update(1. / 60., &Default::default());
    }
    assert!(
        scene
            .world
            .get_component_by_id_as::<ArmatureComponent>(named(&scene, "halo_demo_rig"))
            .unwrap()
            .status
            .starts_with("bound:")
    );
}

#[test]
fn stretch_is_opt_in_rejects_unsupported_collisions_and_restores_translations_on_pause() {
    use mittens_engine::engine::ecs::component::SpringBoneComponent;
    assert!(!SpringBoneComponent::new("hair").allow_stretch);
    let mut scene = fixture();
    let spring = named(&scene, "test_halo_spring");
    let config = scene
        .world
        .get_component_by_id_as_mut::<SpringBoneComponent>(spring)
        .unwrap();
    assert!(config.allow_stretch);
    config.world_collision = true;
    scene
        .systems
        .secondary_motion
        .configuration_changed(&scene.world, spring);
    assert!(
        scene
            .systems
            .secondary_motion
            .bound_snapshot(&scene.world)
            .is_empty()
    );
    scene
        .world
        .get_component_by_id_as_mut::<SpringBoneComponent>(spring)
        .unwrap()
        .world_collision = false;
    scene
        .systems
        .secondary_motion
        .configuration_changed(&scene.world, spring);
    assert_eq!(
        scene
            .systems
            .secondary_motion
            .bound_snapshot(&scene.world)
            .len(),
        1
    );
    let head = named(&scene, "head");
    let t = scene
        .world
        .get_component_by_id_as_mut::<TransformComponent>(head)
        .unwrap();
    t.transform.translation[0] = 0.3;
    t.transform.recompute_model();
    scene
        .systems
        .transform_changed(&mut scene.world, &mut scene.visuals, head);
    for _ in 0..10 {
        scene.update(1. / 60., &Default::default());
    }
    let dirty = scene.systems.secondary_motion.tick_with_settings(
        &mut scene.world,
        1. / 60.,
        &Default::default(),
        false,
    );
    assert!(!dirty.is_empty());
    for index in 0..2 {
        let t = scene
            .world
            .get_component_by_id_as::<TransformComponent>(named(
                &scene,
                &format!("test_halo_ring_{index}"),
            ))
            .unwrap();
        assert_eq!(
            t.transform.translation,
            [0., if index == 0 { 0.26 } else { 0.10 }, 0.]
        );
        assert_eq!(t.transform.rotation, [0., 0., 0., 1.]);
    }
}
