use cat_engine::engine::ecs::SignalEmitter;
use cat_engine::{engine, meow_meow, utils};

#[path = "util/mod.rs"]
mod util;

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let source = include_str!("camera-toggle.mms");
    let output = meow_meow::MeowMeowRunner::eval_with_world_at_path(
        source,
        Some("examples/camera-toggle.mms"),
        &mut universe.world,
        &mut universe.systems.rx,
        &mut universe.command_queue,
    );

    for error in &output.errors {
        eprintln!("[mms] {error}");
    }
    println!(
        "[mms] {} intent(s) from camera-toggle.mms",
        output.intents.len()
    );

    let scope = engine::ecs::ComponentId::default();
    for intent in output.intents {
        universe.command_queue.push_intent_now(scope, intent);
    }

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.render_assets,
        &mut universe.command_queue,
    );

    // let terrain_anchor = universe
    //     .world
    //     .all_components()
    //     .filter(|&id| universe.world.parent_of(id).is_none())
    //     .find_map(|root| universe.world.find_component(root, "#desktop_rig"))
    //     .and_then(|rig| {
    //         universe
    //             .world
    //             .get_component_by_id_as::<engine::ecs::component::TransformComponent>(rig)
    //     })
    //     .map(|transform| {
    //         [
    //             transform.transform.translation[0],
    //             transform.transform.translation[1] - 2.4,
    //             transform.transform.translation[2] - 16.0,
    //         ]
    //     })
    //     .unwrap_or([0.0, -1.2, -10.0]);

    // util::spawn_perlin_cube_patch(&mut universe, terrain_anchor, 128, 128);

    // universe.systems.process_commands(
    //     &mut universe.world,
    //     &mut universe.visuals,
    //     &mut universe.render_assets,
    //     &mut universe.command_queue,
    // );

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
