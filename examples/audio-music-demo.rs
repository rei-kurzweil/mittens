// audio-music-demo
//
// Loads `audio-music-demo.mms`, which targets the audio sources directly
// from keyframe blocks. It keeps the old demo's 174 BPM arrangement so the
// Amen break stays in time. The bass `assets/audio/bass-c2.wav` URI may not
// exist on disk — the AudioClip load path is expected to report missing
// assets without crashing the scene.

use mittens_engine::{engine, engine::ecs::SignalEmitter, scripting, utils};

fn main() {
    mittens_engine::example_support::ensure_model_assets();
    utils::logger::init();

    println!("[audio-music-demo] start");

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Minimal camera so the window opens.
    let input = universe
        .world
        .add_component(engine::ecs::component::InputComponent::new().with_speed(2.0));
    let rig = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(0.0, 0.0, 3.0),
    );
    let input_mode = universe.world.add_component(
        engine::ecs::component::InputTransformModeComponent::forward_z().with_roll_axis_y(),
    );
    let cam = universe
        .world
        .add_component(engine::ecs::component::Camera3DComponent::new().with_fov(60.0));
    let _ = universe.attach(input, input_mode);
    let _ = universe.attach(input, rig);
    let _ = universe.attach(rig, cam);
    universe.add(input);

    // Dim clear color so console output stays readable in case of errors.
    let clear = universe
        .world
        .add_component(engine::ecs::component::BackgroundColorComponent::new());
    let clear_c = universe
        .world
        .add_component(engine::ecs::component::ColorComponent::rgba(
            0.06, 0.07, 0.10, 1.0,
        ));
    let _ = universe.world.add_child(clear, clear_c);
    universe.add(clear);

    let source = include_str!("audio-music-demo.mms");
    let output = scripting::MeowMeowRunner::eval_with_world_at_path(
        source,
        Some("examples/audio-music-demo.mms"),
        &mut universe.world,
        &mut universe.systems.rx,
        &mut universe.command_queue,
    );

    for error in &output.errors {
        eprintln!("[mms] {error}");
    }
    println!(
        "[audio-music-demo] mms ok: {} intent(s)",
        output.intents.len()
    );

    for intent in output.intents {
        universe
            .command_queue
            .push_intent_now(engine::ecs::ComponentId::default(), intent);
    }

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.render_assets,
        &mut universe.command_queue,
    );

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
