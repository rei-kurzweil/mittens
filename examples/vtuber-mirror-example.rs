use cat_engine::engine::ecs::SignalEmitter;
use cat_engine::{engine, meow_meow, utils};

#[path = "example_util/mod.rs"]
mod example_util;

fn main() {
    utils::logger::init();

    let output = meow_meow::MeowMeowRunner::eval(include_str!("vtuber-mirror-example.mms"));

    for error in &output.errors {
        eprintln!("[mms] {error}");
    }
    println!(
        "[mms] {} intent(s) from vtuber-mirror-example.mms",
        output.intents.len()
    );
    println!(
        "[vtuber-mirror-example] expected XR-only views: 2 XR eye views plus mirror-derived views; no desktop scene view unless a Camera3D/Camera2D is added"
    );

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let scope = engine::ecs::ComponentId::default();
    for intent in output.intents {
        universe.command_queue.push_intent_now(scope, intent);
    }

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &universe.render_assets,
        &mut universe.command_queue,
    );

    let bg_root = universe.world.add_component(
        engine::ecs::component::BackgroundComponent::new().with_occlusion_and_lighting(),
    );
    universe.add(bg_root);

    let cloud_params = example_util::CloudRingParams {
        cloud_count: 10,
        radius: 34.0,
        center_y: 8.5,
        puffs_per_cloud: 28,
        angle_jitter: 0.30,
        high_y_probability: 0.45,
        high_y_multiplier: 1.28,
        seed: 0x57_55_B0_0Au32,
    };
    example_util::spawn_cloud_ring(&mut universe, bg_root, cloud_params);

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
