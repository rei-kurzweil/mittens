use cat_engine::{engine, utils};

fn build_demo_scene_7_shapes(universe: &mut engine::Universe) {
    use engine::ecs::component::{
        Camera3DComponent, ColorComponent, EmissiveComponent, GLTFComponent, InputComponent,
        InputTransformModeComponent, PointLightComponent, RenderableComponent, TextureComponent,
        TransformComponent,
    };
    use engine::graphics::primitives::MaterialHandle;
    use engine::graphics::BuiltinMeshType;

    // Built-in CPU meshes are pre-registered; just fetch stable handles.
    let tri_mesh = universe.render_assets.get_mesh(BuiltinMeshType::Triangle2D);
    let square_mesh = universe.render_assets.get_mesh(BuiltinMeshType::Quad2D);
    let tetra_mesh = universe.render_assets.get_mesh(BuiltinMeshType::Tetrahedron);

    fn spawn(
        world: &mut engine::ecs::World,
        queue: &mut engine::ecs::CommandQueue,
        mesh: engine::graphics::primitives::CpuMeshHandle,
        x: f32,
        y: f32,
        s: f32,
        r: f32,
        color: [f32; 4],
        input_driven: bool,
        emissive: bool,
    ) -> engine::ecs::ComponentId {
        let transform = world.add_component(
            TransformComponent::new()
                .with_position(x, y, 0.0)
                .with_scale(s, s, 1.0)
                .with_rotation_euler(0.0, 0.0, r),
        );
        let renderable = world.add_component(RenderableComponent::new(
            engine::graphics::primitives::Renderable::new(mesh, MaterialHandle::TOON_MESH),
        ));
        let color_c = world.add_component(ColorComponent { rgba: color });

        if emissive {
            let emissive_c = world.add_component(EmissiveComponent::on());
            let _ = world.add_child(renderable, emissive_c);
        }

        // Topology: (optional Input) -> Transform -> Renderable
        let _ = world.add_child(transform, renderable);
        let _ = world.add_child(renderable, color_c);

        if input_driven {
            let input = world.add_component(InputComponent::new().with_speed(0.5));
            let _ = world.add_child(input, transform);
            world.init_component_tree(input, queue);
        } else {
            world.init_component_tree(transform, queue);
        }

        transform
    }

    fn spawn_3d(
        world: &mut engine::ecs::World,
        queue: &mut engine::ecs::CommandQueue,
        mesh: engine::graphics::primitives::CpuMeshHandle,
        x: f32,
        y: f32,
        z: f32,
        s: f32,
        rx: f32,
        ry: f32,
        rz: f32,
        color: [f32; 4],
    ) -> engine::ecs::ComponentId {
        let transform = world.add_component(
            TransformComponent::new()
                .with_position(x, y, z)
                .with_scale(s, s, s)
                .with_rotation_euler(rx, ry, rz),
        );
        let renderable = world.add_component(RenderableComponent::new(
            engine::graphics::primitives::Renderable::new(mesh, MaterialHandle::TOON_MESH),
        ));
        let color_c = world.add_component(ColorComponent { rgba: color });

        let _ = world.add_child(transform, renderable);
        let _ = world.add_child(renderable, color_c);
        world.init_component_tree(transform, queue);

        transform
    }

    // Spawn shapes.
    // One triangle is input-driven (WASD/QE). Build a small "rig" so both the triangle
    // and the camera can be driven by the same InputComponent.

    // Topology: Input -> (InputTransformMode) -> RigTransform -> (CameraTransform -> Camera3D), (TriRootTransform -> ...)
    let tri_input = universe
        .world
        .add_component(InputComponent::new().with_speed(0.5));
    let input_mode = universe
        .world
        .add_component(InputTransformModeComponent::forward_z().with_roll_axis_y());
    let _ = universe.world.add_child(tri_input, input_mode);

    let rig_transform = universe
        .world
        .add_component(TransformComponent::new().with_position(0.0, 0.0, 2.5));
    let _ = universe.world.add_child(tri_input, rig_transform);

    let camera3d = universe.world.add_component(Camera3DComponent::new());
    let _ = universe.world.add_child(rig_transform, camera3d);

    let tri_root_transform = universe
        .world
        .add_component(TransformComponent::new().with_position(0.5, 0.50, 0.0));
    let tri_visual_transform = universe.world.add_component(
        TransformComponent::new()
            .with_scale(0.30, 0.30, 1.0)
            .with_rotation_euler(0.0, 0.0, (2.0 * 3.14159 / 3.0) + 3.14159),
    );
    let tri_renderable = universe.world.add_component(RenderableComponent::new(
        engine::graphics::primitives::Renderable::new(tri_mesh, MaterialHandle::TOON_MESH),
    ));
    let tri_color = universe
        .world
        .add_component(ColorComponent::rgba(0.2, 1.0, 0.2, 1.0));

    let _ = universe.world.add_child(rig_transform, tri_root_transform);
    let _ = universe
        .world
        .add_child(tri_root_transform, tri_visual_transform);
    let _ = universe
        .world
        .add_child(tri_visual_transform, tri_renderable);
    let _ = universe.world.add_child(tri_renderable, tri_color);

    let tri_light = universe.world.add_component(
        PointLightComponent::new()
            .with_distance(10.0)
            .with_color(1.0, 1.0, 1.0),
    );
    let light_transform = universe.world.add_component(
        TransformComponent::new()
            .with_position(0.5, 0.50, 1.0)
            .with_scale(0.1, 0.1, 0.1),
    );
    let _ = universe.world.add_child(light_transform, tri_light);

    universe
        .world
        .init_component_tree(tri_input, &mut universe.command_queue);
    universe
        .world
        .init_component_tree(light_transform, &mut universe.command_queue);

    spawn(
        &mut universe.world,
        &mut universe.command_queue,
        square_mesh,
        -0.80,
        -0.30,
        0.25,
        0.0,
        [1.0, 0.2, 0.2, 1.0],
        false,
        true,
    );
    spawn(
        &mut universe.world,
        &mut universe.command_queue,
        square_mesh,
        -0.40,
        -0.30,
        0.25,
        0.0,
        [1.0, 0.6, 0.2, 1.0],
        false,
        true,
    );
    spawn_3d(
        &mut universe.world,
        &mut universe.command_queue,
        tetra_mesh,
        0.55,
        -0.15,
        0.0,
        0.35,
        0.75,
        0.55,
        0.0,
        [0.2, 0.7, 1.0, 1.0],
    );
    spawn(
        &mut universe.world,
        &mut universe.command_queue,
        square_mesh,
        0.00,
        -0.30,
        0.25,
        0.0,
        [1.0, 1.0, 0.2, 1.0],
        false,
        true,
    );
    spawn(
        &mut universe.world,
        &mut universe.command_queue,
        square_mesh,
        0.40,
        -0.30,
        0.25,
        0.0,
        [0.2, 0.6, 1.0, 1.0],
        false,
        true,
    );
    spawn(
        &mut universe.world,
        &mut universe.command_queue,
        square_mesh,
        0.80,
        -0.30,
        0.25,
        0.0,
        [0.8, 0.2, 1.0, 1.0],
        false,
        true,
    );
    spawn(
        &mut universe.world,
        &mut universe.command_queue,
        tri_mesh,
        0.30,
        0.35,
        0.30,
        -3.14159,
        [1.0, 1.0, 1.0, 1.0],
        false,
        false,
    );

    let tex_transform = universe.world.add_component(
        TransformComponent::new()
            .with_position(0.0, 0.1, 0.0)
            .with_scale(0.45, 0.45, 1.0),
    );
    let tex_renderable = universe.world.add_component(RenderableComponent::new(
        engine::graphics::primitives::Renderable::new(square_mesh, MaterialHandle::TOON_MESH),
    ));
    let tex_color = universe
        .world
        .add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));
    let tex = universe.world.add_component(TextureComponent::from_dds(
        "assets/textures/cat-face-amused.dds",
    ));

    let _ = universe.world.add_child(tex_transform, tex_renderable);
    let _ = universe.world.add_child(tex_renderable, tex_color);
    let _ = universe.world.add_child(tex_renderable, tex);
    universe
        .world
        .init_component_tree(tex_transform, &mut universe.command_queue);

    let cat_anchor = universe.world.add_component(
        TransformComponent::new()
            .with_position(0.0, -0.10, -4.0)
            .with_scale(0.50, 0.50, 0.50)
            .with_rotation_euler(0.0, 0.0, 0.0),
    );
    let cat_gltf = universe
        .world
        .add_component(GLTFComponent::new("assets/models/color-cat.2.glb"));
    let _ = universe.world.add_child(cat_anchor, cat_gltf);
    universe
        .world
        .init_component_tree(cat_anchor, &mut universe.command_queue);
}

fn main() {
    utils::logger::init();

    // Parse CLI arguments
    let cli = engine::CLI::parse();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Handle load command before building scene
    if let engine::cli::CliCommand::Load { ref filename } = cli.command {
        println!("[CLI] Loading scene from '{}'...", filename);
        match engine::ecs::ComponentCodec::decode_scene(&mut universe.world, filename) {
            Ok(root_ids) => {
                println!(
                    "[CLI] Scene loaded successfully. {} root(s) loaded.",
                    root_ids.len()
                );
                // Initialize all loaded component trees
                for root_id in root_ids {
                    universe
                        .world
                        .init_component_tree(root_id, &mut universe.command_queue);
                }
                // Process any init commands
                universe.systems.process_commands(
                    &mut universe.world,
                    &mut universe.visuals,
                    &mut universe.command_queue,
                );
            }
            Err(e) => {
                eprintln!("[CLI] Failed to load scene: {}", e);
                eprintln!("[CLI] Building demo scene instead...");
                build_demo_scene_7_shapes(&mut universe);
            }
        }
    } else {
        // Build demo scene if not loading
        build_demo_scene_7_shapes(&mut universe);
    }

    // Handle save command after scene is built
    if let engine::cli::CliCommand::Save { ref filename } = cli.command {
        println!("[CLI] Saving scene to '{}'...", filename);

        // Find all root components (components with no parent)
        let root_components: Vec<engine::ecs::ComponentId> = universe
            .world
            .all_components()
            .filter(|&cid| universe.world.parent_of(cid).is_none())
            .collect();

        if root_components.is_empty() {
            eprintln!("[CLI] No root components found to save.");
        } else {
            println!("[CLI] Found {} root component(s)", root_components.len());

            match engine::ecs::ComponentCodec::encode_scene(
                &universe.world,
                &root_components,
                filename,
            ) {
                Ok(()) => println!(
                    "[CLI] Saved {} roots to '{}'",
                    root_components.len(),
                    filename
                ),
                Err(e) => eprintln!("[CLI] Failed to save scene: {}", e),
            }
        }

        // Exit after saving (don't run the window)
        println!("[CLI] Save complete. Exiting.");
        return;
    }

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
