use cat_engine::{engine, utils};

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let background =
        universe
            .world
            .register(engine::ecs::component::BackgroundColorComponent::rgba(
                0.3, 0.1, 1.0, 1.0,
            ));

    universe.add(background);

    // --- Camera rig (WASD/QE) ---
    // Keep this similar to the main demo so we can fly around the cube field.
    let input = universe
        .world
        .register(engine::ecs::component::InputComponent::new().with_speed(1.5));
    let input_mode = universe.world.register(
        engine::ecs::component::InputTransformModeComponent::forward_z().with_roll_axis_y(),
    );
    let _ = universe.attach(input, input_mode);

    // Start pulled back so the grid is in view.
    let rig_transform = universe.world.register(
        engine::ecs::component::TransformComponent::new().with_position(0.0, 0.0, 4.0),
    );
    let _ = universe.attach(input, rig_transform);

    let camera3d = universe
        .world
        .register(engine::ecs::component::Camera3DComponent::new());
    let _ = universe.attach(rig_transform, camera3d);

    // Simple point light so the toon shader reads well.
    let light = universe.world.register(
        engine::ecs::component::PointLightComponent::new()
            .with_distance(50.0)
            .with_color(1.0, 1.0, 1.0),
    );
    let light_transform = universe.world.register(
        engine::ecs::component::TransformComponent::new().with_position(0.0, 5.0, 2.0),
    );
    let _ = universe.attach(light_transform, light);

    universe.add(input);
    universe.add(light_transform);

    // --- 16x16x16 cube grid ---
    let cube_mesh = universe
        .render_assets
        .get_mesh(engine::graphics::BuiltinMeshType::Cube);

    let n: usize = 32;
    let cube_scale: f32 = 0.10;
    let gap: f32 = 0.50;
    let step: f32 = cube_scale + gap;

    // Center positions around 0 by subtracting half the extent (in steps).
    let half_extent_x = (n as f32 - 1.0) * step * 0.5;
    let half_extent_y = (n as f32 - 1.0) * step * 0.5;
    let half_extent_z = (n as f32 - 1.0) * step * 0.5;

    // Move the whole container up/back based on its content size, plus the requested offsets.
    // - up by +0.5 and by half the content height
    // - back by -(0.5 + 1.0) and by half the content depth
    let container_offset_y = half_extent_y + 0.5;
    let container_offset_z = -(half_extent_z + 1.0 + 0.5);

    let container = universe.world.register(
        engine::ecs::component::TransformComponent::new().with_position(
            0.0,
            container_offset_y,
            container_offset_z,
        ),
    );

    for z in 0..n {
        for y in 0..n {
            for x in 0..n {
                let px = x as f32 * step - half_extent_x;
                let py = y as f32 * step - half_extent_y;
                let pz = z as f32 * step - half_extent_z;

                let tx = universe.world.register(
                    engine::ecs::component::TransformComponent::new()
                        .with_position(px, py, pz)
                        .with_scale(cube_scale, cube_scale, cube_scale),
                );
                let renderable =
                    universe
                        .world
                        .register(engine::ecs::component::RenderableComponent::new(
                            engine::graphics::primitives::Renderable::new(
                                cube_mesh,
                                engine::graphics::primitives::MaterialHandle::TOON_MESH,
                            ),
                        ));

                let denom = (n - 1) as f32;
                let color = engine::ecs::component::ColorComponent::rgba(
                    f32::sin(x as f32 / 10.0),
                    y as f32 / denom,
                    z as f32 / denom,
                    1.0,
                );
                let color_c = universe.world.register(color);

                let _ = universe.attach(container, tx);
                let _ = universe.attach(tx, renderable);
                let _ = universe.attach(renderable, color_c);
            }
        }
    }

    universe.add(container);
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    // Add an OpenXR component so OpenXRSystem initializes and starts polling events.
    let xr_root = universe
        .world
        .register(engine::ecs::component::OpenXRComponent::on());
    universe.add(xr_root);
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    let user_input = engine::user_input::UserInput::new();
    universe.enable_repl();
    engine::Windowing::run_app(universe, user_input).expect("Windowing failed");
}
