use crate::engine::ecs::component::{
    ColorComponent, EmissiveComponent, InspectorPanelComponent, OverlayComponent,
    RaycastableComponent, ScrollingComponent, SelectableComponent, TextBackgroundComponent,
    TransformComponent, WorldPanelComponent,
};
use crate::engine::ecs::system::editor_system::select_editor_target;
use crate::engine::ecs::system::LayoutSystem;
use crate::engine::ecs::rx::RxWorld;
use crate::engine::ecs::{
    ComponentId, EventSignal, IntentValue, SignalEmitter, SignalKind, World,
};

const ROW_HEIGHT: f32 = 0.090;
const TEXT_SCALE: f32 = 0.08;
const INDENT_UNIT: f32 = 0.12;
const PAGE_SIZE: usize = 30;
const MAX_DEPTH: usize = 5;
const PANEL_V_PADDING: f32 = 0.35;
/// Extra glyph-space padding_bottom so adjacent row backgrounds touch exactly.
const ROW_GAP_FILL: f32 = ROW_HEIGHT / TEXT_SCALE - 1.0;
/// Gap between world panel right edge and inspector panel left edge (overlay units).
const PANEL_GAP: f32 = 0.12;

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
        // Compute estimated world panel width so we can auto-place inspector to the right.
        // (World matrices aren't ready yet during setup, so we use an analytic estimate.)
        let estimated_wp_width = LayoutSystem::estimate_panel_width(
            crate::engine::ecs::component::TextComponent::DEFAULT_WRAP_AT,
            TEXT_SCALE,
            MAX_DEPTH as f32 * INDENT_UNIT,
        );

        // If caller used the default inspector position (same x as world panel), push it right.
        let inspector_pos = if (inspector_panel_pos.0 - world_panel_pos.0).abs() < 0.01 {
            (
                world_panel_pos.0 + estimated_wp_width + PANEL_GAP,
                inspector_panel_pos.1,
                inspector_panel_pos.2,
            )
        } else {
            inspector_panel_pos
        };

        let (wpc_id, wpa_id, wsc_id) =
            spawn_world_panel(world, emit, editor_root, world_panel_pos);
        let (ipc_id, ipa_id, isc_id) =
            spawn_inspector_panel(world, emit, editor_root, inspector_pos);

        rebuild_world_panel(world, emit, wpc_id, editor_root, None, 0);
        rebuild_inspector_panel(world, emit, ipc_id, None, 0);

        // --- World panel: Click on a row → select that node ---
        rx.add_handler_closure(
            SignalKind::Click,
            wpa_id,
            move |world, emit, env| {
                let Some(EventSignal::Click { renderable, .. }) = env.event.as_ref() else {
                    return;
                };
                let renderable = *renderable;

                let (row_roots, row_to_node, window_start) = {
                    let Some(wpc) = world.get_component_by_id_as::<WorldPanelComponent>(wpc_id)
                    else {
                        return;
                    };
                    (
                        wpc.row_roots.clone(),
                        wpc.row_to_node.clone(),
                        wpc.scroll_offset_rows as usize,
                    )
                };

                let Some(panel_idx) = find_ancestor_in_list(world, renderable, &row_roots) else {
                    return;
                };
                let global_idx = window_start + panel_idx;
                let Some(&node_id) = row_to_node.get(panel_idx) else {
                    return;
                };
                let _ = global_idx; // used for future reference
                select_editor_target(world, emit, editor_root, node_id, false);
            },
        );

        // --- World panel: DragMove on scroll anchor → scroll ---
        rx.add_handler_closure(
            SignalKind::DragMove,
            wpa_id,
            move |world, emit, env| {
                let Some(EventSignal::DragMove { delta_world, .. }) = env.event.as_ref() else {
                    return;
                };
                let dy = delta_world[1];
                let (new_start, new_end) = {
                    let Some(sc) =
                        world.get_component_by_id_as_mut::<ScrollingComponent>(wsc_id)
                    else {
                        return;
                    };
                    match sc.apply_drag(dy) {
                        Some(range) => range,
                        None => return,
                    }
                };
                // Update scroll_offset_rows on the WorldPanelComponent for compatibility.
                if let Some(wpc) = world.get_component_by_id_as_mut::<WorldPanelComponent>(wpc_id)
                {
                    wpc.scroll_offset_rows = new_start as i32;
                }
                emit.push_event(
                    wsc_id,
                    crate::engine::ecs::EventSignal::ScrollChanged {
                        scroll_component: wsc_id,
                        window_start: new_start,
                        window_end: new_end,
                    },
                );
            },
        );

        // --- World panel: ScrollChanged → rebuild rows for new window ---
        rx.add_handler_closure(
            SignalKind::ScrollChanged,
            wsc_id,
            move |world, emit, env| {
                let Some(EventSignal::ScrollChanged { window_start, .. }) = env.event.as_ref()
                else {
                    return;
                };
                let ws = *window_start;
                let sel = world
                    .get_component_by_id_as::<WorldPanelComponent>(wpc_id)
                    .and_then(|w| {
                        // Retrieve the editor selection via EditorComponent.
                        let er = w.editor_root?;
                        world
                            .get_component_by_id_as::<crate::engine::ecs::component::EditorComponent>(er)
                            .and_then(|ed| ed.selected)
                    });
                rebuild_world_panel(world, emit, wpc_id, editor_root, sel, ws);
            },
        );

        // --- Inspector panel: DragMove → scroll ---
        rx.add_handler_closure(
            SignalKind::DragMove,
            ipa_id,
            move |world, emit, env| {
                let Some(EventSignal::DragMove { delta_world, .. }) = env.event.as_ref() else {
                    return;
                };
                let dy = delta_world[1];
                let (new_start, new_end) = {
                    let Some(sc) =
                        world.get_component_by_id_as_mut::<ScrollingComponent>(isc_id)
                    else {
                        return;
                    };
                    match sc.apply_drag(dy) {
                        Some(range) => range,
                        None => return,
                    }
                };
                emit.push_event(
                    isc_id,
                    crate::engine::ecs::EventSignal::ScrollChanged {
                        scroll_component: isc_id,
                        window_start: new_start,
                        window_end: new_end,
                    },
                );
            },
        );

        // --- Inspector panel: ScrollChanged → rebuild ---
        rx.add_handler_closure(
            SignalKind::ScrollChanged,
            isc_id,
            move |world, emit, env| {
                let Some(EventSignal::ScrollChanged { window_start, .. }) = env.event.as_ref()
                else {
                    return;
                };
                let ws = *window_start;
                let sel = world
                    .get_component_by_id_as::<InspectorPanelComponent>(ipc_id)
                    .and_then(|i| i.inspected);
                rebuild_inspector_panel(world, emit, ipc_id, sel, ws);
            },
        );

        // --- SelectionChanged → rebuild both panels ---
        rx.add_handler_closure(
            SignalKind::SelectionChanged,
            editor_root,
            move |world, emit, env| {
                let Some(EventSignal::SelectionChanged { selected, .. }) = env.event.as_ref()
                else {
                    return;
                };
                let selected = *selected;

                // Reset world panel scroll on selection change.
                let wp_ws = if let Some(sc) =
                    world.get_component_by_id_as_mut::<ScrollingComponent>(wsc_id)
                {
                    sc.scroll_offset = 0.0;
                    sc.last_window_start = 0;
                    0
                } else {
                    0
                };
                if let Some(wpc) = world.get_component_by_id_as_mut::<WorldPanelComponent>(wpc_id)
                {
                    wpc.scroll_offset_rows = 0;
                }

                rebuild_world_panel(world, emit, wpc_id, editor_root, selected, wp_ws);
                rebuild_inspector_panel(world, emit, ipc_id, selected, 0);

                // Update ScrollingComponent.total_items for the world panel.
                let nodes = collect_visible_nodes(world, editor_root, MAX_DEPTH);
                if let Some(sc) =
                    world.get_component_by_id_as_mut::<ScrollingComponent>(wsc_id)
                {
                    sc.total_items = nodes.len();
                    sc.clamp_to_total();
                }
            },
        );
    }
}

// ---------------------------------------------------------------------------
// Panel spawn helpers
// ---------------------------------------------------------------------------

/// Returns `(panel_component_id, panel_anchor_id, scroll_component_id)`.
fn spawn_world_panel(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    editor_root: ComponentId,
    pos: (f32, f32, f32),
) -> (ComponentId, ComponentId, ComponentId) {
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
    let wsc = world.add_component_boxed_named(
        "world_panel_scroll",
        Box::new(ScrollingComponent::new(ROW_HEIGHT, PAGE_SIZE)),
    );
    let wpr = world.add_component_boxed_named(
        "world_panel_rows",
        Box::new(TransformComponent::new().with_position(pos.0, pos.1, pos.2)),
    );

    let _ = world.add_child(wpa, wpo);
    let _ = world.add_child(wpo, wpc);
    let _ = world.add_child(wpc, wsc);
    let _ = world.add_child(wsc, wpr);

    if let Some(c) = world.get_component_by_id_as_mut::<WorldPanelComponent>(wpc) {
        c.editor_root = Some(editor_root);
        c.rows_anchor = Some(wpr);
    }

    world.init_component_tree(wpa, emit);
    (wpc, wpa, wsc)
}

/// Returns `(panel_component_id, panel_anchor_id, scroll_component_id)`.
fn spawn_inspector_panel(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    editor_root: ComponentId,
    pos: (f32, f32, f32),
) -> (ComponentId, ComponentId, ComponentId) {
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
    let isc = world.add_component_boxed_named(
        "inspector_panel_scroll",
        Box::new(ScrollingComponent::new(ROW_HEIGHT, PAGE_SIZE)),
    );
    let ipr = world.add_component_boxed_named(
        "inspector_panel_rows",
        Box::new(TransformComponent::new().with_position(pos.0, pos.1, pos.2)),
    );

    let _ = world.add_child(ipa, ipo);
    let _ = world.add_child(ipo, ipc);
    let _ = world.add_child(ipc, isc);
    let _ = world.add_child(isc, ipr);

    if let Some(c) = world.get_component_by_id_as_mut::<InspectorPanelComponent>(ipc) {
        c.editor_root = Some(editor_root);
        c.rows_anchor = Some(ipr);
    }

    world.init_component_tree(ipa, emit);
    (ipc, ipa, isc)
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
    window_start: usize,
) {
    let rows_anchor = {
        let Some(wpc) = world.get_component_by_id_as::<WorldPanelComponent>(wpc_id) else {
            return;
        };
        wpc.rows_anchor
    };
    let Some(rows_anchor) = rows_anchor else { return };

    // Clear current row children.
    let old_children: Vec<ComponentId> = world.children_of(rows_anchor).to_vec();
    for old in &old_children {
        world.detach_from_parent(*old);
        emit.push_intent_now(
            rows_anchor,
            IntentValue::RemoveSubtree { component_ids: vec![*old] },
        );
    }

    let nodes = collect_visible_nodes(world, editor_root, MAX_DEPTH);
    let total = nodes.len();
    let win_start = window_start.min(total.saturating_sub(1));
    let win_end = (win_start + PAGE_SIZE).min(total);
    let window = &nodes[win_start..win_end];
    let visible_count = window.len();

    let highlighted = find_highlighted(selected, &nodes, world);
    let mut new_rows = Vec::new();
    let mut new_row_to_node = Vec::new();

    for (panel_i, (node_id, depth, label)) in window.iter().enumerate() {
        let is_highlighted = highlighted == Some(*node_id);
        let text = if is_highlighted { format!("> {label}") } else { label.clone() };
        let text_color = if is_highlighted { HIGHLIGHT_COLOR } else { TEXT_COLOR };

        let row_t = world.add_component_boxed_named(
            format!("wp_row_{panel_i}"),
            Box::new(
                TransformComponent::new()
                    .with_position(
                        *depth as f32 * INDENT_UNIT,
                        -(panel_i as f32) * ROW_HEIGHT,
                        0.0,
                    )
                    .with_scale(TEXT_SCALE, TEXT_SCALE, TEXT_SCALE),
            ),
        );
        let _ = world.add_child(rows_anchor, row_t);

        let color_node = world.add_component_boxed_named(
            "wp_color",
            Box::new(ColorComponent::rgba(
                text_color[0], text_color[1], text_color[2], text_color[3],
            )),
        );
        let _ = world.add_child(row_t, color_node);

        let row_text = world.add_component_boxed_named(
            format!("wp_text_{panel_i}"),
            Box::new(crate::engine::ecs::component::TextComponent::new(&text)),
        );
        let _ = world.add_child(color_node, row_text);

        let emissive =
            world.add_component_boxed_named("wp_emit", Box::new(EmissiveComponent::on()));
        let _ = world.add_child(row_text, emissive);

        let rc = world
            .add_component_boxed_named("wp_rc", Box::new(RaycastableComponent::enabled()));
        let _ = world.add_child(row_text, rc);

        let bg = world.add_component_boxed_named(
            "wp_bg",
            Box::new(panel_row_bg(panel_i, visible_count)),
        );
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
    window_start: usize,
) {
    let rows_anchor = {
        let Some(ipc) = world.get_component_by_id_as::<InspectorPanelComponent>(ipc_id) else {
            return;
        };
        ipc.rows_anchor
    };
    let Some(rows_anchor) = rows_anchor else { return };

    let old_children: Vec<ComponentId> = world.children_of(rows_anchor).to_vec();
    for old in &old_children {
        world.detach_from_parent(*old);
        emit.push_intent_now(
            rows_anchor,
            IntentValue::RemoveSubtree { component_ids: vec![*old] },
        );
    }

    let lines: Vec<String> = if let Some(sel) = selected {
        if let Some(node) = world.get_component_node(sel) {
            let type_name = &node.component_type;
            let header = if node.name.is_empty() {
                type_name.clone()
            } else {
                format!("{type_name}: {}", node.name)
            };
            vec![header]
        } else {
            vec!["(unknown)".to_string()]
        }
    } else {
        vec![]
    };

    let total = lines.len();
    let win_start = window_start.min(total.saturating_sub(1).max(0));
    let win_end = (win_start + PAGE_SIZE).min(total);
    let window = &lines[win_start..win_end];
    let visible_count = window.len();

    let mut new_rows = Vec::new();
    for (panel_i, line) in window.iter().enumerate() {
        let row_t = world.add_component_boxed_named(
            format!("ip_row_{panel_i}"),
            Box::new(
                TransformComponent::new()
                    .with_position(0.0, -(panel_i as f32) * ROW_HEIGHT, 0.0)
                    .with_scale(TEXT_SCALE, TEXT_SCALE, TEXT_SCALE),
            ),
        );
        let _ = world.add_child(rows_anchor, row_t);

        let color_node = world.add_component_boxed_named(
            "ip_color",
            Box::new(ColorComponent::rgba(
                TEXT_COLOR[0], TEXT_COLOR[1], TEXT_COLOR[2], TEXT_COLOR[3],
            )),
        );
        let _ = world.add_child(row_t, color_node);

        let row_text = world.add_component_boxed_named(
            format!("ip_text_{panel_i}"),
            Box::new(crate::engine::ecs::component::TextComponent::new(line)),
        );
        let _ = world.add_child(color_node, row_text);

        let emissive =
            world.add_component_boxed_named("ip_emit", Box::new(EmissiveComponent::on()));
        let _ = world.add_child(row_text, emissive);

        let bg = world.add_component_boxed_named(
            "ip_bg",
            Box::new(panel_row_bg(panel_i, visible_count)),
        );
        let _ = world.add_child(row_text, bg);

        let cutout = world.add_component_boxed_named(
            "ip_cutout",
            Box::new(crate::engine::ecs::component::TransparentCutoutComponent::new()),
        );
        let _ = world.add_child(row_text, cutout);

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
        if should_skip_world_panel_node(world, node) {
            continue;
        }
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

fn should_skip_world_panel_node(world: &World, node: ComponentId) -> bool {
    if world.component_name(node) == Some("editor_gizmo_anchor") {
        return true;
    }
    let mut cur = Some(node);
    while let Some(cid) = cur {
        if world
            .get_component_by_id_as::<crate::engine::ecs::component::TransformGizmoComponent>(cid)
            .is_some()
        {
            return true;
        }
        cur = world.parent_of(cid);
    }
    false
}

fn node_label(world: &World, id: ComponentId) -> String {
    let Some(node) = world.get_component_node(id) else {
        return format!("{id:?}");
    };
    if node.name.is_empty() {
        node.component_type.clone()
    } else {
        format!("{}: {}", node.component_type, node.name)
    }
}
