use crate::engine::ecs::component::{SelectionComponent, TextComponent};
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, RxWorld, SignalEmitter, SignalKind, World};

#[derive(Debug, Default)]
pub struct SelectionSystem {
    handlers_installed: bool,
}

impl SelectionSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn install_handlers(&mut self, rx: &mut RxWorld) {
        if self.handlers_installed {
            return;
        }
        self.handlers_installed = true;

        rx.add_global_handler_closure(SignalKind::Click, move |world, emit, signal| {
            let Some(EventSignal::Click { renderable, .. }) = signal.event.as_ref() else {
                return;
            };
            let Some(selection_root) = nearest_selection_ancestor(world, *renderable) else {
                return;
            };
            handle_selection_click(world, emit, selection_root, *renderable);
        });
    }
}

fn nearest_selection_ancestor(world: &World, start: ComponentId) -> Option<ComponentId> {
    let mut current = Some(start);
    while let Some(node) = current {
        if world.get_component_by_id_as::<SelectionComponent>(node).is_some() {
            return Some(node);
        }
        current = world.parent_of(node);
    }
    None
}

fn selection_visual_child(world: &World, selection_root: ComponentId) -> ComponentId {
    let children = world.children_of(selection_root);
    if children.len() == 1 {
        children[0]
    } else {
        selection_root
    }
}

fn find_selected_subtree_under_selection(
    world: &World,
    selection_root: ComponentId,
    start: ComponentId,
) -> Option<ComponentId> {
    let content_root = selection_visual_child(world, selection_root);
    let mut current = Some(start);
    while let Some(node) = current {
        if world.parent_of(node) == Some(content_root) {
            return Some(node);
        }
        current = world.parent_of(node);
    }
    None
}

fn find_descendant_by_type(world: &World, root: ComponentId, component_type: &str) -> Option<ComponentId> {
    for &child in world.children_of(root) {
        let node = world.get_component_record(child)?;
        if node.component_type == component_type {
            return Some(child);
        }
        if let Some(found) = find_descendant_by_type(world, child, component_type) {
            return Some(found);
        }
    }
    None
}

fn find_selected_item_text(world: &World, item_id: ComponentId) -> Option<String> {
    let text_id = find_descendant_by_type(world, item_id, "text")?;
    world
        .get_component_by_id_as::<TextComponent>(text_id)
        .map(|text| text.text.clone())
}

fn find_selected_item_index(
    world: &World,
    selection_root: ComponentId,
    item_id: ComponentId,
) -> Option<usize> {
    let content_root = selection_visual_child(world, selection_root);
    let mut index = 0;
    for &child in world.children_of(content_root) {
        if let Some(record) = world.get_component_record(child) {
            if record.component_type == "style" {
                continue;
            }
        }
        if child == item_id {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn set_asset_item_selected_color(world: &World, emit: &mut dyn SignalEmitter, item_id: ComponentId, selected: bool) {
    if let Some(color_id) = find_descendant_by_type(world, item_id, "color") {
        let rgba = if selected {
            [0.33, 0.55, 0.95, 1.0]
        } else {
            [0.25, 0.25, 0.25, 1.0]
        };
        emit.push_intent_now(
            color_id,
            IntentValue::SetColor {
                component_ids: vec![color_id],
                rgba,
            },
        );
    }
}

fn handle_selection_click(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    selection_root: ComponentId,
    renderable: ComponentId,
) {
    let Some(item_id) = find_selected_subtree_under_selection(world, selection_root, renderable) else {
        return;
    };
    let selected_text = match find_selected_item_text(world, item_id) {
        Some(text) => text,
        None => return,
    };
    let selected_index = find_selected_item_index(world, selection_root, item_id);

    let old_selection = {
        let selection = match world.get_component_by_id_as_mut::<SelectionComponent>(selection_root) {
            Some(selection) => selection,
            None => return,
        };

        let old_selected = selection.selected_component;
        selection.selected_index = selected_index;
        selection.selected_item = Some(selected_text);
        selection.selected_component = Some(item_id);
        old_selected
    };

    if let Some(old_id) = old_selection {
        if old_id != item_id {
            set_asset_item_selected_color(world, emit, old_id, false);
        }
    }

    set_asset_item_selected_color(world, emit, item_id, true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::command_queue::CommandQueue;
    use crate::engine::ecs::system::SystemWorld;
    use crate::engine::ecs::{EventSignal, World};
    use crate::engine::graphics::VisualWorld;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_asset_directory() -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let tmp_dir = std::env::temp_dir().join(format!("cat_engine_assets_{}", now));
        std::fs::create_dir_all(&tmp_dir).expect("create temp dir");
        tmp_dir
    }

    #[test]
    fn selection_system_click_updates_selection_state() {
        let tmp_dir = temp_asset_directory();
        let asset_path = tmp_dir.join("test_asset.mms");
        std::fs::write(
            &asset_path,
            r#"
                export fn example() {
                    let root = T {}
                    return root
                }
            "#,
        )
        .expect("write asset file");

        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();

        systems.asset_system
            .scan_assets_dir(&tmp_dir)
            .expect("scan assets dir");

        systems.selection.install_handlers(&mut systems.rx);

        let parent = world.add_component_boxed_named("parent", Box::new(crate::engine::ecs::component::TransformComponent::new()));
        let wrapper = systems
            .asset_system
            .spawn_assets_panel(&mut world, &mut emit, parent, (0.0, 0.0, 0.0))
            .expect("spawn assets panel");

        let selection_root = world
            .find_component(wrapper, "#assets_selection")
            .expect("expected selection root");
        println!("selection_root={:?}", selection_root);
        print_subtree(&world, wrapper, 0);
        println!("text_under_wrapper={:?}", world.find_all_components(wrapper, "Text"));
        println!("text_under_selection={:?}", world.find_all_components(selection_root, "Text"));

        fn print_subtree(world: &World, root: ComponentId, indent: usize) {
            let prefix = "  ".repeat(indent);
            let node = world.get_component_record(root).unwrap();
            println!("{}node={:?} type={} name={:?}", prefix, root, node.component_type, node.name);
            for &child in world.children_of(root) {
                print_subtree(world, child, indent + 1);
            }
        }
        let item_text = super::find_descendant_by_type(&world, selection_root, "text")
            .expect("expected item text component");
        let item = super::find_selected_subtree_under_selection(&world, selection_root, item_text)
            .expect("expected asset item");

        systems.rx.push_event(
            item,
            EventSignal::Click {
                raycaster: item,
                renderable: item,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );

        let _ = systems.process_signals(&mut world, &mut visuals, &mut emit, 100_000);

        let selection = world
            .get_component_by_id_as::<SelectionComponent>(selection_root)
            .expect("expected selection component");

        assert_eq!(selection.selected_component, Some(item));
        assert_eq!(selection.selected_index, Some(0));
        assert!(selection.selected_item.is_some());
    }
}
