use cat_engine::engine::ecs::component::{
    AmbientLightComponent, BackgroundColorComponent, Camera3DComponent, CameraXRComponent,
    ColorComponent, DirectionalLightComponent, GLTFComponent, InputComponent,
    InputTransformModeComponent, InputXRComponent, RenderableComponent, TransformComponent,
};
use cat_engine::{engine, utils};

#[path = "example_util/mod.rs"]
mod example_util;

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Light pink background.
    let background = universe
        .world
        .add_component(BackgroundColorComponent::new());
    let background_c = universe
        .world
        .add_component(ColorComponent::rgba(1.0, 0.82, 0.90, 1.0));
    let _ = universe.world.add_child(background, background_c);
    universe.add(background);

    // Small ambient so shadowed areas aren't pure black.
    let ambient = universe
        .world
        .add_component(AmbientLightComponent::rgb(0.10, 0.10, 0.12));
    universe.add(ambient);

    // --- Camera rig (WASD + mouse) ---
    // InputComponent is the root, and it owns a Transform (the camera rig).
    let input = universe
        .world
        .add_component(InputComponent::new().with_speed(1.5));
    let input_mode = universe.world.add_component(
        InputTransformModeComponent::forward_z()
            .with_fps_rotation()
            .with_roll_axis_y(),
    );
    let _ = universe.attach(input, input_mode);

    // Start slightly pulled back looking towards the origin.
    let rig_transform = universe
        .world
        .add_component(TransformComponent::new().with_position(0.0, 0.0, 6.0));
    let _ = universe.attach(input, rig_transform);

    let camera3d = universe.world.add_component(Camera3DComponent::new());
    let _ = universe.attach(rig_transform, camera3d);

    // Topology: I { T { C3D } } — add a small camera-attached controls hint.
    example_util::spawn_desktop_camera_controls_hint(&mut universe, rig_transform);

    universe.add(input);

    // --- lighting ---
    // Directional key light (slightly down + forward Z).
    let sun = universe.world.add_component(
        DirectionalLightComponent::new()
            .with_intensity(1.2)
            .with_color(1.0, 0.98, 0.94),
    );
    let sun_dir = universe
        .world
        .add_component(TransformComponent::new().with_position(0.0, -0.35, 1.0));
    let _ = universe.attach(sun_dir, sun);
    universe.add(sun_dir);

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
    let _ = universe.attach(light_transform, light);
    universe.add(light_transform);

    // --- VTuber model ---
    let model_root = universe.world.add_component(TransformComponent::new());
    let model = universe
        .world
        .add_component(GLTFComponent::new("assets/models/pc-rei.hoodie.glb"));
    // emissive for pc-rei
    let emissive = universe
        .world
        .add_component(engine::ecs::component::EmissiveComponent::on());

    let _ = universe.attach(model, emissive);

    let xr_input = universe.world.add_component(InputXRComponent::on());
    let xr_head = universe.world.add_component(TransformComponent::new());
    let xr_camera = universe.world.add_component(CameraXRComponent::on());
    let _ = universe.attach(xr_input, xr_head);
    let _ = universe.attach(xr_head, xr_camera);
    let _ = universe.attach(xr_head, model_root);

    let _ = universe.attach(model_root, model);
    universe.add(xr_input);
    universe.add(model_root);

    // --- Background clouds (occluded + lit) ---
    let bg_root = universe.world.add_component(
        engine::ecs::component::BackgroundComponent::new().with_occlusion_and_lighting(),
    );
    universe.add(bg_root);
    let mut cloud_params = example_util::CloudRingParams::default();
    cloud_params.cloud_count = 8; // +3 clusters
    cloud_params.angle_jitter = 0.35;
    cloud_params.high_y_probability = 0.5;
    cloud_params.high_y_multiplier = 1.5;
    cloud_params.seed = 0x57_55_B0_01u32;
    example_util::spawn_cloud_ring(&mut universe, bg_root, cloud_params);

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

        let _ = universe.attach(transform, renderable);
        let _ = universe.attach(renderable, color);

        universe.add(transform);
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
        (0.0, 0.35, 1.0),
        (1.0, 0.75, 0.5),
        (0.75, 0.70, 0.65, 1.0),
    );

    let xr_root = universe
        .world
        .add_component(engine::ecs::component::XrComponent::on());
    universe.add(xr_root);

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.render_assets,
        &mut universe.command_queue,
    );

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
