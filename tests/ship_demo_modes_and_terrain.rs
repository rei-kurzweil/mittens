use mittens_engine::engine::{
    Universe,
    ecs::{World, component::*},
    graphics::{MeshHandle, MeshUploader, mesh::CpuMesh},
};

fn named(world: &World, label: &str) -> mittens_engine::engine::ecs::ComponentId {
    world
        .all_components()
        .find(|&id| world.component_label(id) == Some(label))
        .unwrap()
}

fn finish_materials(scene: &Universe) {
    let materials: Vec<_> = scene
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
    while materials
        .iter()
        .any(|material| material.compiled.get().is_none())
    {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[derive(Default)]
struct Meshes {
    next: u32,
}
impl MeshUploader for Meshes {
    fn upload_mesh(&mut self, _: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        self.next += 1;
        Ok(MeshHandle(self.next))
    }
}

impl mittens_engine::engine::graphics::TextureUploader for Meshes {
    fn upload_texture_rgba8(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
    ) -> Result<mittens_engine::engine::graphics::TextureHandle, Box<dyn std::error::Error>> {
        self.next += 1;
        Ok(mittens_engine::engine::graphics::TextureHandle(self.next))
    }
    fn upload_texture_bc7(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
        _: bool,
    ) -> Result<mittens_engine::engine::graphics::TextureHandle, Box<dyn std::error::Error>> {
        self.next += 1;
        Ok(mittens_engine::engine::graphics::TextureHandle(self.next))
    }
}

#[test]
fn ship_view_flags_rider_patrol_and_clipped_park_ground_work_together() {
    let case = std::env::var("SHIP_DEMO_TEST_CASE").ok();
    if case.is_none() {
        // Native renderer initialization and retained GLTF callbacks have process
        // lifetime state. Exercise each startup preset in its own process.
        for case in ["none", "desktop", "xr", "both", "xr_free", "patrol"] {
            eprintln!("ship startup case {case}");
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "ship_view_flags_rider_patrol_and_clipped_park_ground_work_together",
                    "--nocapture",
                ])
                .env("SHIP_DEMO_TEST_CASE", case)
                .status()
                .unwrap();
            assert!(status.success(), "ship startup case {case} failed");
        }
        return;
    }
    let case = case.unwrap();
    let source = include_str!("../examples/ship_interior.mms");
    for (label, desktop, xr, avatar) in [
        ("none", false, false, false),
        ("desktop", true, false, true),
        ("xr", false, true, true),
        ("both", true, true, true),
        ("xr_free", false, true, false),
    ] {
        if case != label {
            continue;
        }
        // The example's editable defaults may select desktop or XR. Each
        // fixture explicitly chooses its preset without assuming those defaults.
        let configured = source
            .lines()
            .map(|line| {
                if line.starts_with("let desktop_view_enabled = ") {
                    format!("let desktop_view_enabled = {desktop}")
                } else if line.starts_with("let xr_view_enabled = ") {
                    format!("let xr_view_enabled = {xr}")
                } else if line.starts_with("let xr_avatar_enabled = ") {
                    format!("let xr_avatar_enabled = {avatar}")
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let mut scene = Universe::new(World::default());
        // Verify XR scene wiring without probing the workstation's runtime.
        scene
            .systems
            .xr
            .set_external_frame(Some(Default::default()));
        scene
            .load_mms_source_at_path(&configured, "examples/ship_interior.mms")
            .unwrap();
        finish_materials(&scene);
        let world = &scene.world;
        let count = |label: &str| {
            world
                .all_components()
                .filter(|&id| world.component_label(id) == Some(label))
                .count()
        };
        assert_eq!(count("ship_desktop_camera"), usize::from(desktop));
        assert_eq!(count("ship_avatar_control"), usize::from(xr && avatar));
        assert_eq!(count("ship_xr_free_camera"), usize::from(xr && !avatar));
        assert_eq!(count("ship_lumae_motorcycle_rider"), 0);
        assert_eq!(count("recreational_vehicle"), 0);
        assert_eq!(count("ship_trailer_park"), 0);
        assert_eq!(count("ship_orchard_pickup_north"), 1);
        assert_eq!(count("ship_orchard_pickup_south"), 1);
        assert_eq!(count("pickup_truck"), 2); // hot-dog stand is commented out for profiling
        for label in [
            "ship_bridge_computers",
            "ship_coffee_store",
            "ship_noodle_store",
            "ship_grill_store",
            "ship_hot_dog_store",
            "ship_hot_dog_vendor",
            "ship_orchard_north",
            "ship_orchard_south",
            "ship_orchard_tree",
            "ship_barracks_inventory",
            "ship_grocery_bananas",
            "ship_grocery_oranges",
            "ship_grocery_checkout",
        ] {
            assert_eq!(
                count(label),
                0,
                "profiling baseline must not instantiate {label}"
            );
        }
        assert_eq!(count("ship_barracks_weapon_table"), 1);
        for index in 0..3 {
            assert_eq!(count(&format!("ship_rack_pistol_{index}_grip")), 1);
        }

        assert_eq!(count("ship_convenience_store"), 0);
        assert_eq!(count("ship_liquor_store"), 0);
        assert_eq!(count("ship_mens_washroom_hallway_stub"), 1);
        assert_eq!(count("ship_park_load_zone"), 0);
        assert_eq!(count("ship_park_unload_zone"), 0);
        assert_eq!(count("toilet"), 3);
        assert!(!world.all_components().any(|id| {
            world
                .get_component_by_id_as::<EditorComponent>(id)
                .is_some()
                || world
                    .get_component_by_id_as::<EditorUIComponent>(id)
                    .is_some()
        }));
        assert_eq!(count("ship_washroom_passage"), 1);
        assert_eq!(count("ship_vehicle_biodome"), 0);
        assert_eq!(count("ship_barracks_ceiling_spot"), 3);
        for z in ["0", "6"] {
            for index in 0..3 {
                named(world, &format!("ship_washroom_high_window_{z}_{index}"));
            }
        }
        assert_eq!(count("ship_washroom_mirror_camera_aperture"), 1);
        // Clear window openings must retain collision without alpha panes.
        for label in [
            "ship_bridge_windscreen",
            "ship_park_glass_dome",
            "ship_cafeteria_side_space_window",
        ] {
            let mut pending = vec![named(world, label)];
            while let Some(id) = pending.pop() {
                pending.extend_from_slice(world.children_of(id));
                if let Some(color) = world.get_component_by_id_as::<ColorComponent>(id) {
                    assert_eq!(color.rgba[3], 1.0, "{label} still has an alpha layer");
                }
            }
        }
        // The mirror is a single face, with a camera sightline clear of cubes
        // from its rear surface through the space east of the vanity wall.
        let mirror = named(world, "ship_washroom_wide_mirror");
        assert!(world.children_of(mirror).iter().any(|&id| {
            world
                .get_component_by_id_as::<RenderableComponent>(id)
                .is_some_and(|r| {
                    r.renderable.base_mesh
                        == mittens_engine::engine::graphics::primitives::CpuMeshHandle::QUAD_2D
                })
        }));
        let mirror_model =
            mittens_engine::engine::ecs::system::TransformSystem::world_model(world, mirror)
                .unwrap();
        for distance in [0.08, 0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0] {
            let point = std::array::from_fn::<_, 3, _>(|axis| {
                mirror_model[3][axis] - mirror_model[2][axis] * distance
            });
            for id in world.all_components() {
                if !world
                    .get_component_by_id_as::<RenderableComponent>(id)
                    .is_some_and(|r| {
                        r.renderable.base_mesh
                            == mittens_engine::engine::graphics::primitives::CpuMeshHandle::CUBE
                    })
                {
                    continue;
                }
                let m =
                    mittens_engine::engine::ecs::system::TransformSystem::world_model(world, id)
                        .unwrap();
                let inside = (0..3).all(|axis| {
                    (point[axis] - m[3][axis]).abs()
                        < (0..3).map(|i| m[i][axis].abs() * 0.5).sum::<f32>() - 0.005
                });
                assert!(
                    !inside,
                    "opaque cube {id:?} blocks the mirror camera at {point:?}"
                );
            }
        }
        // Walkable links must be clear at body height, including the turn
        // into the washroom and the cafeteria-to-orchard connection.
        for (y, points) in [
            (
                6.5,
                vec![
                    [8.0, 15.0],
                    [-18.0, 15.0],
                    [-18.0, 11.3],
                    [-20.5, 11.3],
                    [-20.5, 9.0],
                ],
            ),
            (6.5, vec![[8.0, 15.0], [43.0, 15.0]]),
        ] {
            for pair in points.windows(2) {
                for step in 0..=30 {
                    let t = step as f32 / 30.0;
                    let point = [
                        pair[0][0] * (1.0 - t) + pair[1][0] * t,
                        y,
                        pair[0][1] * (1.0 - t) + pair[1][1] * t,
                    ];
                    for id in world.all_components() {
                        if !world.get_component_by_id_as::<RenderableComponent>(id)
                            .is_some_and(|r| r.renderable.base_mesh == mittens_engine::engine::graphics::primitives::CpuMeshHandle::CUBE) { continue; }
                        let matrix =
                            mittens_engine::engine::ecs::system::TransformSystem::world_model(
                                world, id,
                            )
                            .unwrap();
                        let inside = (0..3).all(|axis| {
                            (point[axis] - matrix[3][axis]).abs()
                                < (0..3).map(|i| matrix[i][axis].abs() * 0.5).sum::<f32>() - 0.01
                        });
                        assert!(!inside, "walking link {point:?} blocked by {id:?}");
                    }
                }
            }
        }
        assert_eq!(count("washroom_sink"), 2);
        assert_eq!(count("hand_dryer"), 2);
        assert_eq!(count("paper_towel_dispenser"), 1);
        assert_eq!(count("ship_washroom_recessed_bin"), 1);
        assert_eq!(count("ship_bridge_xr_settings"), 1);
        assert_eq!(count("ship_cafeteria_graphics_settings"), 1);
        assert_eq!(count("ship_space_sky"), 1);
        assert_eq!(count("ship_bridge_windscreen"), 1);
        assert_eq!(count("ship_park_glass_dome"), 1);
        assert_eq!(count("ship_small_golden_stars"), 1);
        assert_eq!(count("ship_cafeteria_side_space_window"), 1);
        for index in 0..3 {
            let window = named(world, &format!("ship_cafeteria_space_window_{index}"));
            assert!(
                world
                    .children_of(window)
                    .iter()
                    .any(|&id| { world.get_component_by_id_as::<ZoneComponent>(id).is_some() }),
                "the window aperture retains its pressure-glass collider"
            );
        }
        let sky_shader = scene
            .world
            .all_components()
            .find_map(|id| {
                let registry = &scene.visuals.materials;
                let definition = registry
                    .definition(registry.source_definition(id)?)?
                    .custom
                    .as_ref()?;
                (definition.fragment_uri == "assets/shaders/ship-space-sky.frag")
                    .then_some(definition)
            })
            .unwrap();
        assert!(
            sky_shader.compiled.get().unwrap().is_ok(),
            "{:?}",
            sky_shader.compiled.get().unwrap()
        );
        let mirror = world
            .get_component_by_id_as::<TransformComponent>(named(world, "ship_washroom_wide_mirror"))
            .unwrap();
        assert!(mirror.scale()[0] > mirror.scale()[1]);
        assert_eq!(
            world
                .all_components()
                .filter_map(|id| world.get_component_by_id_as::<CameraXRComponent>(id))
                .count(),
            usize::from(xr)
        );
        let settings = world
            .all_components()
            .filter_map(|id| world.get_component_by_id_as::<RendererSettingsComponent>(id))
            .last()
            .unwrap();
        assert_eq!(settings.window_visible, desktop);
        for id in world.all_components() {
            if let Some(camera) = world.get_component_by_id_as::<CameraXRComponent>(id) {
                assert_eq!(camera.z_far, 1000.0);
            }
            if let Some(camera) = world.get_component_by_id_as::<Camera3DComponent>(id) {
                assert_eq!(camera.z_far, 1000.0);
            }
        }
        let xr_setting = world
            .all_components()
            .filter_map(|id| world.get_component_by_id_as::<XrComponent>(id))
            .last()
            .unwrap();
        assert_eq!(xr_setting.enabled, xr);

        if !desktop && !xr {
            let mut uploads = Meshes::default();
            scene.systems.implicit_surface.reconcile_and_build(
                &mut scene.world,
                &mut scene.visuals,
                &mut scene.render_assets,
                &mut uploads,
                &mut scene.systems.mesh_bounds,
            );
            for _ in 0..4 {
                scene.update(1.0 / 90.0, &Default::default());
                scene.systems.prepare_render(
                    &mut scene.world,
                    &mut scene.visuals,
                    &mut scene.render_assets,
                    &mut uploads,
                    &mut scene.command_queue,
                );
            }
            // The profiling baseline omits the vendor; rendered architecture
            // must still use the station's local environment preset.
            let materials = &scene.visuals.materials;
            let preset = |label| {
                let source = named(&scene.world, label);
                let record = materials
                    .instance(materials.source_instance(source).unwrap())
                    .unwrap();
                let definition = materials.definition(record.definition).unwrap();
                mittens_engine::engine::graphics::visual_world::AnimeShadingParams::from_material(
                    &definition.parameter_schema,
                    &record.values,
                )
            };
            let environment = preset("ship_environment_anime_material");
            assert!(
                scene
                    .visuals
                    .instances()
                    .iter()
                    .any(|instance| instance.anime_shading(materials) == environment)
            );
            let legacy_defaults = scene
                .visuals
                .instances()
                .iter()
                .filter(|instance| {
                    matches!(instance.renderable.material,
                    mittens_engine::engine::graphics::primitives::MaterialHandle::TOON_MESH |
                    mittens_engine::engine::graphics::primitives::MaterialHandle::SKINNED_TOON_MESH)
                        && !instance
                            .material_instance
                            .and_then(|id| scene.visuals.materials.instance(id))
                            .and_then(|material| {
                                scene.visuals.materials.definition(material.definition)
                            })
                            .is_some_and(|definition| definition.custom.is_some())
                })
                .count();
            assert_eq!(legacy_defaults, 0, "ship defaults must use anime shading");
            let surface = named(&scene.world, "ship_park_rolling_hills");
            let zone = scene
                .world
                .children_of(surface)
                .iter()
                .copied()
                .find(|&id| {
                    scene
                        .world
                        .get_component_by_id_as::<ZoneComponent>(id)
                        .is_some()
                })
                .unwrap();
            let mesh = scene
                .world
                .children_of(zone)
                .iter()
                .find_map(|&id| {
                    scene
                        .world
                        .get_component_by_id_as::<CutGeometryComponent>(id)
                })
                .expect("implicit hill publishes its exact contact mesh");
            assert!(
                mesh.mesh.bounds.min[1] >= -0.081,
                "terrain stays within the upper deck slab"
            );
            assert!(mesh.mesh.bounds.max[1] > 2.3 && mesh.mesh.bounds.max[1] < 2.7);
            let hit = mesh
                .mesh
                .ray([-24.0, 4.0, -16.0], [0.0, -1.0, 0.0])
                .unwrap();
            assert!(
                (4.0 - hit - 2.40).abs() < 0.10,
                "tree grove has a smooth raised ground surface"
            );
            let retained_collision = mesh.mesh.clone();
            let upload_count = uploads.next;
            scene.systems.implicit_surface.reconcile_and_build(
                &mut scene.world,
                &mut scene.visuals,
                &mut scene.render_assets,
                &mut uploads,
                &mut scene.systems.mesh_bounds,
            );
            assert_eq!(
                uploads.next, upload_count,
                "unchanged hills do not rebake or reupload"
            );
            let retained = scene
                .world
                .children_of(zone)
                .iter()
                .find_map(|&id| {
                    scene
                        .world
                        .get_component_by_id_as::<CutGeometryComponent>(id)
                })
                .unwrap();
            assert!(std::sync::Arc::ptr_eq(&retained_collision, &retained.mesh));
        }
    }

    if case != "patrol" {
        return;
    }

    // Advance the small state machine deterministically through both directions.
    let mut scene = Universe::new(World::default());
    scene.load_mms_source_at_path(r#"
        import { lumae_motorcycle_patrol } from "../assets/components/vehicles/lumae_motorcycle_patrol.mms"
        let patrol = lumae_motorcycle_patrol()
        patrol
        for step in range(125) { patrol.advance(0.1) }
        let turn = patrol.status()
        T.position(turn.x, turn.yaw, turn.direction) { name = "patrol_at_first_turn" }
        for step in range(20) { patrol.advance(0.1) }
        let reversed = patrol.status()
        T.position(reversed.x, reversed.yaw, reversed.direction) { name = "patrol_reversed" }
        for step in range(145) { patrol.advance(0.1) }
        let looped = patrol.status()
        T.position(looped.x, looped.yaw, looped.direction) { name = "patrol_looped" }
        patrol.set_running(false)
        patrol.advance(100.0)
    "#, "examples/_ship_motorcycle_patrol_test.mms").unwrap();
    finish_materials(&scene);
    let translation = |label| {
        scene
            .world
            .get_component_by_id_as::<TransformComponent>(named(&scene.world, label))
            .unwrap()
            .translation()
    };
    let first = translation("patrol_at_first_turn");
    let reversed = translation("patrol_reversed");
    let looped = translation("patrol_looped");
    assert!((first[0] + 3.5).abs() < 0.001);
    assert!((reversed[1] + std::f32::consts::FRAC_PI_2).abs() < 0.001);
    assert_eq!(reversed[2], 1.0);
    assert!((looped[0] - 3.5).abs() < 0.001);
    assert_eq!(looped[2], -1.0);
    assert!((looped[1] - std::f32::consts::FRAC_PI_2).abs() < 0.001);
    assert!((translation("ship_lumae_motorcycle_patrol")[0] - looped[0]).abs() < 0.001);
    let mut ancestor = Some(named(&scene.world, "ship_lumae_motorcycle_rider"));
    let mut attached_to_patrol = false;
    while let Some(id) = ancestor {
        attached_to_patrol |=
            scene.world.component_label(id) == Some("ship_lumae_motorcycle_patrol");
        ancestor = scene.world.parent_of(id);
    }
    assert!(attached_to_patrol);
    for side in [-1, 1] {
        named(&scene.world, &format!("motorcycle_handlebar_{side}"));
        named(&scene.world, &format!("motorcycle_footrest_{side}"));
    }
}
