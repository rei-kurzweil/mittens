use cat_engine::{engine, utils};

#[path = "example_util/mod.rs"]
mod example_util;

fn hash_u32(mut x: u32) -> u32 {
    // A tiny integer hash (deterministic, cheap, less linear than an LCG).
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

fn rand01(seed: u32) -> f32 {
    (hash_u32(seed) as f32) / (u32::MAX as f32)
}

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Dark-ish background clear color so the effect reads.
    let clear = universe
        .world
        .add_component(engine::ecs::component::BackgroundColorComponent::new());
    let clear_c = universe
        .world
        .add_component(engine::ecs::component::ColorComponent::rgba(
            0.01, 0.01, 0.02, 1.0,
        ));
    let _ = universe.world.add_child(clear, clear_c);
    universe.add(clear);

    // --- Camera rig (WASD/QE) ---
    let input = universe
        .world
        .add_component(engine::ecs::component::InputComponent::new().with_speed(2.0));
    let input_mode = universe.world.add_component(
        engine::ecs::component::InputTransformModeComponent::forward_z().with_roll_axis_y(),
    );
    let _ = universe.attach(input, input_mode);

    // Start pulled back so both background + foreground are in view.
    let rig_transform = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(0.0, 1.0, 6.0),
    );
    let _ = universe.attach(input, rig_transform);

    let camera3d = universe
        .world
        .add_component(engine::ecs::component::Camera3DComponent::new());
    let _ = universe.attach(rig_transform, camera3d);

    // Simple light for toon-shaded foreground.
    let light = universe.world.add_component(
        engine::ecs::component::PointLightComponent::new()
            .with_distance(50.0)
            .with_color(1.0, 1.0, 1.0),
    );
    let light_transform = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(0.0, 6.0, 2.0),
    );
    let _ = universe.attach(light_transform, light);

    // Topology: I { T { C3D } } — add a small camera-attached controls hint.
    example_util::spawn_desktop_camera_controls_hint(&mut universe, rig_transform);

    universe.add(input);
    universe.add(light_transform);

    let cube_mesh = universe
        .render_assets
        .get_mesh(engine::graphics::BuiltinMeshType::Cube);

    // --- Background world ---
    // Any renderables under this node will go through the background draw list.
    let bg_root = universe
        .world
        .add_component(engine::ecs::component::BackgroundComponent::new());
    universe.add(bg_root);

    // A large thin "ground" plane in the background layer.
    // (Using a scaled cube for now; visually it's a plane.)
    let ground_tx = universe.world.add_component(
        engine::ecs::component::TransformComponent::new()
            .with_position(0.0, -40.0, 0.0)
            .with_scale(200.0, 1.0, 200.0),
    );
    let ground_renderable =
        universe
            .world
            .add_component(engine::ecs::component::RenderableComponent::new(
                engine::graphics::primitives::Renderable::new(
                    cube_mesh,
                    engine::graphics::primitives::MaterialHandle::UNLIT_MESH,
                ),
            ));
    let ground_color = universe
        .world
        .add_component(engine::ecs::component::ColorComponent::rgba(
            0.015, 0.02, 0.03, 1.0,
        ));

    let _ = universe.attach(bg_root, ground_tx);
    let _ = universe.attach(ground_tx, ground_renderable);
    let _ = universe.attach(ground_renderable, ground_color);

    // Add some bright "stars" (small unlit cubes) scattered on a sphere.
    // Use a grid + jitter + conditional skip so the distribution looks more even.
    let lat_steps: u32 = 24;
    let lon_steps: u32 = 48;
    let density: f32 = 0.14;
    let radius = 25.0;

    for lat in 0..lat_steps {
        for lon in 0..lon_steps {
            let cell = lon + lat * lon_steps;
            if rand01(cell ^ 0x5a1d_c0de) > density {
                continue;
            }

            // Jitter within the cell.
            let jx = rand01(cell ^ 0xA341_316C) - 0.5;
            let jy = rand01(cell ^ 0xC801_3EA4) - 0.5;

            // Latitude in [-pi/2, +pi/2], longitude in [0, 2pi).
            let lat_t = (lat as f32 + 0.5 + 0.9 * jy) / (lat_steps as f32);
            let lon_t = (lon as f32 + 0.5 + 0.9 * jx) / (lon_steps as f32);
            let phi = (lat_t - 0.5) * std::f32::consts::PI; // [-pi/2,+pi/2]
            let theta = lon_t * std::f32::consts::TAU;

            let x = phi.cos() * theta.cos();
            let y = phi.sin();
            let z = phi.cos() * theta.sin();

            let px = x * radius;
            let py = y * radius;
            let pz = z * radius;

            let scale = 0.12 + rand01(cell.wrapping_add(12345)) * 0.20;

            let tx = universe.world.add_component(
                engine::ecs::component::TransformComponent::new()
                    .with_position(px, py, pz)
                    .with_scale(scale, scale, scale),
            );
            let renderable =
                universe
                    .world
                    .add_component(engine::ecs::component::RenderableComponent::new(
                        engine::graphics::primitives::Renderable::new(
                            cube_mesh,
                            engine::graphics::primitives::MaterialHandle::UNLIT_MESH,
                        ),
                    ));

            let c = 0.75 + rand01(cell.wrapping_mul(3)) * 0.25;
            let color = universe
                .world
                .add_component(engine::ecs::component::ColorComponent::rgba(c, c, c, 1.0));

            let _ = universe.attach(bg_root, tx);
            let _ = universe.attach(tx, renderable);
            let _ = universe.attach(renderable, color);
        }
    }

    // --- Foreground world ---
    // A small cube field that should parallax as you move.
    let fg_root = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(0.0, 0.0, 0.0),
    );

    let n: i32 = 8;
    let step: f32 = 0.8;
    for z in 0..n {
        for x in 0..n {
            let px = (x - n / 2) as f32 * step;
            let pz = -(z as f32) * step;

            let tx = universe.world.add_component(
                engine::ecs::component::TransformComponent::new()
                    .with_position(px, 0.5, pz)
                    .with_scale(0.25, 0.25, 0.25),
            );
            let renderable =
                universe
                    .world
                    .add_component(engine::ecs::component::RenderableComponent::new(
                        engine::graphics::primitives::Renderable::new(
                            cube_mesh,
                            engine::graphics::primitives::MaterialHandle::TOON_MESH,
                        ),
                    ));

            let fx = (x as f32) / ((n - 1) as f32);
            let fz = (z as f32) / ((n - 1) as f32);
            let color = universe
                .world
                .add_component(engine::ecs::component::ColorComponent::rgba(
                    0.2 + 0.8 * fx,
                    0.2 + 0.8 * (1.0 - fz),
                    0.4,
                    1.0,
                ));

            let _ = universe.attach(fg_root, tx);
            let _ = universe.attach(tx, renderable);
            let _ = universe.attach(renderable, color);
        }
    }

    universe.add(fg_root);

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &universe.render_assets,
        &mut universe.command_queue,
    );

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
