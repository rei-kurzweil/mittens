// Focused repro for docs/bugs/vtuber-desktop-scrolling-interference.md
//
// Loads panel-pierce.mms (scroll panel in front of a raycastable cube wall)
// and populates the scroll track with rows so the bug is observable: try to
// click+drag the yellow viewport — the drag is intercepted by background cubes.

use cat_engine::{engine, engine::ecs::SignalEmitter, meow_meow, utils};

fn find_named(world: &engine::ecs::World, label: &str) -> engine::ecs::ComponentId {
    world
        .all_components()
        .find(|&cid| world.component_label(cid) == Some(label))
        .unwrap_or_else(|| panic!("missing component {label:?}"))
}

fn spawn_row(
    universe: &mut engine::Universe,
    scrolling: engine::ecs::ComponentId,
    index: usize,
) -> engine::ecs::ComponentId {
    use engine::ecs::component::{
        ColorComponent, RenderableComponent, TextComponent, TransformComponent,
    };

    let label = format!("row_{index:02}");
    let row = universe.world.add_component_boxed_named(
        label.clone(),
        Box::new(TransformComponent::new().with_position(0.0, -(index as f32) * 1.15, 0.05)),
    );
    let panel = universe.world.add_component_boxed_named(
        format!("{label}_panel"),
        Box::new(
            TransformComponent::new()
                .with_position(2.3, -0.45, 0.0)
                .with_scale(4.6, 0.9, 1.0),
        ),
    );
    let panel_renderable = universe.world.add_component_boxed_named(
        format!("{label}_rend"),
        Box::new(RenderableComponent::square()),
    );
    let panel_color = universe
        .world
        .add_component(ColorComponent::rgba(0.98, 0.94, 0.78, 1.0));

    let text_anchor = universe.world.add_component_boxed_named(
        format!("{label}_text_anchor"),
        Box::new(
            TransformComponent::new()
                .with_position(0.35, -0.38, 0.02)
                .with_scale(0.11, 0.11, 0.11),
        ),
    );
    let text = universe.world.add_component_boxed_named(
        format!("{label}_text"),
        Box::new(TextComponent::new(label.clone())),
    );
    let text_color = universe
        .world
        .add_component(ColorComponent::rgba(0.20, 0.16, 0.08, 1.0));

    let _ = universe.world.add_child(row, panel);
    let _ = universe.world.add_child(panel, panel_renderable);
    let _ = universe.world.add_child(panel_renderable, panel_color);
    let _ = universe.world.add_child(row, text_anchor);
    let _ = universe.world.add_child(text_anchor, text);
    let _ = universe.world.add_child(text, text_color);

    universe.attach(scrolling, row).expect("attach scroll row");
    row
}

fn main() {
    utils::logger::init();

    let output = meow_meow::MeowMeowRunner::eval(include_str!("panel-pierce.mms"));

    for error in &output.errors {
        eprintln!("[mms] {error}");
    }
    println!(
        "[mms] {} intent(s) from panel-pierce.mms",
        output.intents.len()
    );

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
        &mut universe.render_assets,
        &mut universe.command_queue,
    );

    let panel_scroll = find_named(&universe.world, "panel_scroll");

    let drag_scope = universe
        .world
        .get_component_by_id_as::<engine::ecs::component::ScrollingComponent>(panel_scroll)
        .and_then(|sc| sc.drag_scope)
        .expect("panel drag scope");

    for index in 0..18 {
        spawn_row(&mut universe, panel_scroll, index);
    }

    println!(
        "[panel-pierce] panel_scroll={:?} drag_scope={:?} — drag yellow viewport to repro bug",
        panel_scroll, drag_scope
    );

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
