# Layout root should store computed size and emit `LayoutRootSizeAvailable`

Date: 2026-06-06

Status: planning only.

This is a `docs/task` note only. No `src/` changes are proposed here yet.

## Goal

1. After layout runs on a `LayoutRoot` (`LayoutComponent`), store the computed total width/height in world units back on the `LayoutComponent` itself.
2. Emit a new `EventSignal::LayoutRootSizeAvailable { layout_id, width_wu, height_wu }` so downstream systems can react to the size.
3. Have the editor workspace system listen for this event and shift the entire workspace layout root upward by its own height (`+height_wu` on Y), so content that accumulates downward (panels, rows, wrapped items) does not end up underground.

## Problem

Layout computes item positions correctly, but the total extent of a `LayoutRoot` is never stored or communicated:

- `LayoutComponent` has only input fields (`available_width`, `available_height`, `dirty`, `unit_scale`).
- No output/computed-size field exists.
- No event fires when layout finishes, so consumers must poll `dirty == false` or read ephemeral `LayoutBoundsComponent` subtrees.

The editor workspace layout root places panels that stack downward (Y accumulates). Without knowing the root's total height, the workspace cannot shift itself up so its top edge sits at the origin. The result: content below the first row sits at negative Y and may be underground or clipped.

## Proposed changes

### Phase 1: store computed size on `LayoutComponent`

Add a new field:

```rust
/// Computed total extent of this layout root's direct children, in world units,
/// populated after each layout pass. `None` before the first layout.
pub computed_size_wu: Option<(f32 /* width */, f32 /* height */)>,
```

- Width: the furthest right edge of the top-level items (margin-box extents), converted to world units. For typical block layout where items fill available width, this is effectively `available_width * unit_scale`.
- Height: the sum of top-level items' `margin_box_height_gu * unit_scale` (block), or `(cursor_y_gu + line_height_gu) * unit_scale` (inline).

### Phase 2: emit `LayoutRootSizeAvailable` event

Add a variant to `EventSignal`:

```rust
LayoutRootSizeAvailable {
    layout_id: ComponentId,
    width_wu: f32,
    height_wu: f32,
}
```

And a corresponding `SignalKind::LayoutRootSizeAvailable`.

The event is scoped to `layout_id`. Emit it from `LayoutSystem::tick()` after `run_layout()` returns and we've stored `computed_size_wu`.

### Phase 3: block/inline layout returns total height

Modify `block::layout()` and `inline::layout()` — or the tick's post-layout computation — to derive the total height from the top-level `MeasuredItem` list.

For block: `items.iter().map(|i| i.margin_box_height_gu).sum::<f32>() * unit_scale`

For inline: compute `(cursor_y_gu + line_height_gu) * unit_scale` (the last Y cursor position after the loop).

Width: the maximum right edge of items in world units. For block items that stretch (auto-width), use `available_width * unit_scale`. For fixed-width items, use the maximum `(margin_left + box_width + margin_right) * unit_scale`.

### Phase 4: editor workspace consumes the event

The workspace layout root in the editor (owned by `EditorContextSystem` or the panel setup in `editor_inspector_system_stopgap_mms_adapter.rs`) should listen (via `RxWorld::add_handler_closure`) for `LayoutRootSizeAvailable` on its own layout root.

When the event fires:

```rust
// Shift the root's TransformComponent translation Y by +height_wu.
// This moves the entire layout subtree up so its top sits at y=0.
```

Concretely, the handler fetches the root's `TransformComponent` and emits `UpdateTransform` with `translation[1] += height_wu`.

The handler should be registered once when the editor workspace bootstraps, scoped to the workspace layout root.

## Why this is better

- Layout root becomes self-describing: you can read its computed size without walking children.
- Event-driven: consumers don't poll or guess.
- The workspace shift becomes a reactive response to actual layout, not a hardcoded or post-hoc estimate.
- Adding/removing panels or changing their sizes automatically repositions the workspace root on the next layout pass.

## Edge cases

- **First layout before trees are populated**: `computed_size_wu` remains `None`. No event emitted. Layout with zero items produces `(0.0, 0.0)`.
- **Empty layout root**: width/height are 0; handler should not shift (or shift by 0, which is a no-op).
- **Inline layout wrapping multiple lines**: height must account for all lines, not just a single pass. After the loop in `inline::layout_items`, `cursor_y_gu + line_height_gu` is the total.
- **Nested layout roots**: each `LayoutRoot` in the tree gets its own `computed_size_wu` and emits its own event. Only the editor workspace root's handler shifts its parent transform.
- **Re-layout after dirtied**: every layout pass updates `computed_size_wu` and re-emits the event. The editor handler should compare against the previous height to avoid no-op `UpdateTransform`s.

## Implementation order

1. Add `computed_size_wu` to `LayoutComponent`
2. Add `EventSignal::LayoutRootSizeAvailable` + `SignalKind::LayoutRootSizeAvailable` to signal.rs
3. Modify `block::layout()` and `inline::layout()` to return total size (or compute it from items)
4. Modify `LayoutSystem::tick()` to store the size + emit event after `run_layout()`
5. Register handler in the editor workspace bootstrap to shift the root TC up by `height_wu`

## Relevant files

- `src/engine/ecs/component/layout.rs` — add `computed_size_wu` field
- `src/engine/ecs/rx/signal.rs` — add `EventSignal::LayoutRootSizeAvailable` + `SignalKind`
- `src/engine/ecs/system/layout/mod.rs` — store size + emit event in `tick()`
- `src/engine/ecs/system/layout/block.rs` — return/derive total height from items
- `src/engine/ecs/system/layout/inline.rs` — return/derive total height from items
- `src/engine/ecs/system/editor_context_system.rs` — register handler for the event
- `src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs` — where the workspace layout root is created

## Related

- `docs/task/editor-workspace-width-from-post-layout-bounds.md` — similar theme of post-layout measurement for workspace geometry
