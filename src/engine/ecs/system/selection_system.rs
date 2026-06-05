use crate::engine::ecs::component::{
    BoundsComponent, ColorComponent, Component, EmissiveComponent, LayoutComponent,
    OptionComponent, RenderableComponent, SelectionComponent, SelectionEntry, SelectionMode,
    StyleComponent, TextComponent, TransformComponent,
};
use crate::engine::ecs::{
    ComponentId, EventSignal, IntentValue, RxWorld, SignalEmitter, SignalKind, World,
};
use crate::engine::graphics::bounds::{mat4_identity, mat4_mul, Aabb};

const SELECTED_HIGHLIGHT_RGBA: [f32; 4] = [1.0, 0.84, 0.0, 1.0];
const SELECTED_HIGHLIGHT_EMISSIVE: f32 = 3.0;
const OVERLAY_HIGHLIGHT_Z_OFFSET: f32 = 0.01;
const OVERLAY_HIGHLIGHT_Z_THICKNESS: f32 = 0.001;

#[derive(Debug, Clone, Copy)]
struct SelectionStyleStateComponent {
    original_background_color: Option<[f32; 4]>,
    component: Option<ComponentId>,
}

impl SelectionStyleStateComponent {
    fn new(original_background_color: Option<[f32; 4]>) -> Self {
        Self {
            original_background_color,
            component: None,
        }
    }
}

impl Component for SelectionStyleStateComponent {
    fn set_id(&mut self, id: ComponentId) {
        self.component = Some(id);
    }

    fn name(&self) -> &'static str {
        "selection_style_state"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        ce("SelectionStyleState")
    }
}

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
            let Some((selection_root, option_owner)) = resolve_selection_click(world, *renderable)
            else {
                return;
            };
            handle_selection_click(world, emit, selection_root, option_owner);
        });
    }
}

fn selection_marker_on_node(world: &World, node: ComponentId) -> Option<ComponentId> {
    if world
        .get_component_by_id_as::<SelectionComponent>(node)
        .is_some()
    {
        return Some(node);
    }
    world.children_of(node).iter().copied().find(|&child| {
        world
            .get_component_by_id_as::<SelectionComponent>(child)
            .is_some()
    })
}

fn option_marker_on_node(world: &World, node: ComponentId) -> Option<ComponentId> {
    if world
        .get_component_by_id_as::<OptionComponent>(node)
        .is_some()
    {
        return Some(node);
    }
    world.children_of(node).iter().copied().find(|&child| {
        world
            .get_component_by_id_as::<OptionComponent>(child)
            .is_some()
    })
}

fn selection_scope_owner(world: &World, selection_root: ComponentId) -> ComponentId {
    if world.children_of(selection_root).is_empty() {
        world.parent_of(selection_root).unwrap_or(selection_root)
    } else {
        selection_root
    }
}

fn nearest_enclosing_selection(world: &World, start: ComponentId) -> Option<ComponentId> {
    let mut current = Some(start);
    while let Some(node) = current {
        if let Some(selection) = selection_marker_on_node(world, node) {
            return Some(selection);
        }
        current = world.parent_of(node);
    }
    None
}

fn resolve_selection_click(
    world: &World,
    renderable: ComponentId,
) -> Option<(ComponentId, ComponentId)> {
    let mut current = Some(renderable);
    while let Some(node) = current {
        if option_marker_on_node(world, node).is_some() {
            let selection_root = nearest_enclosing_selection(world, node)?;
            return Some((selection_root, node));
        }
        if selection_marker_on_node(world, node).is_some() {
            return None;
        }
        current = world.parent_of(node);
    }
    None
}

fn find_descendant_by_type(
    world: &World,
    root: ComponentId,
    component_type: &str,
) -> Option<ComponentId> {
    // Check root itself first
    if let Some(node) = world.get_component_record(root) {
        if node.component_type == component_type {
            return Some(root);
        }
    }
    // Then check children recursively
    for &child in world.children_of(root) {
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
    let scope_owner = selection_scope_owner(world, selection_root);
    let mut index = 0usize;
    for &child in world.children_of(scope_owner) {
        if option_marker_on_node(world, child).is_some() {
            if child == item_id {
                return Some(index);
            }
            index += 1;
        }
    }
    None
}

fn immediate_style_child(world: &World, root: ComponentId) -> Option<ComponentId> {
    world.children_of(root).iter().copied().find(|&child| {
        world
            .get_component_by_id_as::<StyleComponent>(child)
            .is_some()
    })
}

fn selection_style_state_child(world: &World, root: ComponentId) -> Option<ComponentId> {
    world.children_of(root).iter().copied().find(|&child| {
        world
            .get_component_by_id_as::<SelectionStyleStateComponent>(child)
            .is_some()
    })
}

fn styled_option_target(world: &World, root: ComponentId) -> Option<ComponentId> {
    immediate_style_child(world, root)
}

fn mark_nearest_layout_dirty(world: &mut World, start: ComponentId) {
    let mut current = Some(start);
    while let Some(component_id) = current {
        if let Some(layout) = world.get_component_by_id_as_mut::<LayoutComponent>(component_id) {
            layout.mark_dirty();
            return;
        }
        current = world.parent_of(component_id);
    }
}

fn set_styled_selection(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    item_id: ComponentId,
    selected: bool,
) -> bool {
    let Some(style_id) = styled_option_target(world, item_id) else {
        return false;
    };

    if selected {
        if selection_style_state_child(world, item_id).is_none() {
            let original_background_color = world
                .get_component_by_id_as::<StyleComponent>(style_id)
                .map(|style| style.background_color)
                .unwrap_or(None);
            let state_id = world.add_component_boxed_named(
                "selection_style_state",
                Box::new(SelectionStyleStateComponent::new(original_background_color)),
            );
            let _ = world.add_child(item_id, state_id);
            world.init_component_tree(state_id, emit);
        }
        if let Some(style) = world.get_component_by_id_as_mut::<StyleComponent>(style_id) {
            style.background_color = Some(SELECTED_HIGHLIGHT_RGBA);
        }
        mark_nearest_layout_dirty(world, item_id);
        return true;
    }

    let original_background_color =
        selection_style_state_child(world, item_id).and_then(|state_id| {
            world
                .get_component_by_id_as::<SelectionStyleStateComponent>(state_id)
                .map(|state| state.original_background_color)
        });
    if let Some(style) = world.get_component_by_id_as_mut::<StyleComponent>(style_id) {
        style.background_color = original_background_color.unwrap_or(None);
    }
    if let Some(state_id) = selection_style_state_child(world, item_id) {
        emit.push_intent_now(
            state_id,
            IntentValue::RemoveSubtree {
                component_ids: vec![state_id],
            },
        );
    }
    mark_nearest_layout_dirty(world, item_id);
    true
}

fn subtree_local_bounds(world: &World, root: ComponentId) -> Option<Aabb> {
    fn visit(
        world: &World,
        node: ComponentId,
        parent_to_root: [[f32; 4]; 4],
        acc: &mut Option<Aabb>,
    ) {
        let mut local_to_root = parent_to_root;
        if let Some(tc) = world.get_component_by_id_as::<TransformComponent>(node) {
            local_to_root = mat4_mul(parent_to_root, tc.transform.model);
        }
        if world
            .get_component_by_id_as::<RenderableComponent>(node)
            .is_some()
        {
            for &child in world.children_of(node) {
                if let Some(bounds) = world.get_component_by_id_as::<BoundsComponent>(child) {
                    let transformed = bounds.local.transformed(local_to_root);
                    *acc = Some(match acc {
                        Some(prev) => prev.union(&transformed),
                        None => transformed,
                    });
                    break;
                }
            }
        }
        for &child in world.children_of(node) {
            if world.component_label(child) == Some("selection_highlight") {
                continue;
            }
            visit(world, child, local_to_root, acc);
        }
    }

    let mut acc = None;
    visit(world, root, mat4_identity(), &mut acc);
    acc
}

fn ensure_selection_overlay(world: &mut World, emit: &mut dyn SignalEmitter, item_id: ComponentId) {
    let Some(bounds) = subtree_local_bounds(world, item_id) else {
        return;
    };

    let highlight_id = world
        .children_of(item_id)
        .iter()
        .copied()
        .find(|&child| world.component_label(child) == Some("selection_highlight"))
        .unwrap_or_else(|| {
            let highlight = world.add_component_boxed_named(
                "selection_highlight",
                Box::new(TransformComponent::new()),
            );
            let color = world.add_component_boxed(Box::new(ColorComponent::rgba(
                SELECTED_HIGHLIGHT_RGBA[0],
                SELECTED_HIGHLIGHT_RGBA[1],
                SELECTED_HIGHLIGHT_RGBA[2],
                SELECTED_HIGHLIGHT_RGBA[3],
            )));
            let renderable = world.add_component_boxed(Box::new(RenderableComponent::square()));
            let emissive = world.add_component_boxed(Box::new(EmissiveComponent::new(
                SELECTED_HIGHLIGHT_EMISSIVE,
            )));

            let _ = world.add_child(highlight, color);
            let _ = world.add_child(highlight, renderable);
            let _ = world.add_child(highlight, emissive);
            let _ = world.add_child(item_id, highlight);
            world.init_component_tree(highlight, emit);
            highlight
        });

    let center = bounds.center();
    emit.push_intent_now(
        highlight_id,
        IntentValue::UpdateTransform {
            component_ids: vec![highlight_id],
            translation: [
                center[0],
                center[1],
                bounds.max[2] + OVERLAY_HIGHLIGHT_Z_OFFSET,
            ],
            rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
            scale: [
                bounds.width().max(0.001),
                bounds.height().max(0.001),
                OVERLAY_HIGHLIGHT_Z_THICKNESS,
            ],
        },
    );
}

fn remove_selection_overlay(world: &World, emit: &mut dyn SignalEmitter, item_id: ComponentId) {
    for &child in world.children_of(item_id) {
        if let Some(record) = world.get_component_record(child) {
            if record.name == "selection_highlight" {
                emit.push_intent_now(
                    child,
                    IntentValue::RemoveSubtree {
                        component_ids: vec![child],
                    },
                );
            }
        }
    }
}

fn add_selection_highlight(world: &mut World, emit: &mut dyn SignalEmitter, item_id: ComponentId) {
    if set_styled_selection(world, emit, item_id, true) {
        remove_selection_overlay(world, emit, item_id);
        return;
    }
    ensure_selection_overlay(world, emit, item_id);
}

fn remove_selection_highlight(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    item_id: ComponentId,
) {
    if set_styled_selection(world, emit, item_id, false) {
        remove_selection_overlay(world, emit, item_id);
        return;
    }
    remove_selection_overlay(world, emit, item_id);
}

pub fn emit_selection_events(
    emit: &mut dyn SignalEmitter,
    selection_root: ComponentId,
    mode: SelectionMode,
    old_entries: &[SelectionEntry],
    old_selected_component: Option<ComponentId>,
    new_entries: Vec<SelectionEntry>,
    new_selected_component: Option<ComponentId>,
) {
    for entry in new_entries.iter() {
        if !old_entries
            .iter()
            .any(|old_entry| old_entry.component == entry.component)
        {
            emit.push_event(
                selection_root,
                EventSignal::SelectionAdded {
                    selection_root,
                    entry: entry.clone(),
                },
            );
        }
    }

    for entry in old_entries.iter() {
        if !new_entries
            .iter()
            .any(|new_entry| new_entry.component == entry.component)
        {
            emit.push_event(
                selection_root,
                EventSignal::SelectionRemoved {
                    selection_root,
                    entry: entry.clone(),
                },
            );
        }
    }

    if !old_entries.is_empty() && new_entries.is_empty() {
        emit.push_event(
            selection_root,
            EventSignal::SelectionCleared { selection_root },
        );
    }

    if old_entries != new_entries || old_selected_component != new_selected_component {
        emit.push_event(
            selection_root,
            EventSignal::SelectionChanged {
                selection_root,
                mode,
                selected_entries: new_entries,
                selected_component: new_selected_component,
            },
        );
    }
}

pub fn apply_selection_set(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    selection_root: ComponentId,
    entries: Vec<SelectionEntry>,
    primary: Option<ComponentId>,
) {
    let (mode, old_entries, old_selected_component, new_entries, new_selected_component) = {
        let selection = match world.get_component_by_id_as_mut::<SelectionComponent>(selection_root)
        {
            Some(selection) => selection,
            None => return,
        };

        let old_entries = selection.selected_entries.clone();
        let old_selected_component = selection.selected_component;
        let mut new_entries = entries;
        if matches!(selection.mode, SelectionMode::Single) && new_entries.len() > 1 {
            new_entries.truncate(1);
        }

        selection.selected_entries = new_entries.clone();
        if let Some(primary_component) = primary {
            if let Some(entry) = selection
                .selected_entries
                .iter()
                .find(|entry| entry.component == primary_component)
                .cloned()
            {
                selection.selected_index = entry.index;
                selection.selected_item = entry.item;
                selection.selected_component = Some(entry.component);
            } else if let Some(entry) = selection.selected_entries.last().cloned() {
                selection.selected_index = entry.index;
                selection.selected_item = entry.item;
                selection.selected_component = Some(entry.component);
            } else {
                selection.selected_index = None;
                selection.selected_item = None;
                selection.selected_component = None;
            }
        } else if let Some(entry) = selection.selected_entries.last().cloned() {
            selection.selected_index = entry.index;
            selection.selected_item = entry.item;
            selection.selected_component = Some(entry.component);
        } else {
            selection.selected_index = None;
            selection.selected_item = None;
            selection.selected_component = None;
        }

        (
            selection.mode,
            old_entries,
            old_selected_component,
            selection.selected_entries.clone(),
            selection.selected_component,
        )
    };

    for entry in old_entries.iter() {
        if !new_entries
            .iter()
            .any(|new_entry| new_entry.component == entry.component)
        {
            remove_selection_highlight(world, emit, entry.component);
        }
    }

    for entry in new_entries.iter() {
        if !old_entries
            .iter()
            .any(|old_entry| old_entry.component == entry.component)
        {
            add_selection_highlight(world, emit, entry.component);
        }
    }

    if matches!(mode, SelectionMode::Single) {
        if let Some(selected_component) = new_selected_component {
            if old_selected_component != Some(selected_component) {
                add_selection_highlight(world, emit, selected_component);
            }
        }
    }

    emit_selection_events(
        emit,
        selection_root,
        mode,
        &old_entries,
        old_selected_component,
        new_entries,
        new_selected_component,
    );
}

fn handle_selection_click(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    selection_root: ComponentId,
    item_id: ComponentId,
) {
    if let Some(rec) = world.get_component_record(item_id) {
        println!(
            "[selection] selected item name={:?} type={} id={:?}",
            rec.name, rec.component_type, item_id
        );
    }
    let selected_text = find_selected_item_text(world, item_id);
    let selected_index = find_selected_item_index(world, selection_root, item_id);
    println!(
        "[selection] text={:?} index={:?}",
        selected_text, selected_index
    );

    let entry = SelectionEntry {
        index: selected_index,
        item: selected_text,
        component: item_id,
    };

    let (next_entries, next_primary) = {
        let selection = match world.get_component_by_id_as::<SelectionComponent>(selection_root) {
            Some(selection) => selection,
            None => return,
        };

        if selection.is_multiple() {
            let mut next_entries = selection.selected_entries.clone();
            if let Some(index) = next_entries
                .iter()
                .position(|selected| selected.component == item_id)
            {
                next_entries.remove(index);
                let next_primary = next_entries.last().map(|entry| entry.component);
                (next_entries, next_primary)
            } else {
                next_entries.push(entry.clone());
                (next_entries, Some(entry.component))
            }
        } else {
            (vec![entry.clone()], Some(entry.component))
        }
    };

    apply_selection_set(world, emit, selection_root, next_entries, next_primary);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::command_queue::CommandQueue;
    use crate::engine::ecs::component::{
        EditorComponent, OptionComponent, SelectionMode, TransformComponent,
    };
    use crate::engine::ecs::system::SystemWorld;
    use crate::engine::ecs::{EventSignal, IntentValue, SignalKind, World};
    use crate::engine::graphics::{RenderAssets, VisualWorld};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
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

    fn find_named_root(world: &World, name: &str) -> ComponentId {
        world
            .all_components()
            .find(|&component_id| {
                world.parent_of(component_id).is_none()
                    && world
                        .component_label(component_id)
                        .is_some_and(|label| label == name)
            })
            .unwrap_or_else(|| panic!("expected root named {name}"))
    }

    fn spawn_test_option_item(
        world: &mut World,
        parent: ComponentId,
        name: &str,
        with_style: bool,
    ) -> (ComponentId, ComponentId, Option<ComponentId>) {
        let item = world.add_component_boxed_named(name, Box::new(TransformComponent::new()));
        let _ = world.add_child(parent, item);

        let option = world.add_component_boxed(Box::new(OptionComponent::new()));
        let _ = world.add_child(item, option);

        let style_id = if with_style {
            let style = world.add_component_boxed(Box::new(StyleComponent::default()));
            let _ = world.add_child(item, style);
            Some(style)
        } else {
            None
        };

        let renderable_root =
            world.add_component_boxed_named("renderable_root", Box::new(TransformComponent::new()));
        let _ = world.add_child(item, renderable_root);
        let renderable = world.add_component_boxed(Box::new(RenderableComponent::square()));
        let _ = world.add_child(renderable_root, renderable);
        let bounds = world.add_component_boxed(Box::new(BoundsComponent::new(
            Aabb::from_points(&[
                [-0.5, -0.5, 0.0],
                [0.5, -0.5, 0.0],
                [-0.5, 0.5, 0.0],
                [0.5, 0.5, 0.0],
            ])
            .expect("bounds"),
        )));
        let _ = world.add_child(renderable, bounds);

        (item, renderable, style_id)
    }

    fn fit_bounds_content_scale(world: &World, item_id: ComponentId) -> [f32; 3] {
        let fit = world
            .children_of(item_id)
            .iter()
            .copied()
            .find(|&child| {
                world
                    .get_component_by_id_as::<crate::engine::ecs::component::FitBoundsComponent>(
                        child,
                    )
                    .is_some()
            })
            .expect("expected fit_bounds child");
        let content = world
            .children_of(fit)
            .iter()
            .copied()
            .find(|&child| world.component_label(child) == Some("__fit_bounds_content"))
            .expect("expected fit_bounds content transform");
        world
            .get_component_by_id_as::<TransformComponent>(content)
            .expect("expected fit_bounds content transform component")
            .transform
            .scale
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
        let render_assets = crate::engine::graphics::RenderAssets::new();

        systems
            .asset_system
            .scan_assets_dir(&tmp_dir)
            .expect("scan assets dir");

        systems.selection.install_handlers(&mut systems.rx);

        let parent = world.add_component_boxed_named(
            "parent",
            Box::new(crate::engine::ecs::component::TransformComponent::new()),
        );
        let wrapper = systems
            .asset_system
            .spawn_assets_panel(
                &mut world,
                &render_assets,
                &mut emit,
                parent,
                (0.0, 0.0, 0.0),
            )
            .expect("spawn assets panel");

        let selection_root = world
            .find_component(wrapper, "#assets_selection")
            .expect("expected selection root");
        let assets_content_area = world
            .find_component(wrapper, "#assets_content_area")
            .expect("expected assets content area");

        fn print_subtree(world: &World, root: ComponentId, indent: usize) {
            let prefix = "  ".repeat(indent);
            let node = world.get_component_record(root).unwrap();
            println!(
                "{}node={:?} type={} name={:?}",
                prefix, root, node.component_type, node.name
            );
            for &child in world.children_of(root) {
                print_subtree(world, child, indent + 1);
            }
        }
        print_subtree(&world, wrapper, 0);

        let item_text = super::find_descendant_by_type(&world, assets_content_area, "text")
            .expect("expected item text component");
        let (resolved_selection, item) =
            super::resolve_selection_click(&world, item_text).expect("expected option hit");
        assert_eq!(resolved_selection, selection_root);

        systems.rx.push_event(
            item_text,
            EventSignal::Click {
                raycaster: item_text,
                renderable: item_text,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );

        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

        let selection = world
            .get_component_by_id_as::<SelectionComponent>(selection_root)
            .expect("expected selection component");

        assert_eq!(selection.selected_component, Some(item));
        assert_eq!(selection.selected_index, Some(0));
        assert!(selection.selected_item.is_some());
    }

    #[test]
    fn selection_system_multiple_mode_toggles_membership() {
        let tmp_dir = temp_asset_directory();
        let asset_path = tmp_dir.join("test_asset.mms");
        std::fs::write(
            &asset_path,
            r#"
                export fn example() {
                    let root = T {}
                    return root
                }

                export fn second_example() {
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
        let render_assets = RenderAssets::new();

        systems
            .asset_system
            .scan_assets_dir(&tmp_dir)
            .expect("scan assets dir");
        systems.selection.install_handlers(&mut systems.rx);

        let parent = world.add_component_boxed_named(
            "parent",
            Box::new(crate::engine::ecs::component::TransformComponent::new()),
        );
        let wrapper = systems
            .asset_system
            .spawn_assets_panel(
                &mut world,
                &render_assets,
                &mut emit,
                parent,
                (0.0, 0.0, 0.0),
            )
            .expect("spawn assets panel");

        let selection_root = world
            .find_component(wrapper, "#assets_selection")
            .expect("expected selection root");
        {
            let selection = world
                .get_component_by_id_as_mut::<SelectionComponent>(selection_root)
                .expect("expected selection component");
            *selection = SelectionComponent::multiple();
        }

        let assets_content_area = world
            .find_component(wrapper, "#assets_content_area")
            .expect("expected assets content area");
        let items = world.find_all_components(assets_content_area, "[name='asset_item']");
        assert!(items.len() >= 2, "expected at least two asset items");
        let first = items[0];
        let second = items[1];

        systems.rx.push_event(
            first,
            EventSignal::Click {
                raycaster: first,
                renderable: first,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );
        systems.rx.push_event(
            second,
            EventSignal::Click {
                raycaster: second,
                renderable: second,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );

        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

        let selection = world
            .get_component_by_id_as::<SelectionComponent>(selection_root)
            .expect("expected selection component");
        assert_eq!(selection.mode, SelectionMode::Multiple);
        assert_eq!(selection.selected_entries.len(), 2);
        assert!(selection.contains(first));
        assert!(selection.contains(second));
        assert_eq!(selection.selected_component, Some(second));

        systems.rx.push_event(
            first,
            EventSignal::Click {
                raycaster: first,
                renderable: first,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );

        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

        let selection = world
            .get_component_by_id_as::<SelectionComponent>(selection_root)
            .expect("expected selection component");
        assert_eq!(selection.selected_entries.len(), 1);
        assert!(!selection.contains(first));
        assert!(selection.contains(second));
        assert_eq!(selection.selected_component, Some(second));
    }

    #[test]
    fn editor_panel_layout_selection_selects_one_panel_at_a_time() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let render_assets = RenderAssets::new();
        let asset_system = crate::engine::ecs::system::AssetSystem::new();

        systems.selection.install_handlers(&mut systems.rx);

        let editor_root =
            world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_root =
            world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let _ = world.add_child(editor_root, scene_root);

        systems.inspector.setup_panels_for_editor(
            &mut systems.rx,
            &mut world,
            &render_assets,
            &mut emit,
            editor_root,
            (-0.7, 1.6, -1.2),
            (-0.7, 1.6, -1.2),
            &asset_system,
        );

        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

        let runtime_ui_root = find_named_root(&world, "editor_runtime_ui_root");
        let selection_root = world
            .find_component(runtime_ui_root, "#editor_panel_layout_selection")
            .expect("expected panel layout selection");
        let world_panel_root = world
            .find_component(runtime_ui_root, "#world_panel_root")
            .expect("expected world panel root");
        let paint_panel_root = world
            .find_component(runtime_ui_root, "#paint_panel_root")
            .expect("expected paint panel root");

        systems.rx.push_event(
            world_panel_root,
            EventSignal::Click {
                raycaster: world_panel_root,
                renderable: world_panel_root,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );

        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

        let selection = world
            .get_component_by_id_as::<SelectionComponent>(selection_root)
            .expect("expected selection component");
        assert_eq!(selection.selected_component, Some(world_panel_root));
        assert_eq!(selection.selected_entries.len(), 1);

        systems.rx.push_event(
            paint_panel_root,
            EventSignal::Click {
                raycaster: paint_panel_root,
                renderable: paint_panel_root,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );

        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

        let selection = world
            .get_component_by_id_as::<SelectionComponent>(selection_root)
            .expect("expected selection component");
        assert_eq!(selection.selected_component, Some(paint_panel_root));
        assert_eq!(selection.selected_entries.len(), 1);
    }

    #[test]
    fn paint_tool_selection_selects_one_option_at_a_time() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let render_assets = RenderAssets::new();
        let asset_system = crate::engine::ecs::system::AssetSystem::new();

        systems.selection.install_handlers(&mut systems.rx);

        let editor_root =
            world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_root =
            world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let _ = world.add_child(editor_root, scene_root);

        systems.inspector.setup_panels_for_editor(
            &mut systems.rx,
            &mut world,
            &render_assets,
            &mut emit,
            editor_root,
            (-0.7, 1.6, -1.2),
            (-0.7, 1.6, -1.2),
            &asset_system,
        );

        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

        let runtime_ui_root = find_named_root(&world, "editor_runtime_ui_root");
        let paint_panel_root = world
            .find_component(runtime_ui_root, "#paint_panel_root")
            .expect("expected paint panel root");
        let selection_root = world
            .find_component(paint_panel_root, "#paint_tool_selection")
            .expect("expected paint tool selection");
        let items = world.find_all_components(paint_panel_root, "[name='paint_panel_item']");
        assert!(items.len() >= 2, "expected at least two paint tool items");

        let first = items[0];
        let second = items[1];

        systems.layout.tick(&mut world, &mut emit);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);
        systems
            .fit_bounds
            .tick(&mut world, &render_assets, &mut emit);

        let first_scale_before = fit_bounds_content_scale(&world, first);
        assert!(
            first_scale_before[0] > 1.0,
            "expected FitBounds to expand first icon before selection"
        );

        systems.rx.push_event(
            first,
            EventSignal::Click {
                raycaster: first,
                renderable: first,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );

        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

        systems.layout.tick(&mut world, &mut emit);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);
        systems
            .fit_bounds
            .tick(&mut world, &render_assets, &mut emit);

        let selection = world
            .get_component_by_id_as::<SelectionComponent>(selection_root)
            .expect("expected selection component");
        assert_eq!(selection.selected_component, Some(first));
        assert_eq!(selection.selected_entries.len(), 1);
        assert_eq!(fit_bounds_content_scale(&world, first), first_scale_before);

        systems.rx.push_event(
            second,
            EventSignal::Click {
                raycaster: second,
                renderable: second,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );

        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

        systems.layout.tick(&mut world, &mut emit);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);
        systems
            .fit_bounds
            .tick(&mut world, &render_assets, &mut emit);

        let selection = world
            .get_component_by_id_as::<SelectionComponent>(selection_root)
            .expect("expected selection component");
        assert_eq!(selection.selected_component, Some(second));
        assert_eq!(selection.selected_entries.len(), 1);
        assert_eq!(fit_bounds_content_scale(&world, first), first_scale_before);
    }

    #[test]
    fn world_panel_row_selection_uses_selection_and_option_components() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let render_assets = RenderAssets::new();
        let asset_system = crate::engine::ecs::system::AssetSystem::new();

        systems.selection.install_handlers(&mut systems.rx);

        let editor_root =
            world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_root =
            world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let scene_child =
            world.add_component_boxed_named("scene_child", Box::new(TransformComponent::new()));
        let _ = world.add_child(editor_root, scene_root);
        let _ = world.add_child(scene_root, scene_child);

        systems.inspector.setup_panels_for_editor(
            &mut systems.rx,
            &mut world,
            &render_assets,
            &mut emit,
            editor_root,
            (-0.7, 1.6, -1.2),
            (-0.7, 1.6, -1.2),
            &asset_system,
        );

        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

        let runtime_ui_root = find_named_root(&world, "editor_runtime_ui_root");
        let world_panel_root = world
            .find_component(runtime_ui_root, "#world_panel_root")
            .expect("expected world panel root");
        let selection_root = world
            .find_component(world_panel_root, "#world_panel_selection")
            .expect("expected world panel selection");
        let panel_layout_selection = world
            .find_component(runtime_ui_root, "#editor_panel_layout_selection")
            .expect("expected panel layout selection");
        let world_panel_root = world
            .find_component(runtime_ui_root, "#world_panel_root")
            .expect("expected world panel root");
        let first_row = world
            .find_component(world_panel_root, "#item_1")
            .expect("expected first selectable row");
        let second_row = world
            .find_component(world_panel_root, "#item_2")
            .expect("expected second selectable row");

        assert!(
            world
                .get_component_by_id_as::<SelectionComponent>(selection_root)
                .is_some(),
            "expected Selection on world panel rows mount"
        );
        assert!(
            world
                .get_component_by_id_as::<OptionComponent>(first_row)
                .is_some()
                || world.children_of(first_row).iter().any(|&child| world
                    .get_component_by_id_as::<OptionComponent>(child)
                    .is_some()),
            "expected Option on selectable world panel row"
        );

        systems.rx.push_event(
            first_row,
            EventSignal::Click {
                raycaster: first_row,
                renderable: first_row,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );

        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

        let selection = world
            .get_component_by_id_as::<SelectionComponent>(selection_root)
            .expect("expected selection component");
        assert_eq!(selection.selected_component, Some(first_row));
        assert_eq!(selection.selected_index, Some(1));
        assert_eq!(selection.selected_entries.len(), 1);

        let panel_selection = world
            .get_component_by_id_as::<SelectionComponent>(panel_layout_selection)
            .expect("expected panel layout selection component");
        assert_eq!(panel_selection.selected_component, Some(world_panel_root));
        assert_eq!(panel_selection.selected_entries.len(), 1);

        systems.rx.push_event(
            second_row,
            EventSignal::Click {
                raycaster: second_row,
                renderable: second_row,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );

        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

        let selection = world
            .get_component_by_id_as::<SelectionComponent>(selection_root)
            .expect("expected selection component");
        assert_eq!(selection.selected_component, Some(second_row));
        assert_eq!(selection.selected_index, Some(2));
        assert_eq!(selection.selected_entries.len(), 1);

        let panel_selection = world
            .get_component_by_id_as::<SelectionComponent>(panel_layout_selection)
            .expect("expected panel layout selection component");
        assert_eq!(panel_selection.selected_component, Some(world_panel_root));
        assert_eq!(panel_selection.selected_entries.len(), 1);
    }

    #[test]
    fn inspector_panel_row_selection_uses_selection_and_option_components() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let render_assets = RenderAssets::new();
        let asset_system = crate::engine::ecs::system::AssetSystem::new();

        systems.selection.install_handlers(&mut systems.rx);

        let editor_root =
            world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_root =
            world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let scene_child =
            world.add_component_boxed_named("scene_child", Box::new(TransformComponent::new()));
        let _ = world.add_child(editor_root, scene_root);
        let _ = world.add_child(scene_root, scene_child);

        systems.inspector.setup_panels_for_editor(
            &mut systems.rx,
            &mut world,
            &render_assets,
            &mut emit,
            editor_root,
            (-0.7, 1.6, -1.2),
            (-0.7, 1.6, -1.2),
            &asset_system,
        );

        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

        let runtime_ui_root = find_named_root(&world, "editor_runtime_ui_root");
        let world_panel_root = world
            .find_component(runtime_ui_root, "#world_panel_root")
            .expect("expected world panel root");
        let world_row = world
            .find_component(world_panel_root, "#item_1")
            .expect("expected first selectable world row");

        systems.rx.push_event(
            world_row,
            EventSignal::Click {
                raycaster: world_row,
                renderable: world_row,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );

        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

        let inspector_panel_root = world
            .find_component(runtime_ui_root, "#inspector_panel_root")
            .expect("expected inspector panel root");
        let inspector_selection = world
            .find_component(inspector_panel_root, "#inspector_panel_selection")
            .expect("expected inspector panel selection");
        let inspector_panel_root = world
            .find_component(runtime_ui_root, "#inspector_panel_root")
            .expect("expected inspector panel root");
        let panel_layout_selection = world
            .find_component(runtime_ui_root, "#editor_panel_layout_selection")
            .expect("expected panel layout selection");
        let inspector_row = world
            .find_component(inspector_panel_root, "#inspector_item_1")
            .expect("expected first selectable inspector row");

        assert!(
            world
                .get_component_by_id_as::<SelectionComponent>(inspector_selection)
                .is_some(),
            "expected Selection on inspector rows mount"
        );
        assert!(
            world
                .get_component_by_id_as::<OptionComponent>(inspector_row)
                .is_some()
                || world.children_of(inspector_row).iter().any(|&child| world
                    .get_component_by_id_as::<OptionComponent>(child)
                    .is_some()),
            "expected Option on selectable inspector row"
        );

        systems.rx.push_event(
            inspector_row,
            EventSignal::Click {
                raycaster: inspector_row,
                renderable: inspector_row,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );

        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

        let selection = world
            .get_component_by_id_as::<SelectionComponent>(inspector_selection)
            .expect("expected inspector selection component");
        assert_eq!(selection.selected_component, Some(inspector_row));
        assert_eq!(selection.selected_index, Some(1));
        assert_eq!(selection.selected_entries.len(), 1);

        let panel_selection = world
            .get_component_by_id_as::<SelectionComponent>(panel_layout_selection)
            .expect("expected panel layout selection component");
        assert_eq!(panel_selection.selected_component, Some(inspector_panel_root));
        assert_eq!(panel_selection.selected_entries.len(), 1);
    }

    #[test]
    fn styled_option_selection_mutates_background_and_restores_previous_style() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let render_assets = RenderAssets::new();

        systems.selection.install_handlers(&mut systems.rx);

        let root = world.add_component_boxed_named("root", Box::new(TransformComponent::new()));
        let layout = world.add_component_boxed(Box::new(LayoutComponent::new(20.0)));
        let _ = world.add_child(root, layout);
        world
            .get_component_by_id_as_mut::<LayoutComponent>(layout)
            .expect("layout")
            .dirty = false;
        let selection = world.add_component_boxed(Box::new(SelectionComponent::new()));
        let _ = world.add_child(root, selection);

        let (first, first_hit, first_style) =
            spawn_test_option_item(&mut world, root, "first_item", true);
        let (_second, second_hit, second_style) =
            spawn_test_option_item(&mut world, root, "second_item", true);
        let first_style = first_style.expect("first style");
        let second_style = second_style.expect("second style");

        systems.rx.push_event(
            first_hit,
            EventSignal::Click {
                raycaster: first_hit,
                renderable: first_hit,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );
        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

        assert_eq!(
            world
                .get_component_by_id_as::<StyleComponent>(first_style)
                .expect("first style")
                .background_color,
            Some(SELECTED_HIGHLIGHT_RGBA)
        );
        assert!(
            world
                .find_component(first, "[name='selection_style_state']")
                .is_some(),
            "expected cached style state helper on first selection"
        );
        assert!(
            world
                .get_component_by_id_as::<LayoutComponent>(layout)
                .expect("layout")
                .dirty,
            "expected selecting styled option to dirty the layout root"
        );

        world
            .get_component_by_id_as_mut::<LayoutComponent>(layout)
            .expect("layout")
            .dirty = false;

        systems.rx.push_event(
            second_hit,
            EventSignal::Click {
                raycaster: second_hit,
                renderable: second_hit,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );
        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

        assert_eq!(
            world
                .get_component_by_id_as::<StyleComponent>(first_style)
                .expect("first style")
                .background_color,
            None
        );
        assert_eq!(
            world
                .get_component_by_id_as::<StyleComponent>(second_style)
                .expect("second style")
                .background_color,
            Some(SELECTED_HIGHLIGHT_RGBA)
        );
        assert!(
            world
                .find_component(first, "[name='selection_style_state']")
                .is_none(),
            "expected old styled selection cache to be removed on deselect"
        );
    }

    #[test]
    fn unstyled_option_selection_spawns_bounds_driven_overlay() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let render_assets = RenderAssets::new();

        systems.selection.install_handlers(&mut systems.rx);

        let root = world.add_component_boxed_named("root", Box::new(TransformComponent::new()));
        let selection = world.add_component_boxed(Box::new(SelectionComponent::new()));
        let _ = world.add_child(root, selection);

        let (item, hit, _) = spawn_test_option_item(&mut world, root, "unstyled_item", false);

        systems.rx.push_event(
            hit,
            EventSignal::Click {
                raycaster: hit,
                renderable: hit,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );
        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

        let highlight = world
            .find_component(item, "[name='selection_highlight']")
            .expect("expected bounds-driven selection overlay");
        let transform = world
            .get_component_by_id_as::<TransformComponent>(highlight)
            .expect("highlight transform");
        assert_eq!(
            transform.transform.translation,
            [0.0, 0.0, OVERLAY_HIGHLIGHT_Z_OFFSET]
        );
        assert_eq!(
            transform.transform.scale,
            [1.0, 1.0, OVERLAY_HIGHLIGHT_Z_THICKNESS]
        );
    }

    #[test]
    fn selection_set_intent_updates_selection_state() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let render_assets = RenderAssets::new();

        systems.selection.install_handlers(&mut systems.rx);

        let root = world.add_component_boxed_named("root", Box::new(TransformComponent::new()));
        let selection_root = world.add_component_boxed(Box::new(SelectionComponent::multiple()));
        let _ = world.add_child(root, selection_root);

        let (first, _, _) = spawn_test_option_item(&mut world, root, "first_item", false);
        let (second, _, _) = spawn_test_option_item(&mut world, root, "second_item", false);

        systems.rx.push_intent_now(
            selection_root,
            IntentValue::SelectionSet {
                component_ids: vec![selection_root],
                entries: vec![
                    SelectionEntry {
                        index: Some(0),
                        item: Some("first_item".to_string()),
                        component: first,
                    },
                    SelectionEntry {
                        index: Some(1),
                        item: Some("second_item".to_string()),
                        component: second,
                    },
                ],
                primary: Some(second),
            },
        );

        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

        let selection = world
            .get_component_by_id_as::<SelectionComponent>(selection_root)
            .expect("expected selection component");
        assert_eq!(selection.selected_entries.len(), 2);
        assert_eq!(selection.selected_component, Some(second));
        assert!(selection.contains(first));
        assert!(selection.contains(second));
    }

    #[test]
    fn selection_click_emits_selection_events() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let render_assets = RenderAssets::new();

        systems.selection.install_handlers(&mut systems.rx);

        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_changed = Arc::clone(&seen);
        systems.rx.add_global_handler_closure(
            SignalKind::SelectionChanged,
            move |_world, _emit, signal| {
                if let Some(EventSignal::SelectionChanged {
                    selection_root,
                    selected_component,
                    ..
                }) = signal.event.as_ref()
                {
                    seen_changed
                        .lock()
                        .expect("selection events mutex poisoned")
                        .push(format!("{selection_root:?}:{selected_component:?}"));
                }
            },
        );

        let seen_added = Arc::clone(&seen);
        systems.rx.add_global_handler_closure(
            SignalKind::SelectionAdded,
            move |_world, _emit, signal| {
                if let Some(EventSignal::SelectionAdded {
                    selection_root,
                    entry,
                }) = signal.event.as_ref()
                {
                    seen_added
                        .lock()
                        .expect("selection events mutex poisoned")
                        .push(format!("{selection_root:?}:+{:?}", entry.component));
                }
            },
        );

        let root = world.add_component_boxed_named("root", Box::new(TransformComponent::new()));
        let selection_root = world.add_component_boxed(Box::new(SelectionComponent::new()));
        let _ = world.add_child(root, selection_root);

        let (first, first_hit, _) = spawn_test_option_item(&mut world, root, "first_item", false);

        systems.rx.push_event(
            first_hit,
            EventSignal::Click {
                raycaster: first_hit,
                renderable: first_hit,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );

        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

        let seen = seen.lock().expect("selection events mutex poisoned");
        assert!(
            seen.iter()
                .any(|value| value.contains(&format!("+{first:?}"))),
            "expected selection-added event"
        );
        assert!(
            seen.iter()
                .any(|value| value.contains(&format!("Some({first:?})"))),
            "expected selection-changed event"
        );
    }
}
