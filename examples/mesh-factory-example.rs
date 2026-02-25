use cat_engine::{engine, utils};

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Input-driven camera rig.
    // Topology: I { T { C3D } }
    let input = universe
        .world
        .register(engine::ecs::component::InputComponent::new().with_speed(1.5));
    let camera3d = universe
        .world
        .register(engine::ecs::component::Camera3DComponent::new());
    let rig_transform = universe.world.register(
        engine::ecs::component::TransformComponent::new().with_position(0.0, 0.0, 5.0),
    );
    let input_mode = universe.world.register(
        engine::ecs::component::InputTransformModeComponent::forward_z().with_roll_axis_y(),
    );
    let _ = universe.attach(input, input_mode);
    let _ = universe.attach(input, rig_transform);
    let _ = universe.attach(rig_transform, camera3d);
    universe.add(input);

    // Light.
    let light_transform = universe
        .world
        .register(engine::ecs::component::TransformComponent::new().with_position(0.0, 2.0, 2.0));
    let light = universe.world.register(
        engine::ecs::component::PointLightComponent::new()
            .with_distance(50.0)
            .with_color(1.0, 1.0, 1.0),
    );
    let _ = universe.attach(light_transform, light);
    universe.add(light_transform);

    fn spawn_labeled_mesh(
        universe: &mut engine::Universe,
        x: f32,
        y: f32,
        label: &str,
        mesh: engine::graphics::primitives::CpuMeshHandle,
        scale: [f32; 3],
        color: [f32; 4],
    ) {
        use engine::ecs::component::{
            ColorComponent, EmissiveComponent, RenderableComponent, TextComponent,
            TransformComponent,
        };
        use engine::graphics::primitives::{MaterialHandle, Renderable};

        // Mesh.
        let root = universe.world.register(
            TransformComponent::new()
                .with_position(x, y, 0.0)
                .with_scale(scale[0], scale[1], scale[2]),
        );
        let renderable = universe.world.register(RenderableComponent::new(Renderable::new(
            mesh,
            MaterialHandle::TOON_MESH,
        )));
        let color_c = universe
            .world
            .register(ColorComponent::rgba(color[0], color[1], color[2], color[3]));
        let emissive = universe.world.register(EmissiveComponent::on());

        let _ = universe.attach(root, renderable);
        let _ = universe.attach(renderable, color_c);
        let _ = universe.attach(renderable, emissive);

        universe.add(root);

        // Label (separate transform so we can scale text independently).
        let text_root = universe.world.register(
            TransformComponent::new()
                .with_position(x - 0.55, y + 0.75, 0.0)
                .with_scale(0.09, 0.09, 1.0),
        );
        let text = universe.world.register(TextComponent::new(label));
        let _ = universe.attach(text_root, text);
        universe.add(text_root);
    }

    // Built-in meshes (stable ids).
    let tri = universe
        .render_assets
        .get_mesh(engine::graphics::BuiltinMeshType::Triangle2D);
    let quad = universe
        .render_assets
        .get_mesh(engine::graphics::BuiltinMeshType::Quad2D);
    let cube = universe
        .render_assets
        .get_mesh(engine::graphics::BuiltinMeshType::Cube);
    let tetra = universe
        .render_assets
        .get_mesh(engine::graphics::BuiltinMeshType::Tetrahedron);
    let sphere = universe
        .render_assets
        .get_mesh(engine::graphics::BuiltinMeshType::Sphere);
    let cone = universe
        .render_assets
        .get_mesh(engine::graphics::BuiltinMeshType::Cone);
    let circle = universe
        .render_assets
        .get_mesh(engine::graphics::BuiltinMeshType::Circle2D);

    // Layout.
    let y = 0.0;
    let dx = 1.8;
    let x0 = -dx * 3.0;

    spawn_labeled_mesh(
        &mut universe,
        x0 + dx * 0.0,
        y,
        "Triangle2D\nMeshFactory::triangle_2d()",
        tri,
        [1.0, 1.0, 1.0],
        [1.0, 1.0, 1.0, 1.0],
    );
    spawn_labeled_mesh(
        &mut universe,
        x0 + dx * 1.0,
        y,
        "Quad2D\nMeshFactory::quad_2d()",
        quad,
        [1.0, 1.0, 1.0],
        [1.0, 1.0, 1.0, 1.0],
    );
    spawn_labeled_mesh(
        &mut universe,
        x0 + dx * 2.0,
        y,
        "Cube\nMeshFactory::cube()",
        cube,
        [0.9, 0.9, 0.9],
        [1.0, 1.0, 1.0, 1.0],
    );
    spawn_labeled_mesh(
        &mut universe,
        x0 + dx * 3.0,
        y,
        "Tetrahedron\nMeshFactory::tetrahedron()",
        tetra,
        [1.0, 1.0, 1.0],
        [1.0, 1.0, 1.0, 1.0],
    );
    spawn_labeled_mesh(
        &mut universe,
        x0 + dx * 4.0,
        y,
        "Sphere\nMeshFactory::sphere()",
        sphere,
        [1.0, 1.0, 1.0],
        [1.0, 1.0, 1.0, 1.0],
    );
    spawn_labeled_mesh(
        &mut universe,
        x0 + dx * 5.0,
        y,
        "Cone\nMeshFactory::cone(32)",
        cone,
        [1.0, 1.0, 1.0],
        [1.0, 1.0, 1.0, 1.0],
    );
    spawn_labeled_mesh(
        &mut universe,
        x0 + dx * 6.0,
        y,
        "Circle2D\nMeshFactory::circle_2d(0.45, 0.5, 64)",
        circle,
        [1.0, 1.0, 1.0],
        [1.0, 1.0, 1.0, 1.0],
    );

    // Process init-time registrations (Text expands into glyph subtrees here).
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    engine::Windowing::run_app(universe).expect("Windowing failed");
}
