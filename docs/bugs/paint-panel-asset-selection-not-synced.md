# Selecting an asset does not activate the paint panel

## Summary

Clicking an asset in the asset panel updates the asset's `SelectionComponent` in-place, but the
paint state (`PaintState.selected_asset`) is not updated until `begin_frame()` promotes deferred
`SelectionChanged` events. If `is_paint_active()` is evaluated before the next frame, it reports
`asset_ok=false` even though the user has just selected an asset.

The same issue affects the tool selection: clicking a paint tool updates its `SelectionComponent`
synchronously, but the bridge handler that translates this into `PaintEvent::ToolSelectionChanged`
won't fire until the next frame.

## Locations

- `src/engine/ecs/system/editor_paint_system.rs:121-158` — bridge `SelectionChanged` handler at
  `panel_query_root`
- `src/engine/ecs/rx/rx_world.rs:203-206` — `begin_frame()` promotes `deferred_events` to
  `ready_events`
- `src/engine/ecs/system/selection_system.rs:399-462` — `emit_selection_events()` emits
  `SelectionChanged` via whatever `emit` is provided

## Root Cause

The signal/intent model uses two event queues:

| Queue | Source | Promoted by |
|---|---|---|
| `ready_events` | External pushes (`rx.push_event`), CommandQueue `drain_into_rx` | Immediately processed in `process_signals` |
| `deferred_events` | Events emitted inside handler dispatch (via `Emitter`) | `begin_frame()` (once per real frame tick) |

The flow is:

1. User clicks asset item
2. Global Click handler fires (from `ready_events`)
3. `handle_selection_click` → `apply_selection_set` → `emit_selection_events` emits
   `EventSignal::SelectionChanged`
4. **The `emit` inside handler dispatch is the `Emitter`, so the event goes to `deferred_events`**
5. `process_signals` loop ends without dispatching `deferred_events`
6. Bridge handler at `panel_query_root` never fires
7. `PaintState.selected_asset` remains unchanged
8. `is_paint_active()` returns false → paint appears inactive

On the **next frame**, `begin_frame()` promotes `deferred_events` → `ready_events`, then
`process_signals` dispatches them → bridge fires → paint state updates. This one-frame delay is
usually invisible, but if any code checks `is_paint_active()` in the same transaction as the
click, it will see stale state.

## Affected Flow (in production)

1. User opens app → `bootstrap_paint_state` reads current selections → `selected_asset = None`
2. User clicks an asset → asset panel highlights it, `SelectionComponent` updated in-place
3. User clicks scene to place the asset → `handle_scene_click` → `is_paint_active()` returns false
   → `resolve_paint_context` returns `None` → nothing happens
4. User clicks scene again (next frame) → `is_paint_active()` now returns true → asset placed

## Fix Options

1. **`bootstrap_paint_state` after every user click** — not scalable, but works for the test.
2. **Make the bridge handler also listen for `SelectionAdded`/`SelectionRemoved` directly on the
   scope** — events emitted during `emit_selection_events` that target the selection component
   directly, but they'd still go to `deferred_events` via the `Emitter`.
3. **Change `emit_selection_events` to use a direct (non-deferred) emit path when called from
   handler dispatch** — would break the deferred-event contract and could cause re-entrancy.
4. **Call `begin_frame()` at the end of `process_signals`** — promotes deferred events for the
   current frame, making them dispatch in the same `process_signals` cycle. This is the simplest
   fix with the least architectural impact.

## Test Workaround

The current test calls `bootstrap_paint_state` explicitly after pushing Click events and
`process_signals`, which reads the updated `SelectionComponent` state directly (bypassing the
deferred event chain). This works because `bootstrap_paint_state` reads the selection components
directly from the world, not from events.

```rust
// In init_editor_fixture, after Click events and process_signals:
bootstrap_paint_state(&world, runtime_ui_root, &paint_state);
```

This workaround is test-only and does not fix the production behavior.

## Related

- `docs/bugs/single-select-option-deselects-on-re-click.md` — clicking the active tool again
  deselects it, compounding the issue (tool + asset both appear unselected).
