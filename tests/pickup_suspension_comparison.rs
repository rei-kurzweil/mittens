use mittens_engine::engine::{
    Universe,
    ecs::{
        ComponentId, EventSignal, SignalEmitter, World,
        component::{GLTFComponent, TransformComponent},
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

fn named(world: &World, label: &str) -> ComponentId {
    world
        .all_components()
        .find(|&id| world.component_label(id) == Some(label))
        .unwrap()
}

fn tick(scene: &mut Universe) {
    for _ in 0..3 {
        scene.update(1.0 / 60.0, &Default::default());
    }
}

fn click(scene: &mut Universe, index: usize) {
    let target = named(&scene.world, &format!("pickup_suspension_compare_{index}"));
    scene.command_queue.push_event(
        target,
        EventSignal::Click {
            raycaster: ComponentId::default(),
            renderable: target,
            hit_point: [0.0; 3],
            screen_pos_px: None,
        },
    );
    tick(scene);
}

fn check_wheels(scene: &Universe, left_y: f32, right_y: f32) {
    for root in ["fork_fixture", "sibling_fixture"] {
        let scope = named(&scene.world, root);
        for corner in ["fl", "fr", "rl", "rr"] {
            let wheel = scene
                .world
                .find_component(scope, &format!("#wheel_{corner}"))
                .unwrap();
            let matrix = TransformSystem::world_model(&scene.world, wheel).unwrap();
            let expected_y = if corner.ends_with('l') {
                left_y
            } else {
                right_y
            };
            assert!(
                (matrix[3][1] - expected_y).abs() < 1e-4,
                "{root}/{corner}: hub height {}, expected {expected_y}",
                matrix[3][1]
            );
            let spring = scene
                .world
                .find_component(scope, &format!("#spring_mesh_{corner}"))
                .unwrap();
            let spring = scene
                .world
                .get_component_by_id_as::<GLTFComponent>(spring)
                .unwrap();
            assert!(
                spring.spawned && spring.spawned_node_transforms.len() == 1,
                "{root}/{corner}: the spring GLB must actually load"
            );
            let spring_matrix =
                TransformSystem::world_model(&scene.world, spring.spawned_node_transforms[0])
                    .unwrap();
            let spring_scale_y = spring_matrix[1][..3]
                .iter()
                .map(|v| v * v)
                .sum::<f32>()
                .sqrt();
            assert!(
                (spring_scale_y - (0.9 - expected_y) / 0.5).abs() < 1e-4,
                "{root}/{corner}: the imported coil must compress with suspension travel"
            );
            for column in &matrix[..3] {
                let length = column[..3].iter().map(|v| v * v).sum::<f32>().sqrt();
                assert!(
                    (length - 1.0).abs() < 1e-4,
                    "{root}/{corner}: spring scale must not distort the wheel: {length}"
                );
            }
            let other = if root == "fork_fixture" {
                "sibling_fixture"
            } else {
                "fork_fixture"
            };
            let other_scope = named(&scene.world, other);
            let other_wheel = scene
                .world
                .find_component(other_scope, &format!("#wheel_{corner}"))
                .unwrap();
            let other_matrix = TransformSystem::world_model(&scene.world, other_wheel).unwrap();
            for axis in 0..3 {
                let local = matrix[3][axis]
                    - TransformSystem::world_position(&scene.world, scope).unwrap()[axis];
                let other_local = other_matrix[3][axis]
                    - TransformSystem::world_position(&scene.world, other_scope).unwrap()[axis];
                assert!(
                    (local - other_local).abs() < 1e-4,
                    "the variants must agree on hub placement"
                );
            }
        }
    }
}

#[test]
fn both_pickup_suspensions_compress_without_scaling_tyres() {
    let mut scene = Universe::new(World::default());
    scene.load_mms_source_at_path(r#"
        import { pickup_truck, pickup_truck_sibling_suspension } from "../assets/components/vehicles/pickup_truck.mms"
        import { pickup_suspension_comparison } from "../assets/components/vehicles/pickup_suspension_comparison.mms"
        let fork = pickup_truck()
        let sibling = pickup_truck_sibling_suspension()
        T.rotation(0,0.6,0) { name="fork_fixture" fork }
        T.position(8,0,0).rotation(0,0.6,0) { name="sibling_fixture" sibling }
        pickup_suspension_comparison(fork,sibling)
    "#, "examples/_pickup_suspension_comparison_test.mms").unwrap();
    let mut upload = Upload::default();
    for _ in 0..3 {
        tick(&mut scene);
        scene.systems.prepare_render(
            &mut scene.world,
            &mut scene.visuals,
            &mut scene.render_assets,
            &mut upload,
            &mut scene.command_queue,
        );
    }
    // Finish asynchronous shader preparation before making assertions/exiting.
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
    tick(&mut scene);
    check_wheels(&scene, 0.4, 0.4);
    for _ in 0..3 {
        click(&mut scene, 1);
        check_wheels(&scene, 0.65, 0.65);
        click(&mut scene, 2);
        check_wheels(&scene, 0.65, 0.4);
        click(&mut scene, 0);
        check_wheels(&scene, 0.4, 0.4);
    }
    // The fixed probe mounts must not follow spring compression.
    for root in ["fork_fixture", "sibling_fixture"] {
        let scope = named(&scene.world, root);
        let mount = scene
            .world
            .find_component(scope, "#suspension_mount_fl")
            .unwrap();
        assert!(
            (TransformSystem::world_position(&scene.world, mount).unwrap()[1] - 0.9).abs() < 1e-4
        );
        let wheel = scene.world.find_component(scope, "#wheel_fl").unwrap();
        assert_eq!(
            scene
                .world
                .get_component_by_id_as::<TransformComponent>(wheel)
                .unwrap()
                .scale(),
            [1.0; 3]
        );
    }
}
