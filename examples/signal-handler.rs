/// MMS signal handler demo.
///
/// Loads signal-handler.mms, which spawns three colored cubes and registers
/// Click handlers via `on()`. Clicking a cube prints its name to stdout.
///
/// Run: cargo run --release --example signal-handler
use cat_engine::{engine, engine::ecs::SignalEmitter, meow_meow, utils};

#[path = "example_util/mod.rs"]
mod example_util;

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    use engine::ecs::component::{
        AmbientLightComponent, BackgroundColorComponent, Camera3DComponent, ColorComponent,
        DirectionalLightComponent, InputComponent, InputTransformModeComponent, PointerComponent,
        TransformComponent,
    };

    // Background
    let bg = universe.world.add_component(BackgroundColorComponent::new());
    let bg_c = universe.world.add_component(ColorComponent::rgba(0.10, 0.10, 0.14, 1.0));
    let _ = universe.world.add_child(bg, bg_c);
    universe.add(bg);

    // Ambient light
    let ambient = universe.world.add_component(AmbientLightComponent::rgb(0.35, 0.35, 0.38));
    universe.add(ambient);

    // Directional light
    let sun_t = universe.world.add_component(
        TransformComponent::new().with_position(3.0, 5.0, 4.0),
    );
    let sun = universe.world.add_component(DirectionalLightComponent::new());
    let _ = universe.attach(sun_t, sun);
    universe.add(sun_t);

    // Camera + FPS input + pointer
    let input = universe.world.add_component(InputComponent::new().with_speed(3.0));
    let input_mode = universe.world.add_component(
        InputTransformModeComponent::forward_z()
            .with_fps_rotation()
            .with_roll_axis_y(),
    );
    let _ = universe.attach(input, input_mode);

    let cam_t = universe.world.add_component(
        TransformComponent::new().with_position(0.0, 0.5, 4.0),
    );
    let cam = universe.world.add_component(Camera3DComponent::new().with_fov(70.0));
    let pointer = universe.world.add_component(PointerComponent::new());

    let _ = universe.attach(input, cam_t);
    let _ = universe.attach(cam_t, cam);
    let _ = universe.attach(cam_t, pointer);

    example_util::spawn_desktop_camera_controls_hint(&mut universe, cam_t);
    universe.add(input);

    // Evaluate the MMS script with live world access.
    // on() calls inside the script register Click handlers into universe.systems.rx.
    let source = include_str!("signal-handler.mms");
    let output = meow_meow::MeowMeowRunner::eval_with_world(
        source,
        &mut universe.world,
        &mut universe.systems.rx,
        &mut universe.command_queue,
    );

    for err in &output.errors {
        eprintln!("[mms] {err}");
    }
    if !output.errors.is_empty() {
        std::process::exit(1);
    }

    // Any bare CE emissions from the script (not let-bound) are in output.intents.
    // Our script only uses let-bindings so this is empty, but handle it anyway.
    for intent in output.intents {
        universe.command_queue.push_intent_now(engine::ecs::ComponentId::default(), intent);
    }

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
