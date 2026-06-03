use cat_engine::{engine, engine::ecs::SignalEmitter, meow_meow, utils};

fn spawn_runtime_text(
    universe: &mut engine::Universe,
    owner: engine::ecs::ComponentId,
    label: &str,
) {
    use engine::ecs::component::{ColorComponent, EdgeInsets, SizeDimension, StyleComponent, TextComponent, TransformComponent};

    let root = universe
        .world
        .add_component_boxed_named(label, Box::new(TransformComponent::new()));
    let style = universe.world.add_component_boxed_named(
        format!("{label}_style"),
        Box::new({
            let mut style = StyleComponent::new();
            style.margin = EdgeInsets::all(0.5);
            style.padding = EdgeInsets::axes(0.75, 0.5);
            style.width = SizeDimension::Auto;
            style.background_color = Some([0.93, 0.88, 0.98, 1.0]);
            style
        }),
    );
    let text = universe
        .world
        .add_component_boxed_named(format!("{label}_text"), Box::new(TextComponent::new(label)));
    let color = universe
        .world
        .add_component(ColorComponent::rgba(0.38, 0.14, 0.62, 1.0));

    let _ = universe.world.add_child(root, style);
    let _ = universe.world.add_child(root, text);
    let _ = universe.world.add_child(text, color);

    universe.attach(owner, root).expect("attach routed child");
}

fn main() {
    utils::logger::init();

    let output = meow_meow::MeowMeowRunner::eval(include_str!("router.mms"));

    for error in &output.errors {
        eprintln!("[mms] {error}");
    }
    println!("[mms] {} intent(s) from router.mms", output.intents.len());

    if !output.errors.is_empty() {
        std::process::exit(1);
    }

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

    universe.enable_repl();

    let demo_root = universe
        .world
        .all_components()
        .find(|&cid| universe.world.component_label(cid) == Some("router_demo"))
        .expect("router demo root");
    let container = universe
        .find_component(demo_root, "[name='container']")
        .expect("container target");
    let authored_child = universe
        .find_component(demo_root, "[name='authored_child']")
        .expect("authored child");

    assert_eq!(universe.parent_of(authored_child), Some(container));

    spawn_runtime_text(&mut universe, demo_root, "late child routed on attach");

    let late_child = universe
        .find_component(demo_root, "[name='late child routed on attach']")
        .expect("late child");
    assert_eq!(universe.parent_of(late_child), Some(container));

    println!("[router-demo] init-time and attach-time routing verified");

    engine::Windowing::run_app(universe).expect("Windowing failed");
}