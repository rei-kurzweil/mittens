use crate::engine::ecs::component::{
    ColorComponent, EmissiveComponent, HtmlElementComponent, InspectorPanelComponent,
    LayoutComponent, OpacityComponent, OverlayComponent, RaycastableComponent,
    RaycastableShapeComponent, RaycastableShapeType, RenderableComponent, ScrollingComponent,
    SelectableComponent, StyleComponent, TextBackgroundComponent, TransformComponent,
    TransformGizmoComponent, WorldPanelComponent,
    style::{EdgeInsets, SizeDimension},
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
/// Indent per depth level in glyph units (= INDENT_UNIT / TEXT_SCALE).
const INDENT_UNIT_GU: f32 = INDENT_UNIT / TEXT_SCALE;
const PAGE_SIZE: usize = 30;
const MAX_DEPTH: usize = 5;
const PANEL_V_PADDING: f32 = 0.35;
/// Extra glyph-space padding_bottom so adjacent row backgrounds touch exactly.
const ROW_GAP_FILL: f32 = ROW_HEIGHT / TEXT_SCALE - 1.0;
/// Gap between world panel right edge and inspector panel left edge (overlay units).
const PANEL_GAP: f32 = 0.12;
/// Extra margin around the panel that the drag plane extends beyond content edges.
const DRAG_MARGIN: f32 = 0.15;
/// Z offset of the drag plane relative to panel content (negative = behind content, away from camera).
/// Must be more negative than text background quads, which land at z_offset(-0.1) * TEXT_SCALE(0.08) = -0.008.
const DRAG_PLANE_Z_OFFSET: f32 = -0.015;
/// Drag plane debug color: translucent blue.
const DRAG_PLANE_COLOR: [f32; 4] = [0.3, 0.5, 1.0, 1.0];
const DRAG_PLANE_OPACITY: f32 = 0.25;

/// Title bar height in world units. Two glyph rows tall.
const TITLE_BAR_HEIGHT: f32 = 2.0 * TEXT_SCALE;
/// Title bar height in glyph units (= 2 rows). Used for LayoutComponent styling.
const TITLE_BAR_HEIGHT_GU: f32 = 2.0;
/// Gap between title bar bottom and content top, in glyph units.
/// Applied as `margin.bottom` on `header_style`; LayoutSystem inserts this space.
const TITLE_CONTENT_GAP_GU: f32 = 0.5;
/// Title bar background: medium slate, more opaque than content bg.
const TITLE_BG_COLOR: [f32; 4] = [0.45, 0.47, 0.55, 0.95];
/// Title bar label text color: white.
const TITLE_TEXT_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
/// Gizmo visual scale for panel title-bar gizmos.
const PANEL_GIZMO_SCALE: f32 = 0.25;

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

        // --- World panel: DragMove on scroll anchor → smooth scroll ---
        rx.add_handler_closure(
            SignalKind::DragMove,
            wpa_id,
            move |world, emit, env| {
                let Some(EventSignal::DragMove { delta_world, .. }) = env.event.as_ref() else {
                    return;
                };
                // Negate dy: dragging down (negative world-Y delta) should decrease
                // scroll_offset (reveal earlier items), matching direct-manipulation feel.
                let dy = -delta_world[1];
                let (new_start, window_changed, sub_y) = {
                    let Some(sc) =
                        world.get_component_by_id_as_mut::<ScrollingComponent>(wsc_id)
                    else {
                        return;
                    };
                    let Some((s, _e, wc)) = sc.apply_drag(dy) else { return };
                    (s, wc, sc.sub_row_y_offset())
                };
                // Rebuild rows synchronously when the window changes so content and
                // anchor offset are always consistent within the same frame.
                if window_changed {
                    if let Some(wpc) =
                        world.get_component_by_id_as_mut::<WorldPanelComponent>(wpc_id)
                    {
                        wpc.scroll_offset_rows = new_start as i32;
                    }
                    let sel = world
                        .get_component_by_id_as::<WorldPanelComponent>(wpc_id)
                        .and_then(|w| {
                            let er = w.editor_root?;
                            world
                                .get_component_by_id_as::<crate::engine::ecs::component::EditorComponent>(er)
                                .and_then(|ed| ed.selected)
                        });
                    rebuild_world_panel(world, emit, wpc_id, editor_root, sel, new_start);
                }
                // rows_anchor.y = base + sub_y gives a continuous offset across window
                // boundaries: when window_start increments, panel_i decrements by 1
                // (-item_h), and sub_y resets by -item_h, so they cancel perfectly.
                let (rows_anchor, base_pos) = {
                    let Some(wpc) = world.get_component_by_id_as::<WorldPanelComponent>(wpc_id)
                    else {
                        return;
                    };
                    (wpc.rows_anchor, wpc.rows_anchor_base_pos)
                };
                if let Some(ra) = rows_anchor {
                    emit.push_intent_now(
                        ra,
                        IntentValue::UpdateTransform {
                            component_ids: vec![ra],
                            translation: [base_pos[0], base_pos[1] + sub_y, base_pos[2]],
                            rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                            scale: [1.0, 1.0, 1.0],
                        },
                    );
                }
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

        // --- Inspector panel: DragMove → smooth scroll ---
        rx.add_handler_closure(
            SignalKind::DragMove,
            ipa_id,
            move |world, emit, env| {
                let Some(EventSignal::DragMove { delta_world, .. }) = env.event.as_ref() else {
                    return;
                };
                let dy = -delta_world[1];
                let (new_start, window_changed, sub_y) = {
                    let Some(sc) =
                        world.get_component_by_id_as_mut::<ScrollingComponent>(isc_id)
                    else {
                        return;
                    };
                    let Some((s, _e, wc)) = sc.apply_drag(dy) else { return };
                    (s, wc, sc.sub_row_y_offset())
                };
                if window_changed {
                    let sel = world
                        .get_component_by_id_as::<InspectorPanelComponent>(ipc_id)
                        .and_then(|i| i.inspected);
                    rebuild_inspector_panel(world, emit, ipc_id, sel, new_start);
                }
                let (rows_anchor, base_pos) = {
                    let Some(ipc) =
                        world.get_component_by_id_as::<InspectorPanelComponent>(ipc_id)
                    else {
                        return;
                    };
                    (ipc.rows_anchor, ipc.rows_anchor_base_pos)
                };
                if let Some(ra) = rows_anchor {
                    emit.push_intent_now(
                        ra,
                        IntentValue::UpdateTransform {
                            component_ids: vec![ra],
                            translation: [base_pos[0], base_pos[1] + sub_y, base_pos[2]],
                            rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                            scale: [1.0, 1.0, 1.0],
                        },
                    );
                }
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

/// Spawn the panel root transform + title bar (background rect, label, gizmo).
///
/// Returns the `panel_transform` ComponentId. All panel content should be
/// attached as children of this node — the gizmo targets it, so dragging
/// the gizmo handles moves the entire panel.
///
/// Hierarchy produced (as children of `parent`):
/// ```text
/// panel_transform  (TransformComponent at world pos)   ← gizmo target
///   title_bar_t    (TransformComponent — sized to title bar)
///     title_bar_col + title_bar_r                      ← flat rect visual
///   title_label_t  (TransformComponent — text scale)
///     title_label_col + title_label_text               ← label
///   panel_gizmo    (TransformGizmoComponent)           ← finds panel_transform
/// ```
/// Spawn the panel root + a `LayoutComponent` + a `header_slot` TransformComponent (with
/// title-bar visuals and gizmo).
///
/// Returns `(panel_t, layout_root_id)` so the caller can attach the content slot as a second
/// flex item under the same `LayoutComponent`.
///
/// `panel_width_world` — panel content width in world units.
/// `content_height_world` — height of the scrollable content area in world units.
fn spawn_panel_title_bar(
    world: &mut World,
    parent: ComponentId,
    pos: (f32, f32, f32),
    panel_width_world: f32,
    content_height_world: f32,
    label: &str,
) -> (ComponentId, ComponentId) {
    // ── Panel root — gizmo target ────────────────────────────────────────
    // Plain position anchor (scale=1.0 → world units).  The gizmo walks up
    // ancestry to find the nearest TransformComponent and drags this node.
    let panel_t = world.add_component_boxed_named(
        "panel_transform",
        Box::new(TransformComponent::new().with_position(pos.0, pos.1, pos.2)),
    );

    // ── LayoutComponent — flex-column container ──────────────────────────
    // Two flex items: header_slot (fixed 2 gu) + content_slot (flex_grow=1).
    // unit_scale converts glyph heights to world offsets (TEXT_SCALE = 0.08).
    // header margin_box = TITLE_BAR_HEIGHT_GU + TITLE_CONTENT_GAP_GU (margin.bottom)
    // content margin_box = content_height_world / TEXT_SCALE
    let avail_height_gu = TITLE_BAR_HEIGHT_GU + TITLE_CONTENT_GAP_GU + content_height_world / TEXT_SCALE;
    let avail_width_gu = panel_width_world / TEXT_SCALE;
    let layout_root = world.add_component_boxed_named(
        "panel_layout",
        Box::new(
            LayoutComponent::new(avail_width_gu)
                .with_height(avail_height_gu)
                .with_unit_scale(TEXT_SCALE),
        ),
    );

    // ── Header slot — flex item for title bar ────────────────────────────
    // LayoutSystem will set its translation to [0, 0, 0] (top of panel).
    // Pre-set to the correct position so the first frame has no flicker.
    let header_slot = world.add_component_boxed_named(
        "header_slot",
        Box::new(TransformComponent::new()),
    );
    let header_el = world.add_component_boxed_named(
        "header_el",
        Box::new(HtmlElementComponent::header()),
    );
    let header_style = world.add_component_boxed_named(
        "header_style",
        Box::new({
            let mut s = StyleComponent::new();
            s.height = SizeDimension::GlyphUnits(TITLE_BAR_HEIGHT_GU);
            s.margin.bottom = TITLE_CONTENT_GAP_GU;
            s
        }),
    );

    // ── Title bar background rect ────────────────────────────────────────
    // Spans full panel footprint including drag margins; slightly in front.
    let bar_full_width = panel_width_world + 2.0 * DRAG_MARGIN;
    let bar_t = world.add_component_boxed_named(
        "panel_titlebar_t",
        Box::new(
            TransformComponent::new()
                .with_position(panel_width_world * 0.5, -TITLE_BAR_HEIGHT * 0.5, 0.005)
                .with_scale(bar_full_width, TITLE_BAR_HEIGHT, 1.0),
        ),
    );
    let bar_col = world.add_component_boxed_named(
        "panel_titlebar_col",
        Box::new(ColorComponent::rgba(
            TITLE_BG_COLOR[0], TITLE_BG_COLOR[1], TITLE_BG_COLOR[2], TITLE_BG_COLOR[3],
        )),
    );
    let bar_r = world.add_component_boxed_named(
        "panel_titlebar_r",
        Box::new(RenderableComponent::square()),
    );

    // ── Title label ──────────────────────────────────────────────────────
    let label_y = -(TITLE_BAR_HEIGHT - TEXT_SCALE) * 0.5;
    let label_t = world.add_component_boxed_named(
        "panel_titlebar_label_t",
        Box::new(
            TransformComponent::new()
                .with_position(0.02, label_y, 0.01)
                .with_scale(TEXT_SCALE, TEXT_SCALE, TEXT_SCALE),
        ),
    );
    let label_col = world.add_component_boxed_named(
        "panel_titlebar_label_col",
        Box::new(ColorComponent::rgba(
            TITLE_TEXT_COLOR[0], TITLE_TEXT_COLOR[1],
            TITLE_TEXT_COLOR[2], TITLE_TEXT_COLOR[3],
        )),
    );
    let label_text = world.add_component_boxed_named(
        "panel_titlebar_label",
        Box::new(crate::engine::ecs::component::TextComponent::new(label)),
    );

    // ── Gizmo (targets panel_transform via ancestry walk) ────────────────
    let gizmo = world.add_component_boxed_named(
        "panel_gizmo",
        Box::new(TransformGizmoComponent::new().with_scale(PANEL_GIZMO_SCALE)),
    );

    // ── Attach ───────────────────────────────────────────────────────────
    let _ = world.add_child(parent, panel_t);
    let _ = world.add_child(panel_t, layout_root);
    let _ = world.add_child(layout_root, header_slot);

    // HtmlElement + Style go first (LayoutSystem reads them from children).
    let _ = world.add_child(header_slot, header_el);
    let _ = world.add_child(header_slot, header_style);

    // Title bar visuals.
    let _ = world.add_child(header_slot, bar_t);
    let _ = world.add_child(bar_t, bar_col);
    let _ = world.add_child(bar_col, bar_r);

    let _ = world.add_child(header_slot, label_t);
    let _ = world.add_child(label_t, label_col);
    let _ = world.add_child(label_col, label_text);

    // Gizmo must be a direct child of panel_t so the ancestry walk finds
    // panel_t (not header_slot) as the drag target.
    let _ = world.add_child(panel_t, gizmo);

    (panel_t, layout_root)
}

/// Returns `(panel_component_id, panel_anchor_id, scroll_component_id)`.
/// Spawn an invisible drag-capture quad in front of a panel.
///
/// `panel_width` and `panel_height` are in world/overlay units. The quad is
/// attached as a child of `parent` (the panel's OverlayComponent node) at
/// `pos` + a small forward Z offset so it sits in front of the row content.
///
/// `RaycastableComponent::drag_only()` means drags land here while clicks
/// pass through to the row items behind it.
///
/// Returns the drag plane TransformComponent id.
fn spawn_drag_plane(
    world: &mut World,
    parent: ComponentId,
    pos: (f32, f32, f32),
    panel_width: f32,
    panel_height: f32,
) -> ComponentId {
    let w = panel_width + 2.0 * DRAG_MARGIN;
    let h = panel_height + 2.0 * DRAG_MARGIN;
    let cx = pos.0 + panel_width * 0.5;
    let cy = pos.1 - panel_height * 0.5;
    let cz = pos.2 + DRAG_PLANE_Z_OFFSET;

    let dp_t = world.add_component_boxed_named(
        "drag_plane_t",
        Box::new(
            TransformComponent::new()
                .with_position(cx, cy, cz)
                .with_scale(w, h, 1.0),
        ),
    );
    let dp_col = world.add_component_boxed_named(
        "drag_plane_col",
        Box::new(ColorComponent::rgba(
            DRAG_PLANE_COLOR[0],
            DRAG_PLANE_COLOR[1],
            DRAG_PLANE_COLOR[2],
            DRAG_PLANE_COLOR[3],
        )),
    );
    let dp_r = world.add_component_boxed_named(
        "drag_plane_r",
        Box::new(RenderableComponent::square()),
    );
    let dp_opacity = world.add_component_boxed_named(
        "drag_plane_opacity",
        Box::new(OpacityComponent { opacity: DRAG_PLANE_OPACITY, multiple_layers: false }),
    );
    let dp_rc = world.add_component_boxed_named(
        "drag_plane_rc",
        Box::new(RaycastableComponent::drag_only()),
    );
    let dp_shape = world.add_component_boxed_named(
        "drag_plane_shape",
        Box::new(RaycastableShapeComponent::new(RaycastableShapeType::Quad2D)),
    );

    let _ = world.add_child(parent, dp_t);
    let _ = world.add_child(dp_t, dp_col);
    let _ = world.add_child(dp_col, dp_r);
    let _ = world.add_child(dp_r, dp_opacity);
    let _ = world.add_child(dp_r, dp_rc);
    let _ = world.add_child(dp_r, dp_shape);

    dp_t
}

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
    // rows_anchor is at local [0, 0, 0] within the content slot.
    // The LayoutSystem positions the content slot below the title bar,
    // so rows_anchor_base_pos is the zero vector.
    let wpr = world.add_component_boxed_named(
        "world_panel_rows",
        Box::new(TransformComponent::new()),
    );

    let wp_width = LayoutSystem::estimate_panel_width(
        crate::engine::ecs::component::TextComponent::DEFAULT_WRAP_AT,
        TEXT_SCALE,
        MAX_DEPTH as f32 * INDENT_UNIT,
    );
    let wp_height = PAGE_SIZE as f32 * ROW_HEIGHT;

    let _ = world.add_child(wpa, wpo);

    // Panel root + LayoutComponent + header slot (with title bar visuals + gizmo).
    let (wp_t, layout_root) =
        spawn_panel_title_bar(world, wpo, pos, wp_width, wp_height, "World");

    // ── Content slot — second flex item (flex_grow=1) ────────────────────
    // LayoutSystem will position this at [0, -TITLE_BAR_HEIGHT, 0].
    // Pre-set the initial position to avoid a one-frame flicker before
    // LayoutSystem first runs.
    let content_slot = world.add_component_boxed_named(
        "content_slot",
        Box::new(TransformComponent::new().with_position(0.0, -TITLE_BAR_HEIGHT, 0.0)),
    );
    // Style alone is sufficient — block is the layout default when no display is set.
    let content_style = world.add_component_boxed_named(
        "content_style",
        Box::new(StyleComponent::new()), // height: Auto fills remaining container space
    );

    let _ = world.add_child(layout_root, content_slot);
    let _ = world.add_child(content_slot, content_style);

    // Drag plane covers the content area; parent is content_slot so its
    // local [0, 0] aligns with the top of the content region.
    spawn_drag_plane(world, content_slot, (0.0, 0.0, 0.0), wp_width, wp_height);

    let _ = world.add_child(content_slot, wpc);
    let _ = world.add_child(wpc, wsc);
    let _ = world.add_child(wsc, wpr);

    // ── Row layout root ──────────────────────────────────────────────────
    // LayoutSystem positions row TCs within this layout context.
    // available_height is None → rows stack without a height constraint.
    let wpr_layout = world.add_component_boxed_named(
        "world_panel_rows_layout",
        Box::new(
            LayoutComponent::new(wp_width / TEXT_SCALE)
                .with_unit_scale(TEXT_SCALE),
        ),
    );
    let _ = world.add_child(wpr, wpr_layout);

    let _ = wp_t; // panel_t used only for gizmo ancestry

    if let Some(c) = world.get_component_by_id_as_mut::<WorldPanelComponent>(wpc) {
        c.editor_root = Some(editor_root);
        c.rows_anchor = Some(wpr);
        c.rows_layout = Some(wpr_layout);
        // rows_anchor is at [0,0,0] relative to content_slot.
        // LayoutSystem handles the title-bar offset by positioning content_slot.
        c.rows_anchor_base_pos = [0.0, 0.0, 0.0];
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
        Box::new(TransformComponent::new()),
    );

    let ip_width = LayoutSystem::estimate_panel_width(
        crate::engine::ecs::component::TextComponent::DEFAULT_WRAP_AT,
        TEXT_SCALE,
        0.0,
    );
    let ip_height = PAGE_SIZE as f32 * ROW_HEIGHT;

    let _ = world.add_child(ipa, ipo);

    let (ip_t, layout_root) =
        spawn_panel_title_bar(world, ipo, pos, ip_width, ip_height, "Inspector");

    let content_slot = world.add_component_boxed_named(
        "content_slot",
        Box::new(TransformComponent::new().with_position(0.0, -TITLE_BAR_HEIGHT, 0.0)),
    );
    let content_style = world.add_component_boxed_named(
        "content_style",
        Box::new(StyleComponent::new()), // height: Auto fills remaining container space
    );

    let _ = world.add_child(layout_root, content_slot);
    let _ = world.add_child(content_slot, content_style);
    spawn_drag_plane(world, content_slot, (0.0, 0.0, 0.0), ip_width, ip_height);
    let _ = world.add_child(content_slot, ipc);
    let _ = world.add_child(ipc, isc);
    let _ = world.add_child(isc, ipr);

    let ipr_layout = world.add_component_boxed_named(
        "inspector_panel_rows_layout",
        Box::new(
            LayoutComponent::new(ip_width / TEXT_SCALE)
                .with_unit_scale(TEXT_SCALE),
        ),
    );
    let _ = world.add_child(ipr, ipr_layout);

    let _ = ip_t;

    if let Some(c) = world.get_component_by_id_as_mut::<InspectorPanelComponent>(ipc) {
        c.editor_root = Some(editor_root);
        c.rows_anchor = Some(ipr);
        c.rows_layout = Some(ipr_layout);
        c.rows_anchor_base_pos = [0.0, 0.0, 0.0];
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
    let (rows_anchor, rows_layout_id) = {
        let Some(wpc) = world.get_component_by_id_as::<WorldPanelComponent>(wpc_id) else {
            return;
        };
        (wpc.rows_anchor, wpc.rows_layout)
    };
    let Some(rows_anchor) = rows_anchor else { return };
    let Some(rows_layout_id) = rows_layout_id else { return };

    // Clear current row children from the layout root.
    let old_children: Vec<ComponentId> = world.children_of(rows_layout_id).to_vec();
    for old in &old_children {
        world.detach_from_parent(*old);
        emit.push_intent_now(
            rows_layout_id,
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

        // LayoutSystem drives the y-position; only scale is set here.
        let row_t = world.add_component_boxed_named(
            format!("wp_row_{panel_i}"),
            Box::new(TransformComponent::new().with_scale(TEXT_SCALE, TEXT_SCALE, TEXT_SCALE)),
        );
        let _ = world.add_child(rows_layout_id, row_t);

        let row_style = world.add_component_boxed_named(
            "wp_row_style",
            Box::new({
                let mut s = StyleComponent::new();
                s.height = SizeDimension::Auto;
                s.margin = EdgeInsets {
                    left: *depth as f32 * INDENT_UNIT_GU,
                    ..EdgeInsets::ZERO
                };
                s
            }),
        );
        let _ = world.add_child(row_t, row_style);

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
            .add_component_boxed_named("wp_rc", Box::new(RaycastableComponent::click_only()));
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

    // Mark the row layout dirty so LayoutSystem repositions rows next tick.
    if let Some(lc) = world.get_component_by_id_as_mut::<LayoutComponent>(rows_layout_id) {
        lc.dirty = true;
    }

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
    let (rows_anchor, rows_layout_id) = {
        let Some(ipc) = world.get_component_by_id_as::<InspectorPanelComponent>(ipc_id) else {
            return;
        };
        (ipc.rows_anchor, ipc.rows_layout)
    };
    let Some(rows_anchor) = rows_anchor else { return };
    let Some(rows_layout_id) = rows_layout_id else { return };

    let old_children: Vec<ComponentId> = world.children_of(rows_layout_id).to_vec();
    for old in &old_children {
        world.detach_from_parent(*old);
        emit.push_intent_now(
            rows_layout_id,
            IntentValue::RemoveSubtree { component_ids: vec![*old] },
        );
    }

    let lines: Vec<String> = if let Some(sel) = selected {
        if let Some(node) = world.get_component_node(sel) {
            let header = mms_node_label(node);
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
            Box::new(TransformComponent::new().with_scale(TEXT_SCALE, TEXT_SCALE, TEXT_SCALE)),
        );
        let _ = world.add_child(rows_layout_id, row_t);

        let row_style = world.add_component_boxed_named(
            "ip_row_style",
            Box::new({
                let mut s = StyleComponent::new();
                s.height = SizeDimension::Auto;
                s
            }),
        );
        let _ = world.add_child(row_t, row_style);

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

    if let Some(lc) = world.get_component_by_id_as_mut::<LayoutComponent>(rows_layout_id) {
        lc.dirty = true;
    }

    if let Some(ipc) = world.get_component_by_id_as_mut::<InspectorPanelComponent>(ipc_id) {
        ipc.row_roots = new_rows;
        ipc.inspected = selected;
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn panel_row_bg(i: usize, total: usize) -> TextBackgroundComponent {
    // Row 0 no longer needs PANEL_V_PADDING at the top: the title bar provides
    // the visual top boundary. Restoring the padding would push the background
    // into the title bar area and cause visible overlap.
    let _ = i;
    TextBackgroundComponent::new()
        .with_padding_top(0.0)
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
    world.get_component_node(id)
        .map(mms_node_label)
        .unwrap_or_else(|| format!("{id:?}"))
}

/// MMS-syntax display for a component node in the inspector tree.
/// `Transform {}` when unlabeled, `Transform { name="catgirl" }` when labeled.
fn mms_node_label(node: &crate::engine::ecs::component::ComponentNode) -> String {
    // Capitalize first letter of component_type for MMS convention.
    let type_name = capitalize_first(&node.component_type);
    let mut attrs = String::new();
    if !node.name.is_empty() {
        attrs.push_str(&format!("name=\"{}\"", node.name));
    }
    if !node.classes.is_empty() {
        if !attrs.is_empty() { attrs.push(' '); }
        attrs.push_str(&format!("class=\"{}\"", node.classes.join(" ")));
    }
    if attrs.is_empty() {
        format!("{type_name} {{}}")
    } else {
        format!("{type_name} {{ {attrs} }}")
    }
}

fn capitalize_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
