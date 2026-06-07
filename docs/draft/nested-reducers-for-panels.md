# Nested Reducers for the Panel System

## Why

The stopgap adapter mixes model building, state mutation, handler registration, UI
tree construction, and destructive rerendering in one 3873-line file. The existing
reducers (`reduce_editor_context_state`, `reduce_inspector_workspace_state`,
`reduce_paint_state`) are flat — each handles events at one level, missing the
natural nesting of the domain. As new panels and interactions are added, the
flat approach leads to duplicated event variants in every reducer and unclear
ownership of state fields.

Nested reducers mirror the tree of state: each level owns exactly its fields,
delegates children to sub-reducers, and can be tested independently.

---

## State Tree

```
EditorWorkspace (top-level coordinator)
 ├── context: EditorContextState       (which editor tree is active, what's selected in it)
 ├── focus: InputFocusState            (what currently has input focus — a panel slot or an editor tree)
 ├── editor_trees: EditorTreeCollection(per-tree settings: visibility, locked)
 ├── world_panel: WorldPanelState      (rows, selected index for the "World" tree panel)
 ├── inspector: InspectorWorkspaceState(open inspector panel instances)
 ├── asset_panel: AssetPanelState      (asset list, filter, selection)
 └── paint: PaintState                 (tool, asset template, stroke state)
```

A note on terminology: there is no "scene" primitive. The world is a graph of
component trees. Trees whose root carries an `EditorComponent` are
"editor-managed trees" — they appear in the world panel, can be selected and
manipulated, and have per-tree settings like visibility and locked edits.
We refer to them as **editor trees**.

### Leaf Shapes

```rust
// ── EditorContextState (exists today) ──────────────────────────────────────
struct EditorContextState {
    active_editor: Option<ComponentId>,
    selected_component: Option<ComponentId>,
}

// ── InputFocusState (replaces PanelFocusState from earlier drafts) ─────────
// There are two kinds of focus target: a panel slot (world, inspector, asset,
// paint) or an editor tree (clicked in the 3D viewport / an editor tree's row
// in the world panel). Only one can be active at a time.
struct InputFocusState {
    target: Option<InputFocusTarget>,
}

enum InputFocusTarget {
    Panel { kind: FocusedPanelKind },
    EditorTree { editor_root: ComponentId },
}

enum FocusedPanelKind {
    World,
    Inspector { panel_id: InspectorPanelId },
    Asset,
    Paint,
}

// ── EditorTreeCollection (new) ─────────────────────────────────────────────
// Manages per-editor-tree settings that the world panel exposes as toggle
// icons on each editor section header.
struct EditorTreeCollection {
    trees: HashMap<ComponentId, EditorTreeSettings>,
}

struct EditorTreeSettings {
    visible: bool,      // show/hide the tree's renderables in the viewport
    locked: bool,       // prevent transform edits + viewport selection
}

// ── WorldPanelState (new) ──────────────────────────────────────────────────
// Currently implicit — the world panel's "selected index" is read directly
// from the ECS SelectionComponent on the world_panel_selection node.
// Pulling it into state makes the reducer pure and diffable.
struct WorldPanelState {
    selected_component: Option<ComponentId>,
    expanded: Vec<ComponentId>,
    scroll_offset: i32,
}

// ── InspectorPanelState (exists today) ─────────────────────────────────────
struct InspectorPanelState {
    panel_id: InspectorPanelId,
    editor_root: ComponentId,
    inspected: Option<ComponentId>,
    pinned: bool,
    subtree_selection: InspectorSubtreeSelection,
    scroll_offset: InspectorScrollState,
}
struct InspectorSubtreeSelection {
    focused_row: Option<ComponentId>,
    expanded: Vec<ComponentId>,
}

// ── InspectorWorkspaceState (exists today) ─────────────────────────────────
struct InspectorWorkspaceState {
    panels: Vec<InspectorPanelState>,
    active_panel: Option<InspectorPanelId>,
    next_panel_id: InspectorPanelId,
}

// ── AssetPanelState (new) ──────────────────────────────────────────────────
// Currently populated during spawn_panel_layout + never updated.
struct AssetPanelState {
    filter_text: String,
    selected_item_index: Option<usize>,
    expanded_modules: Vec<ModuleId>,
}

// ── PaintState (exists today) ──────────────────────────────────────────────
struct PaintState {
    selected_asset: Option<PaintSelection>,
    selected_tool: PaintTool,
    stroke: PaintStrokeMode,
}
```

---

## Event Hierarchy

Each level has its own event enum. A parent reducer matches on its events and
delegates sub-events:

```rust
// ── Top-level coordinator event ────────────────────────────────────────────
enum EditorWorkspaceEvent {
    // Input focus routing
    FocusChanged {
        target: InputFocusTarget,
    },

    // Editor-tree selection (click in 3D viewport + SelectionChanged on an
    // editor root). Focuses the tree + updates selected_component.
    EditorTreeSelectionChanged {
        editor: ComponentId,
        component: Option<ComponentId>,
    },

    // Per-editor-tree settings toggled from world panel header icons
    EditorTreeSettingsChanged {
        editor: ComponentId,
        setting: EditorTreeSetting,
    },

    // World panel events — delegate to world_panel reducer, then maybe sync
    // to context and inspector
    WorldPanel(WorldPanelEvent),

    // Inspector events — delegate to inspector reducer
    Inspector(InspectorWorkspaceEvent),

    // Asset panel events
    AssetPanel(AssetPanelEvent),

    // Paint events
    Paint(PaintEvent),
}

// ── Per-panel events ───────────────────────────────────────────────────────
enum WorldPanelEvent {
    RowClicked {
        target_component: ComponentId,
        index: usize,
    },
    ExpandToggled {
        component: ComponentId,
    },
    Scrolled {
        delta: i32,
    },
}

enum InspectorWorkspaceEvent {
    SelectionChanged {
        editor_root: ComponentId,
        selected_target: Option<ComponentId>,
    },
    PanelFocused {
        panel_id: InspectorPanelId,
    },
    PanelPinToggled {
        panel_id: InspectorPanelId,
    },
    SidebarRowClicked {
        panel_id: InspectorPanelId,
        target_component: ComponentId,
    },
}

enum AssetPanelEvent {
    ItemSelected {
        index: usize,
    },
    FilterChanged {
        text: String,
    },
    ModuleExpanded {
        module_id: ModuleId,
    },
    ModuleCollapsed {
        module_id: ModuleId,
    },
}

// PaintEvent stripped of cross-cutting variants. Paint only receives
// tool/asset/stroke events. Selection changes that affect paint are
// forwarded by the coordinator, not encoded here.
enum PaintEvent {
    ToolSelectionChanged {
        tool: PaintTool,
    },
    AssetSelectionChanged {
        item: Option<String>,
        component: Option<ComponentId>,
    },
    StrokeStarted { editor: ComponentId, renderable: ComponentId, hit_point: [f32; 3] },
    StrokeMoved  { editor: ComponentId, renderable: ComponentId, hit_point: [f32; 3] },
    StrokeEnded  { editor: ComponentId },
}

// ── Settings events ────────────────────────────────────────────────────────
enum EditorTreeSetting {
    VisibilityToggled,
    LockToggled,
}
```

---

## Reducer Tree

### Leaf reducers (pure, testable in isolation)

```rust
fn reduce_world_panel_state(old: &WorldPanelState, event: &WorldPanelEvent) -> WorldPanelState {
    let mut new = old.clone();
    match event {
        WorldPanelEvent::RowClicked { target_component, .. } => {
            new.selected_component = Some(*target_component);
            new.scroll_offset = 0;
        }
        WorldPanelEvent::ExpandToggled { component } => {
            if let Some(pos) = new.expanded.iter().position(|c| c == component) {
                new.expanded.remove(pos);
            } else {
                new.expanded.push(*component);
            }
        }
        WorldPanelEvent::Scrolled { delta } => {
            new.scroll_offset += delta;
        }
    }
    new
}

fn reduce_input_focus_state(old: &InputFocusState, event: &EditorWorkspaceEvent) -> InputFocusState {
    let mut new = old.clone();
    match event {
        EditorWorkspaceEvent::FocusChanged { target } => {
            new.target = Some(*target);
        }
        EditorWorkspaceEvent::EditorTreeSelectionChanged { editor, .. } => {
            // Clicking in an editor tree focuses that tree
            new.target = Some(InputFocusTarget::EditorTree { editor_root: *editor });
        }
        _ => {}
    }
    new
}
```

### Editor-tree settings reducer

```rust
fn reduce_editor_tree_collection(
    old: &EditorTreeCollection,
    event: &EditorWorkspaceEvent,
) -> EditorTreeCollection {
    let mut new = old.clone();
    match event {
        EditorWorkspaceEvent::EditorTreeSettingsChanged { editor, setting } => {
            let settings = new.trees.entry(*editor).or_default();
            match setting {
                EditorTreeSetting::VisibilityToggled => settings.visible = !settings.visible,
                EditorTreeSetting::LockToggled => settings.locked = !settings.locked,
            }
        }
        _ => {}
    }
    new
}
```

### Coordinator reducer (orchestrates cross-panel sync)

Instead of each system independently subscribing to `SelectionChanged` and
duplicating the event-extraction + sync logic, a single coordinator maps raw
signals to the event tree and calls sub-reducers:

```rust
fn reduce_editor_workspace_state(
    old: &EditorWorkspaceState,
    event: &EditorWorkspaceEvent,
) -> EditorWorkspaceState {
    let mut new = old.clone();

    match event {
        // ── Focus routing ─────────────────────────────────────
        EditorWorkspaceEvent::FocusChanged { .. } => {
            new.focus = reduce_input_focus_state(&old.focus, event);
        }

        // ── Editor-tree selection changes context + inspector ─
        EditorWorkspaceEvent::EditorTreeSelectionChanged {
            editor, component
        } => {
            new.focus = reduce_input_focus_state(&old.focus, event);
            new.context = reduce_editor_context_state(&old.context,
                &EditorContextEvent::EditorSelectionChanged {
                    editor: *editor,
                    component: *component,
                },
            );
            new.inspector = reduce_inspector_workspace_state(&old.inspector,
                &InspectorWorkspaceEvent::SelectionChanged {
                    editor_root: *editor,
                    selected_target: *component,
                },
            );
        }

        // ── Editor tree settings ──────────────────────────────
        EditorWorkspaceEvent::EditorTreeSettingsChanged { .. } => {
            new.editor_trees = reduce_editor_tree_collection(&old.editor_trees, event);
        }

        // ── World panel selection syncs to context + inspector ─
        EditorWorkspaceEvent::WorldPanel(event) => {
            let old_world = &old.world_panel;
            new.world_panel = reduce_world_panel_state(old_world, event);

            if old_world.selected_component != new.world_panel.selected_component {
                if let Some(target) = new.world_panel.selected_component {
                    if let Some(editor) = nearest_editor_ancestor(target) {
                        new.focus.target = Some(InputFocusTarget::EditorTree { editor_root: editor });
                        new.context = reduce_editor_context_state(&new.context,
                            &EditorContextEvent::ActiveEditorChanged {
                                editor: Some(editor),
                                selected_component: Some(target),
                            },
                        );
                        new.inspector = reduce_inspector_workspace_state(&new.inspector,
                            &InspectorWorkspaceEvent::SelectionChanged {
                                editor_root: editor,
                                selected_target: Some(target),
                            },
                        );
                    }
                }
            }
        }

        // ── Inspector events — pure delegation ────────────────
        EditorWorkspaceEvent::Inspector(event) => {
            new.inspector = reduce_inspector_workspace_state(&old.inspector, event);
        }

        // ── Asset panel events ────────────────────────────────
        EditorWorkspaceEvent::AssetPanel(event) => {
            new.asset_panel = reduce_asset_panel_state(&old.asset_panel, event);
        }

        // ── Paint events ─────────────────────────────────────
        EditorWorkspaceEvent::Paint(event) => {
            new.paint = reduce_paint_state(&old.paint, event);
        }
    }

    new
}
```

---

## Handler Wiring (how raw signals become events)

Each handler is ~5 lines. The coordinator reducer handles sync.

```rust
fn install_shared_panel_handlers(
    rx: &mut RxWorld,
    panel_query_root: ComponentId,
    workspace: &Arc<Mutex<EditorWorkspaceState>>,
) {
    rx.add_handler_closure(
        SignalKind::SelectionChanged,
        panel_query_root,
        move |world, _emit, signal| {
            let Some(event) = extract_workspace_event(world, panel_query_root, signal) else {
                return;
            };
            apply_workspace_event(workspace, event);
        },
    );
}

// The single chokepoint that maps raw SelectionChanged signals to the
// nested event tree:
fn extract_workspace_event(
    world: &World,
    panel_query_root: ComponentId,
    signal: &Signal,
) -> Option<EditorWorkspaceEvent> {
    let EventSignal::SelectionChanged { selection_root, .. } = signal.event.as_ref()? else {
        return None;
    };

    if is_layout_selection(world, panel_query_root, *selection_root) {
        Some(EditorWorkspaceEvent::FocusChanged {
            target: resolve_focused_panel(world, panel_query_root, signal),
        })
    } else if is_world_panel_selection(world, panel_query_root, *selection_root) {
        extract_world_panel_event(world, signal).map(EditorWorkspaceEvent::WorldPanel)
    } else if is_editor_selection(world, *selection_root) {
        extract_editor_selection_event(world, signal)
            .map(EditorWorkspaceEvent::EditorTreeSelectionChanged)
    } else if is_inspector_sidebar_selection(...) {
        // ...
    } else if is_asset_panel_selection(...) {
        // ...
    } else if is_paint_tool_selection(...) {
        // ...
    } else {
        None
    }
}
```

Editor tree clicks are handled separately because `SelectionChanged` on an
editor root carries the selected component directly (no SemanticTarget
resolution needed):

```rust
fn extract_editor_selection_event(
    world: &World,
    signal: &Signal,
) -> Option<EditorTreeSelectionChanged> {
    let EventSignal::SelectionChanged {
        selection_root,
        selected_component,
        ..
    } = signal.event.as_ref()?;

    if !world.get_component_by_id_as::<EditorComponent>(*selection_root).is_some() {
        return None;
    }

    Some(EditorTreeSelectionChanged {
        editor: *selection_root,
        component: *selected_component,
    })
}
```

---

## World Panel UI — Editor Tree Headers with Toggle Icons

The world panel lists all editor-managed trees, each prefixed by a header row.
The header shows the editor tree's label (e.g. `Editor#avatar_scene`) plus
three toggle icons:

```
 Editor#avatar_scene             [eye] [lock]
 ├── scene_root
 │   ├── mesh
 │   └── armature_pelvis
 │       ├── spine_01
 │       └── ...
 ├── camera_rig
 └── lights

 Editor#ui_overlay              [eye] [lock]
 ├── panel_root
 └── cursor
```

The icons are defined as MMS exports in `assets/components/icons.mms`, exactly
like the paint panel's `pencil_icon`, `line_icon`, etc. — they return simple
`T { ... }` subtrees with colored geometry:

| MMS export | Purpose | Binding |
|---|---|---|
| `eye_icon` | Toggle tree visibility | `EditorTreeSetting::VisibilityToggled` |
| `lock_icon` | Lock/unlock edits + selection | `EditorTreeSetting::LockToggled` |

Locking a tree implies both preventing transform gizmo edits AND preventing
viewport click-selection on that tree's contents. There is no separate
selectability toggle — `locked = true` gates both.

### Rendering pattern

Each icon sits inside a clickable item that follows the same `Option {}` +
`Raycastable` + `Style` pattern used by the paint panel items. The world panel
header row spawns them like this (in `assets/components/panel_items.mms`):

```mms
import { eye_icon, lock_icon } from "./icons.mms"

export fn editor_tree_header(label, icon_setting_bg) {
    return T {
        name = "editor_tree_header"
        Style {
            display("inline-block")
            width(100%)
            height(2.5)
            background_color([0.30, 0.84, 0.38, 0.98])
        }
        T { Text { label } }
        T.position(12.0, 0.0, 0.0) {
            Option {}
            Raycastable.enabled()
            Style {
                display("inline-block")
                width(1.5) height(1.5)
                background_color(icon_setting_bg)
            }
            T.scale(0.25, 0.25, 1.0) { T { eye_icon() } }
        }
        T.position(14.0, 0.0, 0.0) {
            Option {}
            Raycastable.enabled()
            Style {
                display("inline-block")
                width(1.5) height(1.5)
                background_color(icon_setting_bg)
            }
            T.scale(0.25, 0.25, 1.0) { T { lock_icon() } }
        }
    }
}
```

Unlike the paint panel's tool selection (which uses the selection system's
gold highlight), the toggle icons need **persistent** visual state — the icon
should look different when a setting is active vs inactive. The view-diff pass
handles this by replacing each icon's background color based on
`EditorTreeSettings`:

```
active   background = [0.95, 0.82, 0.18, 1.0]  (gold, setting is on)
inactive background = [0.10, 0.55, 0.18, 1.0]  (green, setting is off)
```

This matches how the inspector's pin button toggles its background color
between gold/green in the current stopgap adapter.

### Click handling

Each icon sits in a `SelectionComponent` scope on the world panel (the same
one used for row selection). Clicking an icon fires `SelectionChanged` with a
different semantic target than a row click. `extract_workspace_event`
distinguishes them:

```rust
fn extract_editor_tree_icon_click(
    world: &World,
    signal: &Signal,
    panel_query_root: ComponentId,
) -> Option<EditorWorkspaceEvent> {
    let EventSignal::SelectionChanged {
        selection_root, selected_component, ..
    } = signal.event.as_ref()?;

    let Some(clicked) = selected_component else { return None };

    // Resolve the clicked icon's parent editor tree and which setting it toggles.
    // Icon items carry a DataComponent with keys `editor_root` and `setting_kind`.
    resolve_editor_tree_setting_click(world, panel_query_root, clicked)
        .map(|(editor, setting)| EditorWorkspaceEvent::EditorTreeSettingsChanged {
            editor,
            setting,
        })
}
```

The view-diff pass syncs `EditorTreeSettings` to the actual `EditorComponent`
fields on each tick so the rest of the engine (gizmo, raycasting, rendering)
respects the per-tree toggles.

---

## Diff / View Layer (separate from reducers)

Once the workspace state has been reduced, a separate pass compares old/new
state and emits minimal intents:

```rust
fn diff_and_update_views(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    old: &EditorWorkspaceState,
    new: &EditorWorkspaceState,
    rendered_inspector_models: &Arc<Mutex<Vec<InspectorPanelModel>>>,
) {
    if old.world_panel.selected_component != new.world_panel.selected_component
        || old.world_panel.expanded != new.world_panel.expanded
    {
        let model = build_world_panel_model(world, &new.world_panel);
        rerender_world_panel_content(world, emit, ..., &model);
    }

    if old.editor_trees != new.editor_trees {
        // Sync EditorTreeSettings → EditorComponent fields
        for (editor_root, settings) in &new.editor_trees.trees {
            if old.editor_trees.trees.get(editor_root) != Some(settings) {
                apply_editor_tree_settings(world, emit, *editor_root, settings);
            }
        }
    }

    if old.inspector != new.inspector {
        let inspector_models = build_inspector_panel_models(world, &new.inspector);
        rerender_inspector_panels(world, emit, ..., &inspector_models, rendered_inspector_models);
    }

    if old.focus != new.focus {
        // Update panel title bar highlights or editor tree outline
    }
}
```

---

## Performance Characteristics

| Approach | Clone cost per reduce call | Allocation pattern |
|---|---|---|
| Current (`old.clone()` on flat state) | O(N) panels, each with Vec expansions | Heaps of short-lived allocations |
| Nested reducers + full clone | Same O(N) but **only for the relevant subtree** — selecting an asset panel item only clones `AssetPanelState`, not 256 inspector panels | Less churn |
| Nested + `Arc<Mutex<leaf>>` | O(1) — pointer bumps for unchanged leaves | Mutation in place on the affected leaf |
| Nested + `im::Vector` | O(log N) — structural sharing on Vec backing | Single small allocation per edit |

For the current scale (~1-10 inspector panels, ~500 world panel rows), the
nested reducers with full clone are fine. The `Arc<Mutex<leaf>>` approach
adds locking complexity to what should be a pure function and makes snapshot
comparison for the view-diff layer harder (you'd need to snapshot before
mutating).

**Recommendation:** Use nested reducers with `Clone` on leaf states. If
`InspectorWorkspaceState::panels` grows beyond ~50 instances, wrap it in
`im::Vector` — but only after profiling shows it matters.

---

## Migration Path

1. Extract `InputFocusState` from `EditorContextState.focused_panel`
2. Create `WorldPanelState` + `WorldPanelEvent` + `reduce_world_panel_state`
3. Create `AssetPanelState` + `AssetPanelEvent` + `reduce_asset_panel_state`
4. Create `EditorTreeCollection` + `EditorTreeSettings` + per-tree reducer
5. Build `EditorWorkspaceState` that composes all leaf states
6. Write `reduce_editor_workspace_state` as the coordinator
7. Write `extract_workspace_event` — the single place that maps raw signals
   to the nested event tree
8. Strip cross-cutting event variants from `PaintEvent`
9. Wire the view-diff pass into the tick

Steps 1-4 can happen in parallel. Step 6 replaces the inline mutation in the
current handlers. Step 8 is the payoff — each panel reducer becomes
self-contained.
