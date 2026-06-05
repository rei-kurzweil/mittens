# Paint System Reducer Event Model

## Problem

The current `PaintSystem` still mixes two different models:

- it listens to engine `EventSignal`s directly
- it also reconstructs paint UI state by querying the world

That leaves it in a half-push / half-pull state:

- asset/tool/panel changes arrive as `SelectionChanged`
- but paint does not treat those payloads as the canonical state transition input
- instead it re-reads `SelectionComponent` state from the world

That is not the reducer model we want.

## What should emit `AssetSelectionChanged`?

Not the engine core.

`AssetSelectionChanged` is not a good built-in `EventSignal` candidate because it is:

- editor-specific
- paint/workflow-specific
- derived from the more general `SelectionChanged`

If we add a built-in engine event for every domain-specific interpretation of `SelectionChanged`,
the global `EventSignal` enum will become a bag of panel-specific semantics.

So the question is really:

- should `AssetSelectionChanged` be a new runtime signal variant?
- or should it be an internal paint-domain event derived from `SelectionChanged`?

For Paint, the right first answer is:

- keep `SelectionChanged` as the canonical runtime event
- derive a paint-domain event inside `PaintSystem`

## Event-shape options

### Option A — use raw engine signals directly in the reducer

Example:

- reducer input is `EventSignal`
- `PaintSystem` matches on `SelectionChanged`, `Click`, `DragStart`, `DragMove`, `DragEnd`

Pros:

- no extra event type
- direct mapping from runtime to system

Cons:

- reducer becomes tied to unrelated global signal payloads
- lots of selector checks leak into reducer logic
- panel-specific interpretation gets duplicated in multiple match arms

This is better than the current pull model, but still too low-level for Paint.

### Option B — internal `PaintEvent` enum derived from engine signals

Example:

```rust
enum PaintEvent {
    AssetSelectionChanged { item: Option<String>, component: Option<ComponentId> },
    ToolSelectionChanged { item: Option<String>, component: Option<ComponentId> },
    PanelFocusChanged { focused_panel: Option<ComponentId> },
    SceneClick { renderable: ComponentId, hit_point: [f32; 3] },
    StrokeStarted { renderable: ComponentId, hit_point: [f32; 3] },
    StrokeMoved { renderable: ComponentId, hit_point: [f32; 3] },
    StrokeEnded,
}
```

Pros:

- keeps runtime event vocabulary small
- gives Paint a clean domain reducer API
- one normalization step can map engine signals to paint semantics
- no extra global signal variants needed

Cons:

- requires one normalization layer
- the event is private to Paint unless another system also wants it

This is the best near-term fit.

### Option C — new global runtime `EventSignal` variants like `AssetSelectionChanged`

Pros:

- any system can subscribe to the event directly
- payload can be pre-shaped for editor tools

Cons:

- global signal space gets polluted with editor-panel semantics
- unclear ownership: should `SelectionSystem`, adapter, or Paint emit it?
- tends to duplicate `SelectionChanged` rather than abstract it

This is only justified if the event becomes broadly useful across multiple systems.

## Recommended direction

Use:

- global runtime `SelectionChanged`
- internal `PaintEvent`
- a single normalization function:

```rust
fn paint_event_from_signal(
    world: &World,
    panel_query_root: ComponentId,
    signal: &Signal,
) -> Option<PaintEvent>
```

That function should:

- inspect `SelectionChanged.selection_root`
- inspect the payload already carried by `SelectionChanged`
- emit a paint-domain event only when the changed selection root is relevant to Paint

Important:

- do not call `read_ui_state(...)`
- do not treat the world as the source of truth for asset/tool/focus state
- the source of truth for paint UI transitions should be the event payload

## Reducer shape

The state should be unified:

```rust
struct PaintState {
    selected_asset: Option<PaintSelection>,
    selected_tool: PaintTool,
    focused_panel: Option<ComponentId>,
    stroke: PaintStrokeState,
}
```

Then:

```rust
fn reduce_paint_state(old: &PaintState, event: &PaintEvent) -> PaintState
```

The reducer should not:

- query the world for `SelectionComponent`
- decide side effects
- spawn assets

The reducer should only compute the next logical paint state.

## Side-effect phase

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

This phase may:

- update paint status text
- begin or clear stroke state runtime bookkeeping
- attempt placement on scene gesture events when the reduced state is active

This keeps reducer logic deterministic and testable.

## Relationship to signal pipelines

This does **not** require a new engine-level event primitive.

It is exactly the pattern already described in
[docs/draft/event-signal-pipelines.md](../draft/event-signal-pipelines.md):

- subscribe to upstream engine events
- project them into component-local semantic events

The only difference is that for Paint we may keep the projection fully internal instead of
re-emitting it as a runtime `EventSignal`.

## Recommendation

For the next Paint refactor:

1. Introduce a private `PaintEvent` enum in `paint_system.rs`.
2. Introduce a unified `PaintState`.
3. Replace `read_ui_state(...)` with event-driven state updates from `SelectionChanged` payloads.
4. Keep scene interaction events (`Click`, `Drag*`) as raw runtime signals, but normalize them
   into `PaintEvent` before reduction.
5. Keep runtime `EventSignal` unchanged for now.

If later multiple systems want to observe paint-domain transitions, then we can revisit whether
some `Paint*Changed` runtime signal should be promoted into `EventSignal`.
