use mittens_engine::engine::{
    Universe,
    ecs::{
        ComponentId, EventSignal, SignalEmitter, World,
        component::{GLTFComponent, SpotLightComponent, TransformComponent},
        system::TransformSystem,
    },
    graphics::{CpuMesh, MeshHandle, MeshUploader, TextureHandle, TextureUploader},
};

#[derive(Default)]
struct Upload(u32, std::collections::HashMap<MeshHandle, usize>);
impl MeshUploader for Upload {
    fn upload_mesh(&mut self, mesh: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        self.0 += 1;
        self.1
            .insert(MeshHandle(self.0), mesh.indices_u32.len() / 3);
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

fn descendants(world: &World, root: ComponentId) -> Vec<ComponentId> {
    let mut result = Vec::new();
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        result.push(id);
        pending.extend_from_slice(world.children_of(id));
    }
    result
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

fn check_wheels(scene: &Universe, left_ratio: f32, right_ratio: f32) {
    for root in ["fork_fixture", "sibling_fixture"] {
        let scope = named(&scene.world, root);
        for corner in ["fl", "fr", "rl", "rr"] {
            let wheel = scene
                .world
                .find_component(scope, &format!("#wheel_{corner}"))
                .unwrap();
            let matrix = TransformSystem::world_model(&scene.world, wheel).unwrap();
            let ratio = if corner.ends_with('l') {
                left_ratio
            } else {
                right_ratio
            };
            // Positions and intrinsic wheel scales are authored in mitten_pickup.glb.
            let (mount_y, hub_y, spring_name, wheel_name) = match corner {
                "fl" => (
                    0.801975,
                    -0.45528355,
                    "suspension_spring",
                    "front-left-WHEEL",
                ),
                "fr" => (
                    0.801975,
                    -0.45528355,
                    "suspension_spring.002",
                    "front-left-WHEEL.003",
                ),
                "rl" => (
                    0.755284,
                    -0.45528355,
                    "suspension_spring.001",
                    "front-left-WHEEL.001",
                ),
                _ => (
                    0.755284,
                    -0.45525387,
                    "suspension_spring.003",
                    "front-left-WHEEL.002",
                ),
            };
            let expected_y = mount_y + hub_y * ratio;
            assert!(
                (matrix[3][1] - expected_y).abs() < 1e-4,
                "{root}/{corner}: hub height {}, expected {expected_y}",
                matrix[3][1]
            );
            let spring = scene
                .world
                .find_component(scope, &format!("[name='{spring_name}']"))
                .unwrap();
            let spring_matrix = TransformSystem::world_model(&scene.world, spring).unwrap();
            let spring_scale_y = spring_matrix[1][..3]
                .iter()
                .map(|v| v * v)
                .sum::<f32>()
                .sqrt();
            assert!(
                (spring_scale_y - ratio).abs() < 1e-4,
                "the imported coil must compress"
            );
            let imported_wheel = scene
                .world
                .find_component(scope, &format!("[name='{wheel_name}']"))
                .unwrap();
            let tyre_matrix = TransformSystem::world_model(&scene.world, imported_wheel).unwrap();
            for (column, expected) in tyre_matrix[..3].iter().zip([0.30, 0.12, 0.30]) {
                let length = column[..3].iter().map(|v| v * v).sum::<f32>().sqrt();
                assert!(
                    (length - expected).abs() < 1e-4,
                    "spring travel must preserve authored tyre scale"
                );
            }
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
    for root in ["fork_fixture", "sibling_fixture"] {
        let scope = named(&scene.world, root);
        let model = scene.world.find_component(scope, "#pickup_model").unwrap();
        let gltf = scene
            .world
            .get_component_by_id_as::<GLTFComponent>(model)
            .unwrap();
        assert!(gltf.spawned);
        assert_eq!(gltf.uri, "assets/models/mitten_pickup.glb");
        let vehicle = scene.world.find_component(scope, "#pickup_truck").unwrap();
        let vehicle_matrix = TransformSystem::world_model(&scene.world, vehicle).unwrap();
        let forward = vehicle_matrix[2][..3]
            .iter()
            .map(|v| -v)
            .collect::<Vec<_>>();
        for side in [-1, 1] {
            let frame = scene
                .world
                .find_component(scope, &format!("#pickup_headlight_{side}"))
                .unwrap();
            let spot = scene
                .world
                .children_of(frame)
                .iter()
                .find_map(|&id| scene.world.get_component_by_id_as::<SpotLightComponent>(id))
                .expect("each headlight must be an actual spotlight");
            assert_eq!(spot.distance, 25.0);
            let position = TransformSystem::world_position(&scene.world, frame).unwrap();
            let light = scene
                .visuals
                .lights()
                .iter()
                .find(|light| {
                    light.light_type == 3
                        && (0..3).all(|i| (light.position_ws[i] - position[i]).abs() < 1e-4)
                })
                .unwrap();
            let alignment = (0..3)
                .map(|i| forward[i] * light.direction_ws[i])
                .sum::<f32>();
            assert!(
                alignment > 0.999,
                "headlights must point forward even with a rotated truck"
            );
        }
        for gauge in ["rpm", "speed", "fuel", "odometer"] {
            scene
                .world
                .find_component(scope, &format!("#pickup_gauge_{gauge}"))
                .unwrap();
        }
        let gauges = scene
            .world
            .find_component(scope, "#pickup_dashboard_gauge_mesh")
            .unwrap();
        assert!(
            scene
                .visuals
                .instances()
                .iter()
                .any(|instance| instance.component == gauges),
            "the gauge geometry should be combined into one draw instance"
        );
        assert!(
            !descendants(&scene.world, scope).into_iter().any(|id| {
                scene
                    .world
                    .get_component_by_id_as::<GLTFComponent>(id)
                    .is_some_and(|gltf| gltf.uri.ends_with("suspension-spring.glb"))
            }),
            "use the springs already authored into the truck, without duplicate geometry"
        );
        let batch = scene
            .world
            .find_component(scope, "#pickup_painted_body")
            .unwrap();
        assert!(
            scene
                .visuals
                .instances()
                .iter()
                .any(|instance| instance.component == batch),
            "stationary body parts must produce their combined render output"
        );
        let body_sources=descendants(&scene.world,batch).into_iter().filter(|&id| {
            scene.world.get_component_by_id_as::<mittens_engine::engine::ecs::component::RenderableComponent>(id).is_some()
        }).count();
        let combined = scene
            .visuals
            .instances()
            .iter()
            .find(|instance| instance.component == batch)
            .unwrap();
        assert_eq!(
            upload.1.get(&combined.renderable.mesh).copied(),
            Some(1589),
            "combine all authored body triangles, not unresolved placeholder meshes"
        );
        assert_eq!(
            body_sources, 8,
            "combine the body and its seven static child meshes"
        );
        assert!(
            scene.world.find_component(batch, "#wheel_fl").is_none(),
            "wheel travel must not rebuild the body mesh"
        );
    }
    check_wheels(&scene, 1.0, 1.0);
    for _ in 0..3 {
        click(&mut scene, 1);
        check_wheels(&scene, 0.5, 0.5);
        click(&mut scene, 2);
        check_wheels(&scene, 0.5, 1.0);
        click(&mut scene, 0);
        check_wheels(&scene, 1.0, 1.0);
    }
    // The fixed probe mounts must not follow spring compression.
    for root in ["fork_fixture", "sibling_fixture"] {
        let scope = named(&scene.world, root);
        let mount = scene
            .world
            .find_component(scope, "#suspension_mount_fl")
            .unwrap();
        assert!(
            (TransformSystem::world_position(&scene.world, mount).unwrap()[1] - 0.801975).abs()
                < 1e-4
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
