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
    let rig_transform = universe
        .world
        .register(engine::ecs::component::TransformComponent::new().with_position(0.0, 0.0, 2.5));
    let input_mode = universe.world.register(
        engine::ecs::component::InputTransformModeComponent::forward_z().with_roll_axis_y(),
    );
    let _ = universe.attach(input, input_mode);
    let _ = universe.attach(input, rig_transform);
    let _ = universe.attach(rig_transform, camera3d);
    universe.add(input);

    // Debug square: show the full font texture.
    let debug_root = universe.world.register(
        engine::ecs::component::TransformComponent::new()
            .with_position(0.6, -0.2, 0.0)
            .with_scale(0.8, 0.8, 1.0),
    );
    let debug_renderable = universe
        .world
        .register(engine::ecs::component::RenderableComponent::square());
    let debug_tex = universe
        .world
        .register(engine::ecs::component::TextureComponent::with_uri(
            "assets/textures/font.dds",
        ));
    let debug_filtering = universe
        .world
        .register(engine::ecs::component::TextureFilteringComponent::nearest_magnification());
    let _ = universe.attach(debug_root, debug_renderable);
    let _ = universe.attach(debug_renderable, debug_tex);
    let _ = universe.attach(debug_renderable, debug_filtering);
    universe.add(debug_root);

    // Light so we can actually see non-emissive materials.
    let light_transform = universe
        .world
        .register(engine::ecs::component::TransformComponent::new().with_position(0.0, 0.0, 2.0));
    let light = universe.world.register(
        engine::ecs::component::PointLightComponent::new()
            .with_distance(25.0)
            .with_color(1.0, 1.0, 1.0),
    );
    let _ = universe.attach(light_transform, light);
    universe.add(light_transform);

    // 4 red cubes around the perimeter of the world (easy visual anchors).
    fn spawn_red_cube(universe: &mut engine::Universe, x: f32, y: f32, z: f32, s: f32) {
        let t = universe.world.register(
            engine::ecs::component::TransformComponent::new()
                .with_position(x, y, z)
                .with_scale(s, s, s),
        );
        let r = universe
            .world
            .register(engine::ecs::component::RenderableComponent::cube());
        let c = universe
            .world
            .register(engine::ecs::component::ColorComponent::rgba(
                1.0, 0.0, 0.0, 1.0,
            ));
        let e = universe
            .world
            .register(engine::ecs::component::EmissiveComponent::on());

        let _ = universe.attach(t, r);
        let _ = universe.attach(r, c);
        let _ = universe.attach(r, e);

        universe.add(t);
    }

    let p = 1.5;
    let s = 0.25;
    spawn_red_cube(&mut universe, -p, -p, 0.0, s);
    spawn_red_cube(&mut universe, p, -p, 0.0, s);
    spawn_red_cube(&mut universe, -p, p, 0.0, s);
    spawn_red_cube(&mut universe, p, p, 0.0, s);

    // Text anchor transform (scale down so glyph spacing fits in view).
    let text_root = universe.world.register(
        engine::ecs::component::TransformComponent::new()
            .with_position(-0.9, 0.2, 0.0)
            .with_scale(0.12, 0.12, 1.0),
    );

    let text = universe
        .world
        .register(engine::ecs::component::TextComponent::new("a b c d e f"));
    let _ = universe.attach(text_root, text);

    universe.add(text_root);

    // Process init-time registrations (Text expands into glyph subtrees here).
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    engine::Windowing::run_app(universe).expect("Windowing failed");
}
