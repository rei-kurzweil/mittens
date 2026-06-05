# Draft: Paint System Reducer

## Goal

Refactor `PaintSystem` into a single-owner reducer-driven state machine.

The key requirements are:

- scene paint input still comes from editor-tree gesture events
- asset/tool/panel state changes come from `SelectionChanged`
- Paint does not pull UI state by querying `SelectionComponent`
- one reducer owns all paint state transitions
- one side-effect phase owns status updates and placement work

## Non-goal

This draft does **not** require adding new built-in runtime `EventSignal` variants such as
`AssetSelectionChanged`.

Instead, Paint should define a private event enum and map engine signals into it.

## Proposed state

```rust
struct PaintState {
    selected_asset: Option<PaintSelection>,
    selected_tool: PaintTool,
    focused_panel: Option<ComponentId>,
    stroke: PaintStrokeState,
}

struct PaintSelection {
    item: Option<String>,
    component: Option<ComponentId>,
}

enum PaintTool {
    FreeDraw,
    Line,
    SprayCan,
    Fill,
    Erase,
    Unknown(Option<String>),
}

struct PaintStrokeState {
    active: bool,
    captured_renderable: Option<ComponentId>,
    non_grid_placed: bool,
    last_grid_step: Option<GridStep>,
}
```

## Proposed event enum

```rust
enum PaintEvent {
    AssetSelectionChanged {
        item: Option<String>,
        component: Option<ComponentId>,
    },
    ToolSelectionChanged {
        tool: PaintTool,
        item: Option<String>,
        component: Option<ComponentId>,
    },
    PanelFocusChanged {
        focused_panel: Option<ComponentId>,
    },
    SceneClick {
        renderable: ComponentId,
        hit_point: [f32; 3],
    },
    StrokeStarted {
        renderable: ComponentId,
        hit_point: [f32; 3],
    },
    StrokeMoved {
        renderable: ComponentId,
        hit_point: [f32; 3],
    },
    StrokeEnded,
}
```

## Normalization layer

All engine events that Paint cares about should first go through one function:

```rust
fn paint_event_from_signal(
    world: &World,
    panel_query_root: ComponentId,
    signal: &Signal,
) -> Option<PaintEvent>
```

This function should:

- accept raw runtime `Signal`
- inspect the payload once
- decide whether that signal is relevant to Paint
- produce one `PaintEvent`

### SelectionChanged mapping

For `SelectionChanged`, mapping should be driven by `selection_root`.

If:

- `selection_root == #assets_selection`

emit:

```rust
PaintEvent::AssetSelectionChanged {
    item: selected_entry.item.clone(),
    component: selected_component,
}
```

If:

- `selection_root == #paint_tool_selection`

emit:

```rust
PaintEvent::ToolSelectionChanged { ... }
```

If:

- `selection_root == #editor_panel_layout_selection`

emit:

```rust
PaintEvent::PanelFocusChanged { ... }
```

Important:

- use the `SelectionChanged` payload directly
- do not query `SelectionComponent` to rebuild the same information

### Gesture mapping

Map:

- `Click` → `PaintEvent::SceneClick`
- `DragStart` → `PaintEvent::StrokeStarted`
- `DragMove` → `PaintEvent::StrokeMoved`
- `DragEnd` → `PaintEvent::StrokeEnded`

Only emit those when the hit belongs to the correct editor subtree and is not UI/gizmo content.

## Reducer

```rust
fn reduce_paint_state(old: &PaintState, event: &PaintEvent) -> PaintState
```

Examples:

- `AssetSelectionChanged` updates only `selected_asset`
- `ToolSelectionChanged` updates only `selected_tool`
- `PanelFocusChanged` updates only `focused_panel`
- `StrokeStarted` updates `stroke.active`, `captured_renderable`, and clears per-stroke placement memory
- `StrokeEnded` clears stroke state

The reducer should not:

- spawn assets
- mutate the world
- update panel text
- resolve grid or surface math

## Side effects

After reduction:

```rust
fn apply_paint_side_effects(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    editor_root: ComponentId,
    old: &PaintState,
    new: &PaintState,
    event: &PaintEvent,
)
```

This function handles:

- updating paint status text when visible state changed
- attempting placement when gesture events arrive and the reduced state is paint-active
- clearing transient state at stroke end

### Paint-active predicate

```rust
fn paint_is_active(state: &PaintState, paint_panel_root: ComponentId) -> bool {
    state.selected_asset.is_some()
        && matches!(state.selected_tool, PaintTool::FreeDraw)
        && state.focused_panel == Some(paint_panel_root)
}
```

## One handler path

Instead of separate event branches for UI and scene input, the system should follow this shape:

```rust
fn on_signal(world, emit, signal) {
    let Some(event) = paint_event_from_signal(world, panel_query_root, signal) else {
        return;
    };

    let old = state.clone();
    let new = reduce_paint_state(&old, &event);
    apply_paint_side_effects(world, emit, editor_root, &old, &new, &event);
    state = new;
}
```

There may still be multiple runtime subscriptions:

- one scoped to `panel_query_root` for `SelectionChanged`
- one scoped to `editor_root` for gesture events

But those subscriptions should all delegate into the same reducer entrypoint.

## Why this is better

- no duplicate status ownership
- no `read_ui_state(...)` pull path
- all paint UI state changes come from explicit events
- reducer tests can run without world mutation
- gesture logic and UI logic are unified under one state machine

## Suggested implementation order

1. Introduce `PaintTool`, `PaintSelection`, `PaintStrokeState`, `PaintState`.
2. Introduce `PaintEvent`.
3. Add `paint_event_from_signal(...)`.
4. Replace `PaintUiState` and `StrokeState` mutexes with one `PaintState` mutex.
5. Replace `read_ui_state(...)` with payload-driven reduction.
6. Move status updates entirely into `apply_paint_side_effects(...)`.
7. Keep placement math helpers unchanged initially; only change how activation/state is decided.

## Open question

If later other systems need paint-domain semantic events, we can add an optional second step:

- reduce internally with `PaintEvent`
- optionally emit `EventSignal::PaintStateChanged` or similar

That should come later. The first milestone is a clean internal reducer.
