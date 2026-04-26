use cat_engine::{engine, engine::ecs::SignalEmitter, meow_meow, utils};

const DEBUG_CLONE_WORLD_OFFSET: [f32; 3] = [0.0, 0.0, 0.1];

fn spawn_runtime_text(
    universe: &mut engine::Universe,
    owner: engine::ecs::ComponentId,
    label: &str,
) {
    use engine::ecs::component::{
        ColorComponent, EdgeInsets, SizeDimension, StyleComponent, TextComponent,
        TransformComponent,
    };

    let root = universe
        .world
        .add_component_boxed_named(label, Box::new(TransformComponent::new().with_position(0.0, 0.0, 0.2)));
    let style = universe.world.add_component_boxed_named(
        format!("{label}_style"),
        Box::new({
            let mut style = StyleComponent::new();
            style.margin = EdgeInsets::axes(0.25, 0.25);
            style.padding = EdgeInsets::axes(0.5, 0.5);
            style.height = SizeDimension::GlyphUnits(2.5);
            style.width = SizeDimension::Auto;
            style.background_color = Some([1.0, 0.80, 0.80, 1.0]);
            style
        }),
    );
    let text_root = universe.world.add_component_boxed_named(
        format!("{label}_text_root"),
        Box::new(TransformComponent::new().with_position(0.0, 0.0, 0.2)),
    );
    let text = universe
        .world
        .add_component_boxed_named(format!("{label}_text"), Box::new(TextComponent::new(label)));
    let color = universe
        .world
        .add_component(ColorComponent::rgba(0.40, 0.05, 0.05, 1.0));

    let _ = universe.world.add_child(root, style);
    let _ = universe.world.add_child(root, text_root);
    let _ = universe.world.add_child(text_root, text);
    let _ = universe.world.add_child(text, color);

    universe.attach(owner, root).expect("attach routed child");
}

fn collect_container_items(
    world: &engine::ecs::World,
    container: engine::ecs::ComponentId,
) -> Vec<engine::ecs::ComponentId> {
    use engine::ecs::component::TransformComponent;

    world
        .children_of(container)
        .iter()
        .copied()
        .filter(|&child| {
            world.get_component_by_id_as::<TransformComponent>(child).is_some()
                && !world
                    .component_label(child)
                    .map(|label| label.starts_with("__"))
                    .unwrap_or(false)
        })
        .collect()
}

fn decompose_world_trs(matrix_world: [[f32; 4]; 4]) -> ([f32; 3], [f32; 4], [f32; 3]) {
    let translation = [matrix_world[3][0], matrix_world[3][1], matrix_world[3][2]];
    let scale = [
        (matrix_world[0][0] * matrix_world[0][0]
            + matrix_world[0][1] * matrix_world[0][1]
            + matrix_world[0][2] * matrix_world[0][2])
            .sqrt(),
        (matrix_world[1][0] * matrix_world[1][0]
            + matrix_world[1][1] * matrix_world[1][1]
            + matrix_world[1][2] * matrix_world[1][2])
            .sqrt(),
        (matrix_world[2][0] * matrix_world[2][0]
            + matrix_world[2][1] * matrix_world[2][1]
            + matrix_world[2][2] * matrix_world[2][2])
            .sqrt(),
    ];
    let rotation = cat_engine::utils::math::mat_to_quat(matrix_world);
    (translation, rotation, scale)
}

fn spawn_container_item_debug_clones(
    universe: &mut engine::Universe,
    container: engine::ecs::ComponentId,
) {
    use engine::ecs::{ComponentCodec, IntentValue};
    use engine::ecs::system::TransformSystem;

    let items = collect_container_items(&universe.world, container);
    for (index, item) in items.into_iter().enumerate() {
        let Some(matrix_world) = TransformSystem::world_model(&universe.world, item) else {
            println!("[diy-panel-debug] skip {:?}: no world matrix", item);
            continue;
        };

        let item_label = universe
            .world
            .component_label(item)
            .unwrap_or("")
            .to_string();
        println!(
            "[diy-panel-debug] item={} label={:?} matrix_world={:?}",
            index,
            item_label,
            matrix_world,
        );

        let Ok(encoded) = ComponentCodec::encode_subtree_node(&universe.world, item) else {
            println!("[diy-panel-debug] skip {:?}: failed to encode subtree", item);
            continue;
        };
        let Ok(clone_root) = ComponentCodec::decode_subtree_node_with_new_guids(
            &mut universe.world,
            None,
            &encoded,
        ) else {
            println!("[diy-panel-debug] skip {:?}: failed to decode subtree clone", item);
            continue;
        };

        universe.add(clone_root);

        let (mut clone_translation, clone_rotation, clone_scale) = decompose_world_trs(matrix_world);
        clone_translation[0] += DEBUG_CLONE_WORLD_OFFSET[0];
        clone_translation[1] += DEBUG_CLONE_WORLD_OFFSET[1];
        clone_translation[2] += DEBUG_CLONE_WORLD_OFFSET[2];

        universe.command_queue.push_intent_now(
            clone_root,
            IntentValue::UpdateTransform {
                component_ids: vec![clone_root],
                translation: clone_translation,
                rotation_quat_xyzw: clone_rotation,
                scale: clone_scale,
            },
        );

        let label = if item_label.is_empty() {
            format!("item_{index}")
        } else {
            format!("{}_{}", item_label, index)
        };
        println!(
            "[diy-panel-debug] cloned item={} label={:?} clone_root={:?} offset_translation={:?}",
            index,
            label,
            clone_root,
            clone_translation,
        );
    }
}

fn main() {
    utils::logger::init();

    let output = meow_meow::MeowMeowRunner::eval(include_str!("diy-panel.mms"));

    for error in &output.errors {
        eprintln!("[mms] {error}");
    }
    println!("[mms] {} intent(s) from diy-panel.mms", output.intents.len());

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
        &mut universe.command_queue,
    );

    universe.enable_repl();

    let demo_root = universe
        .world
        .all_components()
        .find(|&cid| universe.world.component_label(cid) == Some("diy_panel_demo"))
        .expect("diy panel demo root");
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

    let input = engine::user_input::InputState::default();
    universe.systems.tick(
        &mut universe.world,
        &mut universe.visuals,
        &input,
        &mut universe.command_queue,
        0.0,
    );
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    //spawn_container_item_debug_clones(&mut universe, container);
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    let scroll = universe
        .world
        .children_of(container)
        .iter()
        .copied()
        .find(|&child| {
            universe
                .world
                .component_label(child)
                == Some("__scroll")
                && universe
                    .world
                    .get_component_by_id_as::<engine::ecs::component::ScrollingComponent>(child)
                    .is_some()
        })
        .expect("layout-owned scroll wrapper");
    let track = universe
        .world
        .get_component_by_id_as::<engine::ecs::component::ScrollingComponent>(scroll)
        .and_then(|sc| sc.track)
        .expect("layout-owned scroll track");

    assert_eq!(universe.parent_of(authored_child), Some(track));
    assert_eq!(universe.parent_of(late_child), Some(track));

    println!("[diy-panel] init-time routing verified, then layout-owned scrolling moved content into __scroll_track");

    engine::Windowing::run_app(universe).expect("Windowing failed");
}
