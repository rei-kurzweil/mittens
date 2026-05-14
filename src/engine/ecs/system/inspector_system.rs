use crate::engine::ecs::component::{
    ColorComponent, EmissiveComponent, HtmlElementComponent, InspectorPanelComponent,
    LayoutComponent, OverlayComponent, RaycastableComponent, RenderableComponent,
    ScrollingComponent,
    SelectableComponent, StyleComponent, TransformComponent,
    TransformGizmoComponent, WorldPanelComponent,
    style::{EdgeInsets, Overflow, SizeDimension},
};
use crate::engine::ecs::system::editor_system::select_editor_target;
use crate::engine::ecs::system::{LayoutSystem, ScrollingSystem};
use crate::engine::ecs::rx::RxWorld;
use crate::engine::ecs::{
    ComponentId, EventSignal, IntentValue, SignalEmitter, SignalKind, World,
};

const ROW_HEIGHT: f32 = 0.090;
const TEXT_SCALE: f32 = 0.08;
const INDENT_UNIT: f32 = 0.12;
/// Indent per depth level in glyph units (= INDENT_UNIT / TEXT_SCALE).
const INDENT_UNIT_GU: f32 = INDENT_UNIT / TEXT_SCALE;
const PAGE_SIZE: usize = 48;
const MAX_DEPTH: usize = 5;
/// Gap between world panel right edge and inspector panel left edge (overlay units).
const PANEL_GAP: f32 = 0.12;

/// Title bar height in world units. Two glyph rows tall.
const TITLE_BAR_HEIGHT: f32 = 2.0 * TEXT_SCALE;
/// Title bar height in glyph units (= 2 rows). Used for LayoutComponent styling.
const TITLE_BAR_HEIGHT_GU: f32 = 2.0;
/// Gap between title bar bottom and content top, in glyph units.
/// Applied as `margin.bottom` on `header_style`; LayoutSystem inserts this space.
const TITLE_CONTENT_GAP_GU: f32 = 0.5;
/// Debug title bar background: green.
const TITLE_BG_COLOR: [f32; 4] = [0.18, 0.78, 0.22, 0.95];
/// Title bar label text color: white.
const TITLE_TEXT_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
/// Gizmo visual scale for panel title-bar gizmos.
const PANEL_GIZMO_SCALE: f32 = 0.25;

/// Debug scroll viewport background: yellow.
const SCROLL_BG_COLOR: [f32; 4] = [0.96, 0.92, 0.18, 0.80];
/// Row background color: light grey, semi-transparent.
const ROW_BG_COLOR: [f32; 4] = [0.92, 0.92, 0.92, 0.80];
/// Normal text color: black.
const TEXT_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
/// Highlighted row text color: dark green.
const HIGHLIGHT_COLOR: [f32; 4] = [0.0, 0.45, 0.0, 1.0];

#[derive(Debug, Default)]
pub struct InspectorSystem {
    /// Cached id of the singleton `LayoutComponent` that hosts every editor's panels.
    /// All editors share one layout root so panels float beside each other instead of
    /// overlapping when multiple `EditorComponent`s exist.
    editor_layout_root: Option<ComponentId>,
}

impl InspectorSystem {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the world-singleton `LayoutComponent` id used to lay out every editor's
    /// panels side-by-side. Spawns it on first call (and again if it was removed
    /// from the world since last call).
    ///
    /// Hierarchy spawned (no parent — top-level node):
    /// ```text
    /// editor_layout_anchor (TransformComponent at default panel position, Selectable::off)
    ///   └── editor_layout_overlay (OverlayComponent)        ← all panels render in overlay phase
    ///       └── editor_layout_root (LayoutComponent)        ← panels attach here
    /// ```
    ///
    /// The returned id is the `LayoutComponent` node — panels (each authored with
    /// `display: inline-block`) become its children.
    pub fn ensure_editor_layout_root(
        &mut self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
    ) -> ComponentId {
        // Validate cached id: the layout component must still exist in the world.
        if let Some(id) = self.editor_layout_root {
            if world.get_component_by_id_as::<LayoutComponent>(id).is_some() {
                return id;
            }
            self.editor_layout_root = None;
        }

        // Default anchor position matches `EditorComponent::world_panel_pos` so the
        // shared layout sits where panels used to spawn pre-merge.
        const ANCHOR_POS: (f32, f32, f32) = (-0.7, 1.6, -1.2);
        // Width budget for all panels combined, in glyph units.  Enough for ~3 panels at
        // `TextComponent::DEFAULT_WRAP_AT`. Panels overflowing wrap to the next row via
        // inline-block flow.
        const LAYOUT_WIDTH_GU: f32 = 3.0
            * (crate::engine::ecs::component::TextComponent::DEFAULT_WRAP_AT as f32)
            + 6.0;

        // SelectableComponent::off sits at the top of the chain so every descendant
        // (panels, their content, etc.) is non-selectable as a region. Per-panel
        // gizmos still work because they walk to the nearest TransformComponent
        // independently of selection routing.
        let selectable_off = world.add_component_boxed_named(
            "editor_layout_selectable_off",
            Box::new(SelectableComponent::off()),
        );
        let anchor = world.add_component_boxed_named(
            "editor_layout_anchor",
            Box::new(
                TransformComponent::new()
                    .with_position(ANCHOR_POS.0, ANCHOR_POS.1, ANCHOR_POS.2),
            ),
        );
        let _ = world.add_child(selectable_off, anchor);

        let overlay = world.add_component_boxed_named(
            "editor_layout_overlay",
            Box::new(OverlayComponent::new()),
        );
        let _ = world.add_child(anchor, overlay);

        let layout_root = world.add_component_boxed_named(
            "editor_layout_root",
            Box::new(
                LayoutComponent::new(LAYOUT_WIDTH_GU).with_unit_scale(TEXT_SCALE),
            ),
        );
        let _ = world.add_child(overlay, layout_root);

        world.init_component_tree(selectable_off, emit);

        self.editor_layout_root = Some(layout_root);
        layout_root
    }

    pub fn setup_panels_for_editor(
        &mut self,
        rx: &mut RxWorld,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        editor_root: ComponentId,
        _world_panel_pos: (f32, f32, f32),
        _inspector_panel_pos: (f32, f32, f32),
    ) {
        // Per-editor panel positions are no longer honored — every editor's panels
        // attach to the world-singleton layout root and flow side-by-side.
        let layout_root = self.ensure_editor_layout_root(world, emit);

        let (wpc_id, wpa_id, wsc_id) = spawn_world_panel(world, emit, editor_root, layout_root);
        let (ipc_id, _ipa_id, isc_id) = spawn_inspector_panel(world, emit, editor_root, layout_root);

        rebuild_world_panel(world, emit, wpc_id, wsc_id, editor_root, None);
        rebuild_inspector_panel(world, emit, ipc_id, isc_id, None);

        // --- World panel: Click on a row → select that node ---
        // Capture the top-level UI root of this editor so the Save handler
        // can exclude it from the dumped scene. `layout_root` is a descendant
        // of `editor_layout_anchor` (the topmost ancestor with no parent).
        let editor_ui_root = {
            let mut top = layout_root;
            while let Some(p) = world.parent_of(top) {
                top = p;
            }
            top
        };

        rx.add_handler_closure(
            SignalKind::Click,
            wpa_id,
            move |world, emit, env| {
                let Some(EventSignal::Click { renderable, .. }) = env.event.as_ref() else {
                    return;
                };
                let renderable = *renderable;

                let (row_roots, row_to_node, save_btn, load_btn, status_text, filename) = {
                    let Some(wpc) = world.get_component_by_id_as::<WorldPanelComponent>(wpc_id)
                    else {
                        return;
                    };
                    (
                        wpc.row_roots.clone(),
                        wpc.row_to_node.clone(),
                        wpc.save_button_renderable,
                        wpc.load_button_renderable,
                        wpc.save_status_text,
                        wpc.save_filename.clone(),
                    )
                };

                // --- Save button hit: dump scene → MMS file → update status ---
                if Some(renderable) == save_btn {
                    let mms = dump_scene_to_mms(world, Some(editor_ui_root));
                    let msg = match std::fs::write(&filename, &mms) {
                        Ok(()) => format!("saved: {filename}"),
                        Err(e) => format!("save failed: {e}"),
                    };
                    println!("[WorldPanel] {msg}");
                    if let Some(text_id) = status_text {
                        emit.push_intent_now(
                            text_id,
                            IntentValue::SetText {
                                component_ids: vec![text_id],
                                text: msg,
                            },
                        );
                    }
                    return;
                }

                // --- Load button hit: stub for now ---
                // Runtime reload requires clearing scene roots first; see
                // docs/task/mms-asset-component-panels.md.
                if Some(renderable) == load_btn {
                    let msg = format!("load not yet wired ({filename})");
                    println!("[WorldPanel] {msg}");
                    if let Some(text_id) = status_text {
                        emit.push_intent_now(
                            text_id,
                            IntentValue::SetText {
                                component_ids: vec![text_id],
                                text: msg,
                            },
                        );
                    }
                    return;
                }

                // --- Otherwise: row click → select that node ---
                let Some(panel_idx) = find_ancestor_in_list(world, renderable, &row_roots) else {
                    return;
                };
                let Some(&node_id) = row_to_node.get(panel_idx) else {
                    return;
                };
                select_editor_target(world, emit, editor_root, node_id, false);
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

                if let Some(sc) = world.get_component_by_id_as_mut::<ScrollingComponent>(wsc_id) {
                    sc.scroll_offset = 0.0;
                };
                if let Some(sc) = world.get_component_by_id_as_mut::<ScrollingComponent>(isc_id) {
                    sc.scroll_offset = 0.0;
                }

                rebuild_world_panel(world, emit, wpc_id, wsc_id, editor_root, selected);
                rebuild_inspector_panel(world, emit, ipc_id, isc_id, selected);
            },
        );
    }
}

// ---------------------------------------------------------------------------
// Panel spawn helpers
// ---------------------------------------------------------------------------

/// Spawn the panel root transform + styled title bar + gizmo.
///
/// Returns the `panel_transform` ComponentId. All panel content should be
/// attached as children of this node — the gizmo targets it, so dragging
/// the gizmo handles moves the entire panel.
///
/// Hierarchy produced (as children of `parent`):
/// ```text
/// panel_transform  (TransformComponent at world pos)   ← gizmo target
///   header_slot    (TransformComponent + StyleComponent background)
///   title_label_t  (TransformComponent — text scale)
///     title_label_col + title_label_text               ← label
///   panel_gizmo    (TransformGizmoComponent)           ← finds panel_transform
/// ```
/// Spawn the panel root + a `LayoutComponent` + a styled `header_slot`.
///
/// Returns `(panel_t, layout_root_id)` so the caller can attach the content slot as a second
/// flex item under the same `LayoutComponent`.
///
/// `panel_width_world` — panel content width in world units.
/// `content_height_world` — height of the scrollable content area in world units.
/// Optional extras spawned by `spawn_panel_title_bar` when `show_buttons` is true.
/// World panel gets these; inspector panel doesn't.
#[derive(Debug, Default, Clone, Copy)]
struct TitleBarButtons {
    /// Renderable id of the Save button background quad. Click events report this.
    save_renderable: ComponentId,
    /// Renderable id of the Load button background quad.
    load_renderable: ComponentId,
    /// TextComponent id of the "saved <filename>" label above the panel.
    save_status_text: ComponentId,
}

fn spawn_panel_title_bar(
    world: &mut World,
    parent: ComponentId,
    panel_width_world: f32,
    content_height_world: f32,
    label: &str,
    show_buttons: bool,
) -> (ComponentId, ComponentId, Option<TitleBarButtons>) {
    // ── Panel root — gizmo target + inline-block flow item ────────────────
    // Position is layout-driven by the singleton editor layout root.  The
    // gizmo walks up ancestry to find the nearest TransformComponent and
    // drags this node — that override will eventually need to compose with
    // the layout-driven position via signal routing, but for now flow wins.
    let panel_t = world.add_component_boxed_named(
        "panel_transform",
        Box::new(TransformComponent::new()),
    );

    // ── Style: inline-block + explicit width/height ──────────────────────
    // Required so the singleton layout root flows panels horizontally with
    // wrap. Width/height are in glyph units (the parent layout has
    // unit_scale = TEXT_SCALE).
    let total_height_gu = TITLE_BAR_HEIGHT_GU + TITLE_CONTENT_GAP_GU + content_height_world / TEXT_SCALE;
    let panel_width_gu = panel_width_world / TEXT_SCALE;
    let panel_outer_style = world.add_component_boxed_named(
        "panel_outer_style",
        Box::new({
            let mut s = StyleComponent::new();
            s.display = Some(crate::engine::ecs::component::style::Display::InlineBlock);
            s.width = SizeDimension::GlyphUnits(panel_width_gu);
            s.height = SizeDimension::GlyphUnits(total_height_gu);
            s.margin = EdgeInsets {
                top: SizeDimension::GlyphUnits(0.5),
                right: SizeDimension::GlyphUnits(1.0),
                bottom: SizeDimension::GlyphUnits(0.5),
                left: SizeDimension::GlyphUnits(0.0),
            };
            s
        }),
    );
    let _ = world.add_child(panel_t, panel_outer_style);

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
            s.margin.bottom = SizeDimension::GlyphUnits(TITLE_CONTENT_GAP_GU);
            s.background_color = Some(TITLE_BG_COLOR);
            s
        }),
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

    let _ = world.add_child(header_slot, label_t);
    let _ = world.add_child(label_t, label_col);
    let _ = world.add_child(label_col, label_text);

    // Gizmo must be a direct child of panel_t so the ancestry walk finds
    // panel_t (not header_slot) as the drag target.
    let _ = world.add_child(panel_t, gizmo);

    let buttons = if show_buttons {
        // Save / Load buttons — pinned to the right of the title bar.
        // Pure-rust mirror of `assets/components/button.mms` styling
        // (rgb 0.30/0.45/0.90 background, white centered label). Switching
        // to importing button.mms directly needs a host API to call MMS
        // factory functions from rust — see
        // docs/task/mms-asset-component-panels.md.
        let load = spawn_titlebar_button(
            world,
            header_slot,
            "Load",
            /* right_edge_x */ panel_width_world - 0.05,
        );
        let save = spawn_titlebar_button(
            world,
            header_slot,
            "Save",
            /* right_edge_x */ panel_width_world - 0.05 - TITLEBAR_BUTTON_WIDTH - 0.05,
        );

        // "saved: <filename>" label above the panel. Default text empty;
        // the click handler swaps it in via SetText.
        let status_t = world.add_component_boxed_named(
            "save_status_t",
            Box::new(
                TransformComponent::new()
                    .with_position(0.02, TEXT_SCALE * 0.6, 0.01)
                    .with_scale(TEXT_SCALE, TEXT_SCALE, TEXT_SCALE),
            ),
        );
        let status_col = world.add_component_boxed_named(
            "save_status_col",
            Box::new(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0)),
        );
        let status_text = world.add_component_boxed_named(
            "save_status_text",
            Box::new(crate::engine::ecs::component::TextComponent::new("")),
        );
        let _ = world.add_child(panel_t, status_t);
        let _ = world.add_child(status_t, status_col);
        let _ = world.add_child(status_col, status_text);

        Some(TitleBarButtons {
            save_renderable: save,
            load_renderable: load,
            save_status_text: status_text,
        })
    } else {
        None
    };

    (panel_t, layout_root, buttons)
}

const TITLEBAR_BUTTON_WIDTH: f32 = 0.55;

/// Default filename used by the World panel's Save button — derived from
/// the running binary's file stem so each example saves to its own `.mms`
/// next to the working directory. Falls back to `scene.mms`.
fn default_save_filename() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .map(|stem| format!("{stem}.mms"))
        .unwrap_or_else(|| "scene.mms".to_string())
}

/// Walk every scene root (anything outside the editor's own UI subtree)
/// and emit one MMS component expression per root, separated by blank lines.
///
/// Skips:
/// - The editor's own panels (anything reachable from `editor_layout_*`).
/// - The editor root itself.
fn dump_scene_to_mms(world: &World, editor_layout_root: Option<ComponentId>) -> String {
    let editor_subtree: std::collections::HashSet<ComponentId> = match editor_layout_root {
        Some(root) => collect_subtree(world, root),
        None => std::collections::HashSet::new(),
    };
    let mut out = String::new();
    for cid in world
        .all_components()
        .filter(|&cid| world.parent_of(cid).is_none())
        .filter(|cid| !editor_subtree.contains(cid))
    {
        match crate::meow_meow::component_registry::subtree_to_ce_ast(world, cid) {
            Ok(ce) => {
                out.push_str(&crate::meow_meow::unparser::unparse_component(&ce));
                out.push_str("\n\n");
            }
            Err(e) => eprintln!("[save] subtree_to_ce_ast failed for {cid:?}: {e}"),
        }
    }
    out
}

fn collect_subtree(
    world: &World,
    root: ComponentId,
) -> std::collections::HashSet<ComponentId> {
    let mut out = std::collections::HashSet::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if !out.insert(id) {
            continue;
        }
        for c in world.children_of(id) {
            stack.push(*c);
        }
    }
    out
}

/// Spawn a single right-aligned button under `parent` (the panel header_slot).
/// Returns the renderable id — the click surface that fires `EventSignal::Click`.
fn spawn_titlebar_button(
    world: &mut World,
    parent: ComponentId,
    label: &str,
    right_edge_x: f32,
) -> ComponentId {
    let btn_h = TITLE_BAR_HEIGHT * 0.78;
    let btn_w = TITLEBAR_BUTTON_WIDTH;
    let btn_center_x = right_edge_x - btn_w * 0.5;
    let btn_center_y = -TITLE_BAR_HEIGHT * 0.5;

    // Button root transform (positions the button group inside the title bar).
    let btn_t = world.add_component_boxed_named(
        "titlebar_btn_t",
        Box::new(TransformComponent::new().with_position(btn_center_x, btn_center_y, 0.02)),
    );

    // Background quad — scaled to button size; carries the renderable that
    // the raycast layer hits.
    let bg_t = world.add_component_boxed_named(
        "titlebar_btn_bg_t",
        Box::new(TransformComponent::new().with_scale(btn_w, btn_h, 1.0)),
    );
    let bg_renderable = world.add_component_boxed_named(
        "titlebar_btn_bg",
        Box::new(RenderableComponent::square()),
    );
    let bg_color = world.add_component_boxed_named(
        "titlebar_btn_bg_col",
        Box::new(ColorComponent::rgba(0.30, 0.45, 0.90, 1.0)),
    );
    let bg_raycast = world.add_component_boxed_named(
        "titlebar_btn_rc",
        Box::new(RaycastableComponent::click_only()),
    );

    // Centered label.
    let label_width_world = label.chars().count() as f32 * TEXT_SCALE * 0.55;
    let text_t = world.add_component_boxed_named(
        "titlebar_btn_text_t",
        Box::new(
            TransformComponent::new()
                .with_position(-label_width_world * 0.5, -TEXT_SCALE * 0.5, 0.01)
                .with_scale(TEXT_SCALE, TEXT_SCALE, TEXT_SCALE),
        ),
    );
    let text_col = world.add_component_boxed_named(
        "titlebar_btn_text_col",
        Box::new(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0)),
    );
    let text = world.add_component_boxed_named(
        "titlebar_btn_text",
        Box::new(crate::engine::ecs::component::TextComponent::new(label)),
    );

    let _ = world.add_child(parent, btn_t);
    let _ = world.add_child(btn_t, bg_t);
    let _ = world.add_child(bg_t, bg_renderable);
    let _ = world.add_child(bg_renderable, bg_color);
    let _ = world.add_child(bg_renderable, bg_raycast);
    let _ = world.add_child(btn_t, text_t);
    let _ = world.add_child(text_t, text_col);
    let _ = world.add_child(text_col, text);

    bg_renderable
}

fn spawn_world_panel(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    editor_root: ComponentId,
    editor_layout_root: ComponentId,
) -> (ComponentId, ComponentId, ComponentId) {
    let wp_width = LayoutSystem::estimate_panel_width(
        crate::engine::ecs::component::TextComponent::DEFAULT_WRAP_AT,
        TEXT_SCALE,
        MAX_DEPTH as f32 * INDENT_UNIT,
    );
    let wp_height = PAGE_SIZE as f32 * ROW_HEIGHT;

    let wpc = world.add_component_boxed_named(
        "world_panel",
        Box::new(WorldPanelComponent::new()),
    );
    let wsc = world.add_component_boxed_named(
        "world_panel_scroll",
        Box::new(ScrollingComponent::new(wp_height, wp_height)),
    );
    let wpr = world.add_component_boxed_named(
        "world_panel_rows_track",
        Box::new(TransformComponent::new()),
    );

    // Panel root + LayoutComponent + header slot (with title bar visuals + gizmo).
    // Parent is the singleton editor layout root; the inline-block style on
    // panel_t makes the layout system flow it beside sibling panels.
    let (wp_t, layout_root, buttons) = spawn_panel_title_bar(
        world,
        editor_layout_root,
        wp_width,
        wp_height,
        "World",
        /* show_buttons */ true,
    );

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
        Box::new({
            let mut s = StyleComponent::new();
            s.height = SizeDimension::GlyphUnits(wp_height / TEXT_SCALE);
            s.overflow = Overflow::Scroll;
            s.background_color = Some(SCROLL_BG_COLOR);
            s
        }),
    );

    let _ = world.add_child(layout_root, content_slot);
    let _ = world.add_child(content_slot, content_style);

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
        c.rows_track = Some(wpr);
        c.rows_layout = Some(wpr_layout);
        if let Some(b) = buttons {
            c.save_button_renderable = Some(b.save_renderable);
            c.load_button_renderable = Some(b.load_renderable);
            c.save_status_text = Some(b.save_status_text);
        }
        c.save_filename = default_save_filename();
    }
    if let Some(sc) = world.get_component_by_id_as_mut::<ScrollingComponent>(wsc) {
        sc.set_track(wpr, [0.0, 0.0, 0.0]);
    }

    world.init_component_tree(wp_t, emit);
    ScrollingSystem::sync_component(world, emit, wsc);
    (wpc, wp_t, wsc)
}

/// Returns `(panel_component_id, panel_anchor_id, scroll_component_id)`.
fn spawn_inspector_panel(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    editor_root: ComponentId,
    editor_layout_root: ComponentId,
) -> (ComponentId, ComponentId, ComponentId) {
    let ip_width = LayoutSystem::estimate_panel_width(
        crate::engine::ecs::component::TextComponent::DEFAULT_WRAP_AT,
        TEXT_SCALE,
        0.0,
    );
    let ip_height = PAGE_SIZE as f32 * ROW_HEIGHT;

    let ipc = world.add_component_boxed_named(
        "inspector_panel",
        Box::new(InspectorPanelComponent::new()),
    );
    let isc = world.add_component_boxed_named(
        "inspector_panel_scroll",
        Box::new(ScrollingComponent::new(ip_height, ip_height)),
    );
    let ipr = world.add_component_boxed_named(
        "inspector_panel_rows_track",
        Box::new(TransformComponent::new()),
    );

    let (ip_t, layout_root, _) = spawn_panel_title_bar(
        world,
        editor_layout_root,
        ip_width,
        ip_height,
        "Inspector",
        /* show_buttons */ false,
    );

    let content_slot = world.add_component_boxed_named(
        "content_slot",
        Box::new(TransformComponent::new().with_position(0.0, -TITLE_BAR_HEIGHT, 0.0)),
    );
    let content_style = world.add_component_boxed_named(
        "content_style",
        Box::new({
            let mut s = StyleComponent::new();
            s.height = SizeDimension::GlyphUnits(ip_height / TEXT_SCALE);
            s.overflow = Overflow::Scroll;
            s.background_color = Some(SCROLL_BG_COLOR);
            s
        }),
    );

    let _ = world.add_child(layout_root, content_slot);
    let _ = world.add_child(content_slot, content_style);
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

    if let Some(c) = world.get_component_by_id_as_mut::<InspectorPanelComponent>(ipc) {
        c.editor_root = Some(editor_root);
        c.rows_track = Some(ipr);
        c.rows_layout = Some(ipr_layout);
    }
    if let Some(sc) = world.get_component_by_id_as_mut::<ScrollingComponent>(isc) {
        sc.set_track(ipr, [0.0, 0.0, 0.0]);
    }

    world.init_component_tree(ip_t, emit);
    ScrollingSystem::sync_component(world, emit, isc);
    (ipc, ip_t, isc)
}

// ---------------------------------------------------------------------------
// Panel rebuild helpers
// ---------------------------------------------------------------------------

fn rebuild_world_panel(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    wpc_id: ComponentId,
    wsc_id: ComponentId,
    editor_root: ComponentId,
    selected: Option<ComponentId>,
) {
    let (rows_track, rows_layout_id) = {
        let Some(wpc) = world.get_component_by_id_as::<WorldPanelComponent>(wpc_id) else {
            return;
        };
        (wpc.rows_track, wpc.rows_layout)
    };
    let Some(rows_track) = rows_track else { return };
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
    let highlighted = find_highlighted(selected, &nodes, world);
    let mut new_rows = Vec::new();
    let mut new_row_to_node = Vec::new();

    for (panel_i, (node_id, depth, label)) in nodes.iter().enumerate() {
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
                    left: SizeDimension::GlyphUnits(*depth as f32 * INDENT_UNIT_GU),
                    ..EdgeInsets::ZERO
                };
                s.background_color = Some(ROW_BG_COLOR);
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

        let rc = world
            .add_component_boxed_named("wp_rc", Box::new(RaycastableComponent::click_only()));
        let _ = world.add_child(row_text, rc);

        new_rows.push(row_t);
        new_row_to_node.push(*node_id);
    }

    world.init_component_tree(rows_track, emit);

    // Mark the row layout dirty so LayoutSystem repositions rows next tick.
    if let Some(lc) = world.get_component_by_id_as_mut::<LayoutComponent>(rows_layout_id) {
        lc.dirty = true;
    }

    if let Some(wpc) = world.get_component_by_id_as_mut::<WorldPanelComponent>(wpc_id) {
        wpc.row_roots = new_rows;
        wpc.row_to_node = new_row_to_node;
    }

    ScrollingSystem::set_content_height(world, emit, wsc_id, nodes.len() as f32 * ROW_HEIGHT);
}

fn rebuild_inspector_panel(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    ipc_id: ComponentId,
    isc_id: ComponentId,
    selected: Option<ComponentId>,
) {
    let (rows_track, rows_layout_id) = {
        let Some(ipc) = world.get_component_by_id_as::<InspectorPanelComponent>(ipc_id) else {
            return;
        };
        (ipc.rows_track, ipc.rows_layout)
    };
    let Some(rows_track) = rows_track else { return };
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

    let mut new_rows = Vec::new();
    for (panel_i, line) in lines.iter().enumerate() {
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
                s.background_color = Some(ROW_BG_COLOR);
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

        new_rows.push(row_t);
    }

    world.init_component_tree(rows_track, emit);

    if let Some(lc) = world.get_component_by_id_as_mut::<LayoutComponent>(rows_layout_id) {
        lc.dirty = true;
    }

    if let Some(ipc) = world.get_component_by_id_as_mut::<InspectorPanelComponent>(ipc_id) {
        ipc.row_roots = new_rows;
        ipc.inspected = selected;
    }

    ScrollingSystem::set_content_height(world, emit, isc_id, lines.len() as f32 * ROW_HEIGHT);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------


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
