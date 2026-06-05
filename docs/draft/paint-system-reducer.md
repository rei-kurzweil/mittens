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

For v1, the reducer-owned state should stay intentionally small.

Even if the initial stroke data is sparse, we should not design `PaintState` in a way that
assumes cloning a future continuous-stroke history will remain cheap. Brush, spray, and line tools
can easily grow per-stroke runtime data into vectors of samples, visited cells, preview handles,
or spacing accumulators.

So the design should separate:

- reducer-owned logical state
- mutable per-stroke runtime state

```rust
struct PaintState {
    selected_asset: Option<PaintSelection>,
    selected_tool: PaintTool,
    focused_panel: Option<ComponentId>,
    stroke: PaintStrokeMode,
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

enum PaintStrokeMode {
    Idle,
    Dragging,
}

struct PaintStrokeRuntime {
    active: bool,
    captured_renderable: Option<ComponentId>,
    non_grid_placed: bool,
    last_grid_step: Option<GridStep>,

    // Intentionally sparse in v1, but expected to grow for line / brush / spray tools.
    // Future candidates:
    // sampled_points: Vec<[f32; 3]>,
    // visited_cells: Vec<GridStep>,
    // preview_handles: Vec<ComponentId>,
    // distance_accumulator: f32,
}
```

### Why split `PaintStrokeMode` from `PaintStrokeRuntime`

If `PaintState` contains a full stroke history, then a reducer shape like:

```rust
let old = state.clone();
let new = reduce_paint_state(&old, &event);
```

gets more expensive as tools become more continuous.

For v1 the runtime stroke data is tiny, but we should still adopt the split now so the design does
not assume full-state cloning remains acceptable once:

- line tool stores endpoints and previews
- free draw stores repeated emit samples
- spray stores spacing / density accumulators
- grid mode tracks many visited cells

The reducer should own only the logical state transition:

- idle vs dragging
- selected asset
- selected tool
- focused panel

The mutable runtime should own large or tool-specific working sets.

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
- `StrokeStarted` updates only `stroke = PaintStrokeMode::Dragging`
- `StrokeEnded` updates only `stroke = PaintStrokeMode::Idle`

The reducer should not:

- spawn assets
- mutate the world
- update panel text
- resolve grid or surface math
- mutate large sampled stroke histories

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
- mutating `PaintStrokeRuntime`
- clearing transient runtime state at stroke end

### Paint-active predicate

```rust
fn paint_is_active(state: &PaintState, paint_panel_root: ComponentId) -> bool {
    state.selected_asset.is_some()
        && matches!(state.selected_tool, PaintTool::FreeDraw)
        && state.focused_panel == Some(paint_panel_root)
}
```

## Runtime mutation model

The reducer should not own large mutable stroke buffers.

Instead:

```rust
struct PaintController {
    state: PaintState,
    stroke_runtime: PaintStrokeRuntime,
}
```

Then the flow becomes:

1. normalize runtime `Signal` into `PaintEvent`
2. reduce `PaintState`
3. apply side effects using:
   - `old_state`
   - `new_state`
   - `event`
   - mutable `PaintStrokeRuntime`

That lets v1 start with a sparse runtime struct while leaving room for richer continuous tools
without forcing deep state cloning.

## One handler path

Instead of separate event branches for UI and scene input, the system should follow this shape:

```rust
fn on_signal(world, emit, signal) {
    let Some(event) = paint_event_from_signal(world, panel_query_root, signal) else {
        return;
    };

    let old = state.clone();
    let new = reduce_paint_state(&old, &event);
    apply_paint_side_effects(
        world,
        emit,
        editor_root,
        &old,
        &new,
        &event,
        &mut stroke_runtime,
    );
    state = new;
}
```

This still clones `PaintState`, but the design intent is that `PaintState` remains small enough for
that to stay cheap. `PaintStrokeRuntime` is where potentially large per-stroke data lives.

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
4. Replace `PaintUiState` and `StrokeState` mutexes with one small `PaintState` plus one
   mutable `PaintStrokeRuntime`.
5. Replace `read_ui_state(...)` with payload-driven reduction.
6. Move status updates entirely into `apply_paint_side_effects(...)`.
7. Keep placement math helpers unchanged initially; only change how activation/state is decided.

## Open question

If later other systems need paint-domain semantic events, we can add an optional second step:

- reduce internally with `PaintEvent`
- optionally emit `EventSignal::PaintStateChanged` or similar

That should come later. The first milestone is a clean internal reducer.
