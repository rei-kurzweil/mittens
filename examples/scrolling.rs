use cat_engine::{engine, engine::ecs::SignalEmitter, meow_meow, utils};

fn find_named(world: &engine::ecs::World, label: &str) -> engine::ecs::ComponentId {
    world
        .all_components()
        .find(|&cid| world.component_label(cid) == Some(label))
        .unwrap_or_else(|| panic!("missing component {label:?}"))
}

fn first_renderable_in_subtree(
    world: &engine::ecs::World,
    root: engine::ecs::ComponentId,
) -> engine::ecs::ComponentId {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if world
            .get_component_by_id_as::<engine::ecs::component::RenderableComponent>(node)
            .is_some()
        {
            return node;
        }
        for &child in world.children_of(node).iter().rev() {
            stack.push(child);
        }
    }

    panic!("missing renderable under subtree {:?}", root);
}

fn spawn_scroll_item(
    universe: &mut engine::Universe,
    scrolling: engine::ecs::ComponentId,
    prefix: &str,
    index: usize,
    panel_rgba: [f32; 4],
    text_rgba: [f32; 4],
) -> engine::ecs::ComponentId {
    use engine::ecs::component::{
        ColorComponent, RenderableComponent, TextComponent, TransformComponent,
    };

    let label = format!("{prefix}_item_{index:02}");
    let root = universe.world.add_component_boxed_named(
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
        format!("{label}_renderable"),
        Box::new(RenderableComponent::square()),
    );
    let panel_color = universe.world.add_component(ColorComponent::rgba(
        panel_rgba[0],
        panel_rgba[1],
        panel_rgba[2],
        panel_rgba[3],
    ));

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
    let text_color = universe.world.add_component(ColorComponent::rgba(
        text_rgba[0],
        text_rgba[1],
        text_rgba[2],
        text_rgba[3],
    ));

    let _ = universe.world.add_child(root, panel);
    let _ = universe.world.add_child(panel, panel_renderable);
    let _ = universe.world.add_child(panel_renderable, panel_color);
    let _ = universe.world.add_child(root, text_anchor);
    let _ = universe.world.add_child(text_anchor, text);
    let _ = universe.world.add_child(text, text_color);

    universe
        .attach(scrolling, root)
        .expect("attach scroll item");
    root
}

fn populate_scrolling(
    universe: &mut engine::Universe,
    scrolling: engine::ecs::ComponentId,
    prefix: &str,
    panel_rgba: [f32; 4],
    text_rgba: [f32; 4],
    count: usize,
) {
    for index in 0..count {
        let root = spawn_scroll_item(universe, scrolling, prefix, index, panel_rgba, text_rgba);
        let track = universe
            .world
            .get_component_by_id_as::<engine::ecs::component::ScrollingComponent>(scrolling)
            .and_then(|sc| sc.track)
            .expect("owned scroll track");
        assert_eq!(universe.parent_of(root), Some(track));
    }
}

fn main() {
    utils::logger::init();

    let output = meow_meow::MeowMeowRunner::eval(include_str!("scrolling.mms"));

    for error in &output.errors {
        eprintln!("[mms] {error}");
    }
    println!(
        "[mms] {} intent(s) from scrolling.mms",
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
        &universe.render_assets,
        &mut universe.command_queue,
    );

    let manual_scroll = find_named(&universe.world, "manual_scroll");
    let layout_scroll = find_named(&universe.world, "layout_scroll");
    let layout_bg = find_named(&universe.world, "__bg");

    let layout_bg_renderable = first_renderable_in_subtree(&universe.world, layout_bg);

    let manual_drag_scope = universe
        .world
        .get_component_by_id_as::<engine::ecs::component::ScrollingComponent>(manual_scroll)
        .and_then(|sc| sc.drag_scope)
        .expect("manual drag scope");
    let layout_drag_scope = universe
        .world
        .get_component_by_id_as::<engine::ecs::component::ScrollingComponent>(layout_scroll)
        .and_then(|sc| sc.drag_scope)
        .expect("layout drag scope");

    assert_ne!(manual_drag_scope, layout_bg_renderable);
    assert_eq!(layout_drag_scope, layout_bg_renderable);

    populate_scrolling(
        &mut universe,
        manual_scroll,
        "manual",
        [0.78, 0.87, 0.98, 1.0],
        [0.08, 0.16, 0.28, 1.0],
        18,
    );
    populate_scrolling(
        &mut universe,
        layout_scroll,
        "layout",
        [0.98, 0.87, 0.74, 1.0],
        [0.30, 0.14, 0.05, 1.0],
        18,
    );

    println!(
        "[scrolling-demo] manual drag_scope={:?} layout drag_scope={:?} late attach routing verified",
        manual_drag_scope, layout_drag_scope
    );

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
