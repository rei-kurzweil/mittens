use little_cat::{engine, utils};

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Input-driven camera rig.
    // Topology: I { T { C3D }  CN{Rigged { Sphere }} }
    let input = universe
        .world
        .add_component(engine::ecs::component::InputComponent::new().with_speed(2.0));

    let rig_transform = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(0.0, 0.75, 0.0),
    );
    let camera3d = universe
        .world
        .add_component(engine::ecs::component::Camera3DComponent::new());
    let input_mode = universe
        .world
        .add_component(engine::ecs::component::InputTransformModeComponent::forward_z()
            .with_roll_axis_y()
            .with_fps_rotation()
        );

    let rig_collision = universe
        .world
        .add_component(engine::ecs::component::CollisionComponent::RIGGED());
    let rig_shape = universe.world.add_component(engine::ecs::component::CollisionShapeComponent::new(
        engine::ecs::component::CollisionShape::sphere_radius(0.25),
    ));

    let _ = universe.world.add_child(input, input_mode);
    let _ = universe.world.add_child(input, rig_transform);
    let _ = universe.world.add_child(rig_transform, camera3d);
    let _ = universe.world.add_child(rig_transform, rig_collision);
    let _ = universe.world.add_child(rig_collision, rig_shape);

    universe
        .world
        .init_component_tree(input, &mut universe.command_queue);

    // Lights so we can see non-emissive meshes and verify attenuation/direction.
    fn spawn_light(
        universe: &mut engine::Universe,
        x: f32,
        y: f32,
        z: f32,
        r: f32,
        g: f32,
        b: f32,
        intensity: f32,
        distance: f32,
    ) {
        let t = universe.world.add_component(
            engine::ecs::component::TransformComponent::new().with_position(x, y, z),
        );
        let l = universe.world.add_component(
            engine::ecs::component::PointLightComponent::new()
                .with_color(r, g, b)
                .with_intensity(intensity)
                .with_distance(distance),
        );
        let _ = universe.world.add_child(t, l);
        universe.world.init_component_tree(t, &mut universe.command_queue);
    }

    // Perimeter of cubes: 16 per side, 90deg turns.
    const N: i32 = 16;
    let spacing = 1.0;
    let half = (N as f32 - 1.0) * spacing * 0.5;

    // Place 4 very distinct colored lights near corners plus a white overhead fill.
    // (High-ish saturation makes it easy to tell which light is contributing.)
    spawn_light(&mut universe, -half, 2.8, -half, 1.0, 0.1, 0.1, 2.5, 14.0); // red
    spawn_light(&mut universe, half, 2.8, -half, 0.1, 1.0, 0.1, 2.5, 14.0); // green
    spawn_light(&mut universe, half, 2.8, half, 0.1, 0.2, 1.0, 2.5, 14.0); // blue
    spawn_light(&mut universe, -half, 2.8, half, 1.0, 0.2, 1.0, 2.5, 14.0); // magenta-ish
    spawn_light(&mut universe, 0.0, 5.5, 0.0, 1.0, 1.0, 1.0, 0.7, 40.0); // white fill

    fn spawn_wall_cube(universe: &mut engine::Universe, x: f32, y: f32, z: f32) {
        let t = universe.world.add_component(
            engine::ecs::component::TransformComponent::new().with_position(x, y, z),
        );
        let r = universe
            .world
            .add_component(engine::ecs::component::RenderableComponent::cube());
        let cn = universe
            .world
            .add_component(engine::ecs::component::CollisionComponent::STATIC());
        let c = universe
            .world
            .add_component(engine::ecs::component::ColorComponent::rgba(0.9, 0.2, 0.2, 1.0));

        let _ = universe.world.add_child(t, r);
        let _ = universe.world.add_child(t, cn);
        let _ = universe.world.add_child(r, c);

        universe
            .world
            .init_component_tree(t, &mut universe.command_queue);
    }

    fn spawn_cube(
        universe: &mut engine::Universe,
        x: f32,
        y: f32,
        z: f32,
        sx: f32,
        sy: f32,
        sz: f32,
        r: f32,
        g: f32,
        b: f32,
    ) {
        let t = universe.world.add_component(
            engine::ecs::component::TransformComponent::new()
                .with_position(x, y, z)
                .with_scale(sx, sy, sz),
        );
        let renderable = universe
            .world
            .add_component(engine::ecs::component::RenderableComponent::cube());
        let color = universe
            .world
            .add_component(engine::ecs::component::ColorComponent::rgba(r, g, b, 1.0));

        let _ = universe.world.add_child(t, renderable);
        let _ = universe.world.add_child(renderable, color);

        universe.world.init_component_tree(t, &mut universe.command_queue);
    }

    let y = 0.5;

    // Bottom edge (left -> right)
    for i in 0..N {
        let x = -half + (i as f32) * spacing;
        let z = -half;
        spawn_wall_cube(&mut universe, x, y, z);
    }

    // Right edge (bottom -> top), skip first corner
    for i in 1..N {
        let x = half;
        let z = -half + (i as f32) * spacing;
        spawn_wall_cube(&mut universe, x, y, z);
    }

    // Top edge (right -> left), skip first corner
    for i in 1..N {
        let x = half - (i as f32) * spacing;
        let z = half;
        spawn_wall_cube(&mut universe, x, y, z);
    }

    // Left edge (top -> bottom), skip first and last corners
    for i in 1..(N - 1) {
        let x = -half;
        let z = half - (i as f32) * spacing;
        spawn_wall_cube(&mut universe, x, y, z);
    }

    // A few larger cubes outside the perimeter (visual landmarks + lighting test surfaces).
    let outer = half + 3.0;
    spawn_cube(&mut universe, 0.0, 0.75, -outer, 3.0, 1.5, 1.0, 0.15, 0.15, 0.18);
    spawn_cube(&mut universe, 0.0, 0.75, outer, 3.0, 1.5, 1.0, 0.15, 0.15, 0.18);
    spawn_cube(&mut universe, -outer, 0.75, 0.0, 1.0, 1.5, 3.0, 0.15, 0.15, 0.18);
    spawn_cube(&mut universe, outer, 0.75, 0.0, 1.0, 1.5, 3.0, 0.15, 0.15, 0.18);

    // Many small cubes inside the perimeter.
    // Deterministic pseudo-random distribution so the scene is stable.
    let mut seed: u32 = 0xC0111510;
    let mut rng01 = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f32) / (u32::MAX as f32)
    };

    for _ in 0..60 {
        let x = (rng01() * 2.0 - 1.0) * (half - 1.5);
        let z = (rng01() * 2.0 - 1.0) * (half - 1.5);
        let s = 0.15 + rng01() * 0.35;
        let y = 0.5 * s;

        // Slightly varied colors to show multiple light contributions.
        let cr = 0.25 + rng01() * 0.6;
        let cg = 0.25 + rng01() * 0.6;
        let cb = 0.25 + rng01() * 0.6;

        spawn_cube(&mut universe, x, y, z, s, s, s, cr, cg, cb);
    }

    // Process init-time registrations.
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    let user_input = engine::user_input::UserInput::new();
    engine::Windowing::run_app(universe, user_input).expect("Windowing failed");
}
