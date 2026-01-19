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
            .with_roll_axis_y());

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

    // Light so we can see non-emissive meshes.
    let light_transform = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(0.0, 2.0, 0.0),
    );
    let light = universe.world.add_component(
        engine::ecs::component::PointLightComponent::new()
            .with_distance(180.0)
            .with_color(1.0, 1.0, 1.0),
    );
    let _ = universe.world.add_child(light_transform, light);
    universe
        .world
        .init_component_tree(light_transform, &mut universe.command_queue);

    // Perimeter of cubes: 16 per side, 90deg turns.
    const N: i32 = 16;
    let spacing = 1.0;
    let half = (N as f32 - 1.0) * spacing * 0.5;

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

    // Process init-time registrations.
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    let user_input = engine::user_input::UserInput::new();
    engine::Windowing::run_app(universe, user_input).expect("Windowing failed");
}
