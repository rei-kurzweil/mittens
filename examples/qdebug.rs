use mittens_engine::{engine, scripting, utils};
fn main() {
    mittens_engine::example_support::ensure_model_assets();
    utils::logger::init();
    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);
    let src = include_str!("qdebug.mms");
    let out = scripting::MeowMeowRunner::eval_with_world(
        src,
        &mut universe.world,
        &mut universe.systems.rx,
        &mut universe.command_queue,
    );
    for e in &out.errors {
        eprintln!("[mms-err] {e}");
    }
    println!("intents: {}", out.intents.len());
    for c in universe.world.all_components() {
        if let Some(label) = universe.world.component_label(c) {
            {
                println!(
                    "  comp {:?} type={:?} name={:?}",
                    c,
                    universe.world.component_name(c),
                    label
                );
            }
        }
    }
}
