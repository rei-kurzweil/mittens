use crate::engine::ecs::component::{
    ColorComponent, EditorComponent, EmissiveComponent, InspectorPanelComponent,
    OverlayComponent, RaycastableComponent, SelectableComponent, TextBackgroundComponent,
    TransformComponent, WorldPanelComponent,
};
use crate::engine::ecs::rx::RxWorld;
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, SignalEmitter, SignalKind, World};

const ROW_HEIGHT: f32 = 0.045;
const TEXT_SCALE: f32 = 0.04;
const INDENT_UNIT: f32 = 0.06;
const MAX_ROWS: usize = 30;
const MAX_DEPTH: usize = 5;
const PANEL_V_PADDING: f32 = 0.35;
/// Extra glyph-space padding_bottom so adjacent row backgrounds touch exactly.
const ROW_GAP_FILL: f32 = ROW_HEIGHT / TEXT_SCALE - 1.0;

/// Panel background color: light grey, semi-transparent.
const BG_COLOR: [f32; 4] = [0.92, 0.92, 0.92, 0.80];
/// Normal text color: black.
const TEXT_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
/// Highlighted row text color: dark green.
const HIGHLIGHT_COLOR: [f32; 4] = [0.0, 0.45, 0.0, 1.0];

#[derive(Debug, Default)]
pub struct InspectorSystem;

impl InspectorSystem {
    pub fn new() -> Self {
        Self
    }

    pub fn setup_panels_for_editor(
        &mut self,
        rx: &mut RxWorld,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        editor_root: ComponentId,
        world_panel_pos: (f32, f32, f32),
        inspector_panel_pos: (f32, f32, f32),
    ) {
        let (wpc_id, wpa_id) = spawn_world_panel(world, emit, editor_root, world_panel_pos);
        let ipc_id = spawn_inspector_panel(world, emit, editor_root, inspector_panel_pos);

        rebuild_world_panel(world, emit, wpc_id, editor_root, None);
        rebuild_inspector_panel(world, emit, ipc_id, None);

        // DragStart on the world panel anchor → select the clicked node.
        rx.add_handler_closure(
            SignalKind::DragStart,
            wpa_id,
            move |world, emit, env| {
                let Some(EventSignal::DragStart { renderable, .. }) = env.event.as_ref() else {
                    return;
                };
                let renderable = *renderable;

                let (row_roots, row_to_node) = {
                    let Some(wpc) =
                        world.get_component_by_id_as::<WorldPanelComponent>(wpc_id)
                    else {
                        return;
                    };
                    (wpc.row_roots.clone(), wpc.row_to_node.clone())
                };

                let Some(idx) = find_ancestor_in_list(world, renderable, &row_roots) else {
                    return;
                };
                let Some(&node_id) = row_to_node.get(idx) else {
                    return;
                };

                if let Some(ed) =
                    world.get_component_by_id_as_mut::<EditorComponent>(editor_root)
                {
                    ed.selected = Some(node_id);
                }
                emit.push_event(
                    editor_root,
                    EventSignal::SelectionChanged {
                        editor_root,
                        selected: Some(node_id),
                    },
                );
            },
        );

        // SelectionChanged → rebuild both panels.
        rx.add_handler_closure(
            SignalKind::SelectionChanged,
            editor_root,
            move |world, emit, env| {
                let Some(EventSignal::SelectionChanged { selected, .. }) = env.event.as_ref()
                else {
                    return;
                };
                let selected = *selected;
                rebuild_world_panel(world, emit, wpc_id, editor_root, selected);
                rebuild_inspector_panel(world, emit, ipc_id, selected);
            },
        );
    }
}

// ---------------------------------------------------------------------------
// Panel spawn helpers
// ---------------------------------------------------------------------------

fn spawn_world_panel(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    editor_root: ComponentId,
    pos: (f32, f32, f32),
) -> (ComponentId, ComponentId) {
    let wpa = world.add_component_boxed_named(
        "world_panel_anchor",
        Box::new(SelectableComponent::off()),
    );
    let wpo = world.add_component_boxed_named(
        "world_panel_overlay",
        Box::new(OverlayComponent::new()),
    );
    let wpc = world.add_component_boxed_named(
        "world_panel",
        Box::new(WorldPanelComponent::new()),
    );
    let wpr = world.add_component_boxed_named(
        "world_panel_rows",
        Box::new(TransformComponent::new().with_position(pos.0, pos.1, pos.2)),
    );

    let _ = world.add_child(wpa, wpo);
    let _ = world.add_child(wpo, wpc);
    let _ = world.add_child(wpc, wpr);

    if let Some(c) = world.get_component_by_id_as_mut::<WorldPanelComponent>(wpc) {
        c.editor_root = Some(editor_root);
        c.rows_anchor = Some(wpr);
    }

    world.init_component_tree(wpa, emit);
    (wpc, wpa)
}

fn spawn_inspector_panel(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    editor_root: ComponentId,
    pos: (f32, f32, f32),
) -> ComponentId {
    let ipa = world.add_component_boxed_named(
        "inspector_panel_anchor",
        Box::new(SelectableComponent::off()),
    );
    let ipo = world.add_component_boxed_named(
        "inspector_panel_overlay",
        Box::new(OverlayComponent::new()),
    );
    let ipc = world.add_component_boxed_named(
        "inspector_panel",
        Box::new(InspectorPanelComponent::new()),
    );
    let ipr = world.add_component_boxed_named(
        "inspector_panel_rows",
        Box::new(TransformComponent::new().with_position(pos.0, pos.1, pos.2)),
    );

    let _ = world.add_child(ipa, ipo);
    let _ = world.add_child(ipo, ipc);
    let _ = world.add_child(ipc, ipr);

    if let Some(c) = world.get_component_by_id_as_mut::<InspectorPanelComponent>(ipc) {
        c.editor_root = Some(editor_root);
        c.rows_anchor = Some(ipr);
    }

    world.init_component_tree(ipa, emit);
    ipc
}

// ---------------------------------------------------------------------------
// Panel rebuild helpers
// ---------------------------------------------------------------------------

fn rebuild_world_panel(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    wpc_id: ComponentId,
    editor_root: ComponentId,
    selected: Option<ComponentId>,
) {
    let rows_anchor = {
        let Some(wpc) = world.get_component_by_id_as::<WorldPanelComponent>(wpc_id) else {
            return;
        };
        wpc.rows_anchor
    };
    let Some(rows_anchor) = rows_anchor else {
        return;
    };

    // Clear ALL current children of rows_anchor synchronously, then queue their removal.
    let old_children: Vec<ComponentId> = world.children_of(rows_anchor).to_vec();
    for old in &old_children {
        world.detach_from_parent(*old);
        emit.push_intent_now(
            rows_anchor,
            IntentValue::RemoveSubtree {
                component_ids: vec![*old],
            },
        );
    }

    let nodes = collect_visible_nodes(world, editor_root, MAX_DEPTH);
    let total_rows = nodes.len().min(MAX_ROWS);
    let highlighted = find_highlighted(selected, &nodes, world);
    let mut new_rows = Vec::new();
    let mut new_row_to_node = Vec::new();

    for (i, (node_id, depth, label)) in nodes.iter().take(MAX_ROWS).enumerate() {
        let is_highlighted = highlighted == Some(*node_id);
        let text = if is_highlighted {
            format!("> {label}")
        } else {
            label.clone()
        };
        let text_color = if is_highlighted { HIGHLIGHT_COLOR } else { TEXT_COLOR };

        let row_t = world.add_component_boxed_named(
            format!("wp_row_{i}"),
            Box::new(
                TransformComponent::new()
                    .with_position(*depth as f32 * INDENT_UNIT, -(i as f32) * ROW_HEIGHT, 0.0)
                    .with_scale(TEXT_SCALE, TEXT_SCALE, TEXT_SCALE),
            ),
        );
        let _ = world.add_child(rows_anchor, row_t);

        // ColorComponent sets text color; consistent structure for all rows.
        let color_node = world.add_component_boxed_named(
            "wp_color",
            Box::new(ColorComponent::rgba(
                text_color[0],
                text_color[1],
                text_color[2],
                text_color[3],
            )),
        );
        let _ = world.add_child(row_t, color_node);

        let row_text = world.add_component_boxed_named(
            format!("wp_text_{i}"),
            Box::new(crate::engine::ecs::component::TextComponent::new(&text)),
        );
        let _ = world.add_child(color_node, row_text);

        let emissive =
            world.add_component_boxed_named("wp_emit", Box::new(EmissiveComponent::on()));
        let _ = world.add_child(row_text, emissive);

        let rc = world
            .add_component_boxed_named("wp_rc", Box::new(RaycastableComponent::enabled()));
        let _ = world.add_child(row_text, rc);

        let bg = world
            .add_component_boxed_named("wp_bg", Box::new(panel_row_bg(i, total_rows)));
        let _ = world.add_child(row_text, bg);
        let bg_col = world.add_component_boxed_named(
            "wp_bg_color",
            Box::new(ColorComponent::rgba(BG_COLOR[0], BG_COLOR[1], BG_COLOR[2], BG_COLOR[3])),
        );
        let _ = world.add_child(bg, bg_col);

        new_rows.push(row_t);
        new_row_to_node.push(*node_id);
    }

    world.init_component_tree(rows_anchor, emit);

    if let Some(wpc) = world.get_component_by_id_as_mut::<WorldPanelComponent>(wpc_id) {
        wpc.row_roots = new_rows;
        wpc.row_to_node = new_row_to_node;
    }
}

fn rebuild_inspector_panel(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    ipc_id: ComponentId,
    selected: Option<ComponentId>,
) {
    let rows_anchor = {
        let Some(ipc) = world.get_component_by_id_as::<InspectorPanelComponent>(ipc_id) else {
            return;
        };
        ipc.rows_anchor
    };
    let Some(rows_anchor) = rows_anchor else {
        return;
    };

    // Clear ALL current children synchronously.
    let old_children: Vec<ComponentId> = world.children_of(rows_anchor).to_vec();
    for old in &old_children {
        world.detach_from_parent(*old);
        emit.push_intent_now(
            rows_anchor,
            IntentValue::RemoveSubtree {
                component_ids: vec![*old],
            },
        );
    }

    let lines: Vec<String> = if let Some(sel) = selected {
        if let Some(node) = world.get_component_node(sel) {
            let type_name = node.component.name().to_string();
            let display_name = &node.name;
            let header = if display_name == &type_name {
                type_name
            } else {
                format!("{type_name}: {display_name}")
            };
            vec![header]
        } else {
            vec!["(unknown)".to_string()]
        }
    } else {
        vec![]
    };

    let total_rows = lines.len();
    let mut new_rows = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let row_t = world.add_component_boxed_named(
            format!("ip_row_{i}"),
            Box::new(
                TransformComponent::new()
                    .with_position(0.0, -(i as f32) * ROW_HEIGHT, 0.0)
                    .with_scale(TEXT_SCALE, TEXT_SCALE, TEXT_SCALE),
            ),
        );
        let _ = world.add_child(rows_anchor, row_t);

        let color_node = world.add_component_boxed_named(
            "ip_color",
            Box::new(ColorComponent::rgba(
                TEXT_COLOR[0],
                TEXT_COLOR[1],
                TEXT_COLOR[2],
                TEXT_COLOR[3],
            )),
        );
        let _ = world.add_child(row_t, color_node);

        let row_text = world.add_component_boxed_named(
            format!("ip_text_{i}"),
            Box::new(crate::engine::ecs::component::TextComponent::new(line)),
        );
        let _ = world.add_child(color_node, row_text);

        let emissive =
            world.add_component_boxed_named("ip_emit", Box::new(EmissiveComponent::on()));
        let _ = world.add_child(row_text, emissive);

        let bg = world
            .add_component_boxed_named("ip_bg", Box::new(panel_row_bg(i, total_rows)));
        let _ = world.add_child(row_text, bg);
        let bg_col = world.add_component_boxed_named(
            "ip_bg_color",
            Box::new(ColorComponent::rgba(BG_COLOR[0], BG_COLOR[1], BG_COLOR[2], BG_COLOR[3])),
        );
        let _ = world.add_child(bg, bg_col);

        new_rows.push(row_t);
    }

    world.init_component_tree(rows_anchor, emit);

    if let Some(ipc) = world.get_component_by_id_as_mut::<InspectorPanelComponent>(ipc_id) {
        ipc.row_roots = new_rows;
        ipc.inspected = selected;
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn panel_row_bg(i: usize, total: usize) -> TextBackgroundComponent {
    TextBackgroundComponent::new()
        .with_padding_top(if i == 0 { PANEL_V_PADDING } else { 0.0 })
        .with_padding_bottom(if i + 1 == total { PANEL_V_PADDING } else { ROW_GAP_FILL })
}

fn find_ancestor_in_list(
    world: &World,
    start: ComponentId,
    list: &[ComponentId],
) -> Option<usize> {
    let mut cur = Some(start);
    while let Some(node) = cur {
        if let Some(idx) = list.iter().position(|&r| r == node) {
            return Some(idx);
        }
        cur = world.parent_of(node);
    }
    None
}

fn find_highlighted(
    selected: Option<ComponentId>,
    nodes: &[(ComponentId, usize, String)],
    world: &World,
) -> Option<ComponentId> {
    let sel = selected?;
    let mut cur = Some(sel);
    while let Some(node) = cur {
        if nodes.iter().any(|(id, _, _)| *id == node) {
            return Some(node);
        }
        cur = world.parent_of(node);
    }
    None
}

// ---------------------------------------------------------------------------
// Tree traversal
// ---------------------------------------------------------------------------

fn collect_visible_nodes(
    world: &World,
    editor_root: ComponentId,
    max_depth: usize,
) -> Vec<(ComponentId, usize, String)> {
    let mut result = Vec::new();
    let mut stack: Vec<(ComponentId, usize)> = world
        .children_of(editor_root)
        .iter()
        .rev()
        .map(|&c| (c, 0))
        .collect();

    while let Some((node, depth)) = stack.pop() {
        if world
            .get_component_by_id_as::<SelectableComponent>(node)
            .map(|s| !s.enabled)
            .unwrap_or(false)
        {
            continue;
        }

        let label = node_label(world, node);
        result.push((node, depth, label));

        if depth < max_depth {
            for &child in world.children_of(node).iter().rev() {
                stack.push((child, depth + 1));
            }
        }
    }

    result
}

fn node_label(world: &World, id: ComponentId) -> String {
    let Some(node) = world.get_component_node(id) else {
        return format!("{id:?}");
    };
    let type_name = node.component.name();
    let display = &node.name;
    if display == type_name {
        type_name.to_string()
    } else {
        format!("{type_name}: {display}")
    }
}
