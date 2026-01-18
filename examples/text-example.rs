use little_cat::{engine, utils};

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Input-driven camera rig.
    // Topology: I { T { C3D } }
    let input = universe
        .world
        .add_component(engine::ecs::component::InputComponent::new().with_speed(1.5));
    let rig_transform = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(0.0, 0.0, 2.5),
    );
    let camera3d = universe
        .world
        .add_component(engine::ecs::component::Camera3DComponent::new());
    let input_mode = universe
        .world
        .add_component(engine::ecs::component::InputTransformModeComponent::forward_z()
            .with_roll_axis_y()
        );
    let _ = universe.world.add_child(input, input_mode);
    let _ = universe.world.add_child(input, rig_transform);
    let _ = universe.world.add_child(rig_transform, camera3d);
    universe
        .world
        .init_component_tree(input, &mut universe.command_queue);

    // Debug square: show the full font texture.
    let debug_root = universe.world.add_component(
        engine::ecs::component::TransformComponent::new()
            .with_position(0.6, -0.2, 0.0)
            .with_scale(0.8, 0.8, 1.0),
    );
    let debug_renderable = universe
        .world
        .add_component(engine::ecs::component::RenderableComponent::square());
    let debug_tex = universe
        .world
        .add_component(engine::ecs::component::TextureComponent::with_uri(
            "assets/textures/font.dds",
        ));
    let _ = universe.world.add_child(debug_root, debug_renderable);
    let _ = universe.world.add_child(debug_renderable, debug_tex);
    universe
        .world
        .init_component_tree(debug_root, &mut universe.command_queue);

    // Light so we can actually see non-emissive materials.
    let light_transform = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(0.0, 0.0, 2.0),
    );
    let light = universe.world.add_component(
        engine::ecs::component::PointLightComponent::new()
            .with_distance(25.0)
            .with_color(1.0, 1.0, 1.0),
    );
    let _ = universe.world.add_child(light_transform, light);
    universe
        .world
        .init_component_tree(light_transform, &mut universe.command_queue);

    // 4 red cubes around the perimeter of the world (easy visual anchors).
    fn spawn_red_cube(
        universe: &mut engine::Universe,
        x: f32,
        y: f32,
        z: f32,
        s: f32,
    ) {
        let t = universe.world.add_component(
            engine::ecs::component::TransformComponent::new()
                .with_position(x, y, z)
                .with_scale(s, s, s),
        );
        let r = universe
            .world
            .add_component(engine::ecs::component::RenderableComponent::cube());
        let c = universe
            .world
            .add_component(engine::ecs::component::ColorComponent::rgba(1.0, 0.0, 0.0, 1.0));
        let e = universe
            .world
            .add_component(engine::ecs::component::EmissiveComponent::on());

        let _ = universe.world.add_child(t, r);
        let _ = universe.world.add_child(r, c);
        let _ = universe.world.add_child(r, e);

        universe
            .world
            .init_component_tree(t, &mut universe.command_queue);
    }

    let p = 1.5;
    let s = 0.25;
    spawn_red_cube(&mut universe, -p, -p, 0.0, s);
    spawn_red_cube(&mut universe, p, -p, 0.0, s);
    spawn_red_cube(&mut universe, -p, p, 0.0, s);
    spawn_red_cube(&mut universe, p, p, 0.0, s);

    // Text anchor transform (scale down so glyph spacing fits in view).
    let text_root = universe.world.add_component(
        engine::ecs::component::TransformComponent::new()
            .with_position(-0.9, 0.2, 0.0)
            .with_scale(0.12, 0.12, 1.0),
    );

    let text = universe
        .world
        .add_component(engine::ecs::component::TextComponent::new("a b c d e f"));
    let _ = universe.world.add_child(text_root, text);

    universe
        .world
        .init_component_tree(text_root, &mut universe.command_queue);

    // Process init-time registrations (Text expands into glyph subtrees here).
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    let user_input = engine::user_input::UserInput::new();
    engine::Windowing::run_app(universe, user_input).expect("Windowing failed");
}
