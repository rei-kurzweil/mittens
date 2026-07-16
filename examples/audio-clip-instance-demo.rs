// audio-clip-instance-demo
//
// Cloned from `audio-music-demo`. Same scene, but demonstrates
// `.instance()` on a live AudioClip handle: two extra voices share the
// AmenBreak decoded buffer, each starting at a different point in the
// sample (0.25 / 0.5 beats in). The keyframes target those clone
// handles directly instead of routing through MusicContext voice names.
//
// See docs/draft/audio-clip-instance-cloning.md.

use mittens_engine::{engine, engine::ecs::SignalEmitter, scripting, utils};

fn main() {
    utils::logger::init();

    println!("[audio-clip-instance-demo] start");

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

    let source = include_str!("audio-clip-instance-demo.mms");
    let output = scripting::MeowMeowRunner::eval_with_world_at_path(
        source,
        Some("examples/audio-clip-instance-demo.mms"),
        &mut universe.world,
        &mut universe.systems.rx,
        &mut universe.command_queue,
    );

    for error in &output.errors {
        eprintln!("[mms] {error}");
    }
    println!(
        "[audio-clip-instance-demo] mms ok: {} intent(s)",
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
