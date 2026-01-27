use little_cat::engine::ecs::component::{
    AmbientLightComponent, BackgroundColorComponent, Camera3DComponent, ColorComponent,
    GLTFComponent, InputComponent, InputTransformModeComponent, RenderableComponent,
    TransformComponent,
};
use little_cat::{engine, utils};

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Light pink background.
    let background = universe
        .world
        .add_component(BackgroundColorComponent::rgba(1.0, 0.82, 0.90, 1.0));
    universe
        .world
        .init_component_tree(background, &mut universe.command_queue);

    // Small ambient so shadowed areas aren't pure black.
    let ambient = universe
        .world
        .add_component(AmbientLightComponent::rgb(0.10, 0.10, 0.12));
    universe
        .world
        .init_component_tree(ambient, &mut universe.command_queue);

    // --- Camera rig (WASD + mouse) ---
    // InputComponent is the root, and it owns a Transform (the camera rig).
    let input = universe
        .world
        .add_component(InputComponent::new().with_speed(1.5));
    let input_mode = universe
        .world
        .add_component(InputTransformModeComponent::forward_z().with_fps_rotation());
    let _ = universe.world.add_child(input, input_mode);

    // Start slightly pulled back looking towards the origin.
    let rig_transform = universe
        .world
        .add_component(TransformComponent::new().with_position(0.0, 0.0, 6.0));
    let _ = universe.world.add_child(input, rig_transform);

    let camera3d = universe.world.add_component(Camera3DComponent::new());
    let _ = universe.world.add_child(rig_transform, camera3d);

    universe
        .world
        .init_component_tree(input, &mut universe.command_queue);

    // --- lighting ---
    let light_transform = universe.world.add_component(
        TransformComponent::new()
            .with_position(1.0, 6.0, 3.0)
            .with_scale(0.1, 0.1, 0.1),
    );
    let light = universe.world.add_component(
        engine::ecs::component::PointLightComponent::new()
            .with_distance(120.0)
            .with_color(1.0, 1.0, 1.0),
    );
    let _ = universe.world.add_child(light_transform, light);
    universe
        .world
        .init_component_tree(light_transform, &mut universe.command_queue);

    // --- VTuber model ---
    let model_root = universe.world.add_component(TransformComponent::new());
    let model = universe
        .world
        .add_component(GLTFComponent::new("assets/models/pc-rei.hoodie.glb"));
    // emissive for pc-rei
    let emissive = universe.world.add_component(
        engine::ecs::component::EmissiveComponent { enabled: true });

    let _ = universe.world.add_child(model, emissive);

    let _ = universe.world.add_child(model_root, model);
    universe
        .world
        .init_component_tree(model_root, &mut universe.command_queue);

    // --- Simple environment ---
    let spawn_cube = |universe: &mut engine::Universe,
                        position: (f32, f32, f32),
                        scale: (f32, f32, f32),
                        color: (f32, f32, f32, f32)| {
        let transform = universe.world.add_component(
            TransformComponent::new()
                .with_position(position.0, position.1, position.2)
                .with_scale(scale.0, scale.1, scale.2),
        );
        let renderable = universe.world.add_component(RenderableComponent::cube());
        let color = universe
            .world
            .add_component(ColorComponent::rgba(color.0, color.1, color.2, color.3));

        let _ = universe.world.add_child(transform, renderable);
        let _ = universe.world.add_child(renderable, color);

        universe
            .world
            .init_component_tree(transform, &mut universe.command_queue);
    };

    // floor
    spawn_cube(
        &mut universe,
        (0.0, -0.05, 0.0),
        (10.0, 0.1, 10.0),
        (0.92, 0.92, 0.92, 1.0),
    );

    // back wall
    spawn_cube(
        &mut universe,
        (0.0, 1.5, -5.0),
        (10.0, 3.0, 1.0),
        (0.95, 0.94, 0.96, 1.0),
    );

    // desk
    spawn_cube(
        &mut universe,
        (0.0, 0.45, 1.0),
        (2.0, 0.9, 1.0),
        (0.75, 0.70, 0.65, 1.0),
    );

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    let user_input = engine::user_input::UserInput::new();
    universe.enable_repl();
    engine::Windowing::run_app(universe, user_input).expect("Windowing failed");
}
