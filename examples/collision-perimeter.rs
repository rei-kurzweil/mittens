use cat_engine::{engine, utils};

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let bg_color = universe
        .world
        .register(engine::ecs::component::BackgroundColorComponent::rgba(
            0.1, 0.02, 0.05, 1.0,
        ));

    universe.add(bg_color);

    // Gravity field for the pushable cubes.
    // Any KineticResponseComponent nested under this subtree will have gravity applied.
    let gravity_field = universe
        .world
        .register(engine::ecs::component::GravityComponent::new().with_coefficient(0.5));
    universe.add(gravity_field);

    // Input-driven camera rig.
    // Topology: I { T { C3D }  CN{Rigged { Sphere }} }
    let input = universe
        .world
        .register(engine::ecs::component::InputComponent::new().with_speed(2.0));

    let rig_transform = universe
        .world
        .register(engine::ecs::component::TransformComponent::new().with_position(0.0, 0.75, 0.0));
    let camera3d = universe
        .world
        .register(engine::ecs::component::Camera3DComponent::new());
    let input_mode = universe.world.register(
        engine::ecs::component::InputTransformModeComponent::forward_z()
            .with_roll_axis_y()
            .with_fps_rotation(),
    );

    let rig_collision = universe
        .world
        .register(engine::ecs::component::CollisionComponent::RIGGED());

    // Opt-in to default kinematic-vs-static collision response.
    // Policy note: collisions still emit signals regardless; response only runs for entities
    // that explicitly add a KineticResponseComponent.
    let rig_response = universe
        .world
        .register(engine::ecs::component::KineticResponseComponent::slide());
    let rig_shape = universe
        .world
        .register(engine::ecs::component::CollisionShapeComponent::new(
            engine::ecs::component::CollisionShape::sphere_radius(0.25),
        ));

    let _ = universe.attach(input, input_mode);
    let _ = universe.attach(input, rig_transform);
    let _ = universe.attach(rig_transform, camera3d);

    // Click-to-pick: raycast from the active camera through the cursor.
    let raycast = universe.world.register(
        engine::ecs::component::RayCastComponent::event_driven().with_max_distance(100.0),
    );
    let _ = universe.attach(rig_transform, raycast);

    let _ = universe.attach(rig_transform, rig_collision);
    let _ = universe.attach(rig_collision, rig_response);
    let _ = universe.attach(rig_collision, rig_shape);

    universe.add(input);

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
        let t = universe.world.register(
            engine::ecs::component::TransformComponent::new()
                .with_position(x, y, z)
                .with_scale(0.12, 0.12, 0.12),
        );

        let l = universe.world.register(
            engine::ecs::component::PointLightComponent::new()
                .with_color(r, g, b)
                .with_intensity(intensity)
                .with_distance(distance),
        );
        // Visual marker: a small white cube at the light position.
        // White ensures it mostly shows the light color.
        let marker = universe
            .world
            .register(engine::ecs::component::RenderableComponent::cube());
        let marker_color = universe
            .world
            .register(engine::ecs::component::ColorComponent::rgba(
                1.0, 1.0, 1.0, 1.0,
            ));

        let _ = universe.attach(t, l);
        let _ = universe.attach(t, marker);
        let _ = universe.attach(marker, marker_color);
        universe.add(t);
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
        let t = universe
            .world
            .register(engine::ecs::component::TransformComponent::new().with_position(x, y, z));
        let r = universe
            .world
            .register(engine::ecs::component::RenderableComponent::cube());
        let cn = universe
            .world
            .register(engine::ecs::component::CollisionComponent::STATIC());
        let c = universe
            .world
            .register(engine::ecs::component::ColorComponent::rgba(
                0.9, 0.2, 0.2, 1.0,
            ));

        let _ = universe.attach(t, r);
        let _ = universe.attach(t, cn);
        let _ = universe.attach(r, c);

        universe.add(t);
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
        let t = universe.world.register(
            engine::ecs::component::TransformComponent::new()
                .with_position(x, y, z)
                .with_scale(sx, sy, sz),
        );
        let renderable = universe
            .world
            .register(engine::ecs::component::RenderableComponent::cube());
        let color = universe
            .world
            .register(engine::ecs::component::ColorComponent::rgba(r, g, b, 1.0));

        let _ = universe.attach(t, renderable);
        let _ = universe.attach(renderable, color);

        universe.add(t);
    }

    fn spawn_pushable_cube(
        universe: &mut engine::Universe,
        parent: engine::ecs::ComponentId,
        x: f32,
        y: f32,
        z: f32,
        s: f32,
        r: f32,
        g: f32,
        b: f32,
    ) {
        let t = universe.world.register(
            engine::ecs::component::TransformComponent::new()
                .with_position(x, y, z)
                .with_scale(s, s, s),
        );
        let renderable = universe
            .world
            .register(engine::ecs::component::RenderableComponent::cube());
        let color = universe
            .world
            .register(engine::ecs::component::ColorComponent::rgba(r, g, b, 1.0));

        let cn = universe
            .world
            .register(engine::ecs::component::CollisionComponent::KINEMATIC());
        let response = universe.world.register(
            engine::ecs::component::KineticResponseComponent::push()
                .with_push_strength(4.0)
                .with_friction_y(10.0),
        );
        let shape = universe
            .world
            .register(engine::ecs::component::CollisionShapeComponent::new(
                engine::ecs::component::CollisionShape::cube_half_extents([
                    0.5 * s,
                    0.5 * s,
                    0.5 * s,
                ]),
            ));

        let _ = universe.attach(t, renderable);
        let _ = universe.attach(renderable, color);

        let _ = universe.attach(t, cn);
        let _ = universe.attach(cn, response);
        let _ = universe.attach(cn, shape);

        let _ = universe.attach(parent, t);
    }

    fn spawn_invisible_static_wall(
        universe: &mut engine::Universe,
        x: f32,
        y: f32,
        z: f32,
        half_extents: [f32; 3],
    ) {
        let t = universe
            .world
            .register(engine::ecs::component::TransformComponent::new().with_position(x, y, z));

        let cn = universe
            .world
            .register(engine::ecs::component::CollisionComponent::STATIC());
        let shape = universe
            .world
            .register(engine::ecs::component::CollisionShapeComponent::new(
                engine::ecs::component::CollisionShape::cube_half_extents(half_extents),
            ));

        // Requested: attach kinematic_response as well (even though STATIC colliders
        // are not responders).
        let response = universe
            .world
            .register(engine::ecs::component::KineticResponseComponent::slide());

        let _ = universe.attach(t, cn);
        let _ = universe.attach(cn, shape);
        let _ = universe.attach(cn, response);
        universe.add(t);
    }

    let y = 0.5;

    // Static ground plane (thin cube). Top surface at y=0.
    {
        let ground_half = half + 2.0;
        let thickness = 0.20;

        // Split transforms so scaling the ground does not scale any potential children.
        // Top surface at y=0.
        let ground_root_t = universe
            .world
            .register(engine::ecs::component::TransformComponent::new());
        let ground_geom_t = universe.world.register(
            engine::ecs::component::TransformComponent::new()
                .with_position(0.0, -thickness, 0.0)
                .with_scale(ground_half * 2.0, thickness * 2.0, ground_half * 2.0),
        );
        let ground_r = universe
            .world
            .register(engine::ecs::component::RenderableComponent::cube());
        let ground_c = universe
            .world
            .register(engine::ecs::component::ColorComponent::rgba(
                0.08, 0.08, 0.09, 1.0,
            ));
        let ground_cn = universe
            .world
            .register(engine::ecs::component::CollisionComponent::STATIC());
        let ground_shape =
            universe
                .world
                .register(engine::ecs::component::CollisionShapeComponent::new(
                    engine::ecs::component::CollisionShape::cube_half_extents([
                        ground_half,
                        thickness,
                        ground_half,
                    ]),
                ));

        let _ = universe.attach(ground_root_t, ground_geom_t);
        let _ = universe.attach(ground_geom_t, ground_r);
        let _ = universe.attach(ground_r, ground_c);
        let _ = universe.attach(ground_geom_t, ground_cn);
        let _ = universe.attach(ground_cn, ground_shape);
        universe.add(ground_root_t);
    }

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
    spawn_cube(
        &mut universe,
        0.0,
        0.75,
        -outer,
        3.0,
        1.5,
        1.0,
        0.15,
        0.15,
        0.18,
    );
    spawn_cube(
        &mut universe,
        0.0,
        0.75,
        outer,
        3.0,
        1.5,
        1.0,
        0.15,
        0.15,
        0.18,
    );
    spawn_cube(
        &mut universe,
        -outer,
        0.75,
        0.0,
        1.0,
        1.5,
        3.0,
        0.15,
        0.15,
        0.18,
    );
    spawn_cube(
        &mut universe,
        outer,
        0.75,
        0.0,
        1.0,
        1.5,
        3.0,
        0.15,
        0.15,
        0.18,
    );

    // Outer containment walls (invisible): keep runaway cubes near the scene.
    // Collision shapes are in world units; transform scale is irrelevant for collision.
    {
        let wall_center_y = 6.0; // bottom at y=0, top at y=12
        let wall_half_height = 6.0;
        let wall_half_thickness = 0.25;
        let wall_offset = half + 6.0;
        let wall_half_len = wall_offset + 2.0;

        // +Z / -Z walls
        spawn_invisible_static_wall(
            &mut universe,
            0.0,
            wall_center_y,
            wall_offset,
            [wall_half_len, wall_half_height, wall_half_thickness],
        );
        spawn_invisible_static_wall(
            &mut universe,
            0.0,
            wall_center_y,
            -wall_offset,
            [wall_half_len, wall_half_height, wall_half_thickness],
        );

        // +X / -X walls
        spawn_invisible_static_wall(
            &mut universe,
            wall_offset,
            wall_center_y,
            0.0,
            [wall_half_thickness, wall_half_height, wall_half_len],
        );
        spawn_invisible_static_wall(
            &mut universe,
            -wall_offset,
            wall_center_y,
            0.0,
            [wall_half_thickness, wall_half_height, wall_half_len],
        );

        // Roof
        spawn_invisible_static_wall(
            &mut universe,
            0.0,
            wall_center_y + wall_half_height + wall_half_thickness,
            0.0,
            [wall_half_len, wall_half_thickness, wall_half_len],
        );
    }

    // Many small cubes inside the perimeter.
    // Deterministic pseudo-random distribution so the scene is stable.
    let mut seed: u32 = 0xC0111510;
    let mut rng01 = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f32) / (u32::MAX as f32)
    };

    // 3x more cubes, with three distinct size bands.
    for band in 0..3 {
        for _ in 0..60 {
            let s = match band {
                0 => 0.12 + rng01() * 0.20, // small
                1 => 0.35 + rng01() * 0.35, // medium
                _ => 0.75 + rng01() * 0.55, // large
            };

            let bound = (half - (1.0 + s)).max(0.5);
            let x = (rng01() * 2.0 - 1.0) * bound;
            let z = (rng01() * 2.0 - 1.0) * bound;
            let y = 0.5 * s;

            // Slightly varied colors to show multiple light contributions.
            let cr = 0.25 + rng01() * 0.6;
            let cg = 0.25 + rng01() * 0.6;
            let cb = 0.25 + rng01() * 0.6;

            spawn_pushable_cube(&mut universe, gravity_field, x, y, z, s, cr, cg, cb);
        }
    }

    // Process init-time registrations.
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    engine::Windowing::run_app(universe).expect("Windowing failed");
}
