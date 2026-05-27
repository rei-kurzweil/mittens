# Click Events and Panel Scrolling

## Overview

This document specifies two related features:

1. **`Click` event signal** — a higher-level gesture emitted by `GestureSystem` when a
   drag gesture ends close enough to where it started (i.e. the pointer didn't actually
   move meaningfully).
2. **World panel scrolling** — drag-to-scroll behaviour for the world panel, using
   `Click` for row selection and `DragMove` for scroll.

---

## ♡ Click event signal

### Motivation

Currently `GestureSystem` emits only `DragStart` / `DragMove` / `DragEnd`. Row selection
in the world panel uses `DragStart` as a proxy for "clicked a row". This is incorrect:
it fires immediately on press, before the user has a chance to drag-scroll the panel.
We need a proper click that fires only if the pointer didn't travel far.

### Definition

A **click** is a drag gesture where the net displacement from start to end is below a
threshold. All intermediate `DragMove` events are still emitted during the gesture; the
`Click` event is emitted *additionally* at `DragEnd` time, on the same scope.

```rust
EventSignal::Click {
    raycaster: ComponentId,
    renderable: ComponentId,
    hit_point: [f32; 3],
    screen_pos_px: Option<(f32, f32)>,
}
```

Payload mirrors `DragStart` (the thing clicked is what was hit at press time, not
at release).

### Threshold

**Screen-space threshold** (primary): if `screen_pos_px` is available on both
`DragStart` and `DragEnd`, compare Euclidean pixel distance. Default: **8 px**.

**World-space fallback**: if screen position is unavailable (XR pointer etc.), use
Euclidean world-space distance on `hit_point`. Default: **0.02 world units**.

Both thresholds are `GestureSystem` constants; no per-pointer configuration is needed
for stage 1.

### Emission rules

- `Click` is emitted on the **same scope** as `DragStart`/`DragEnd` — the raycaster
  component.
- `Click` fires **after** `DragEnd` (same frame, same `process_signals` cycle).
- If the threshold is exceeded no `Click` is emitted; only `DragStart/Move/End` occur.
- The intermediary `DragMove` signals are always emitted regardless of whether the
  gesture will ultimately be classified as a click.

### GestureSystem changes

`GestureSystem` already tracks `DragStart` state. Two additions:

1. Store `start_screen_pos_px: Option<(f32, f32)>` and `start_hit_point: [f32; 3]` from
   `DragStart`.
2. At `DragEnd`: compute displacement; if below threshold, emit `Click` with the
   start-time renderable and hit_point.

The classification happens entirely at `DragEnd` time — no mode-switching mid-gesture.

---

## ⊹ World panel scroll

### Motivation

The world panel currently shows `MAX_ROWS = 30` rows and silently truncates the list.
Long component trees need scrolling. The panel also uses `DragStart` for row selection,
which will be replaced by `Click` once that exists.

### Scroll state

`WorldPanelComponent` gains:

```rust
pub scroll_offset_rows: f32,  // continuous; 0.0 = top
```

`scroll_offset_rows` is a non-negative float. The visible window shows rows
`[floor(scroll_offset_rows) .. floor(scroll_offset_rows) + MAX_ROWS)` from the full
node list.

### Drag-to-scroll

The world panel anchor registers a `DragMove` handler (replacing the existing
`DragStart` row-selection handler). On each `DragMove`:

```
scroll_offset_rows -= delta_world.y / ROW_HEIGHT
scroll_offset_rows  = clamp(scroll_offset_rows, 0.0, max_scroll)
max_scroll = (total_nodes - MAX_ROWS).max(0) as f32
```

`delta_world.y` is the world-space vertical drag delta from `DragMove`. Dragging
up (positive Y) reveals items lower in the list (higher row index), so the sign
is negated.

> **Why world-space delta?** The panel is in overlay space, and ROW_HEIGHT is
> defined in overlay world units (0.045). Using `delta_world.y` directly gives
> a natural 1:1 correspondence between pointer travel and content movement.

### Row selection via Click

Replace the world panel's `DragStart` handler with a `Click` handler. This
ensures a drag-scroll gesture does not also trigger row selection.

### Rebuild trigger

Rebuild the panel rows whenever `floor(scroll_offset_rows)` changes. The rebuild
call (`rebuild_world_panel`) is already cheap — it detaches old rows, creates new
ones with the correct offset window, and calls `init_component_tree`.

`rebuild_world_panel` passes the window start index to `collect_visible_nodes` (or
slices the returned `Vec` from index `floor(scroll_offset_rows) as usize`).

Row vertical positions use the **panel-local** row index (0..MAX_ROWS), not the
global node index:

```rust
let panel_row_i = i;  // 0-based within the visible window
y_pos = -(panel_row_i as f32) * ROW_HEIGHT
```

The effective node index into the full list is `window_start + panel_row_i`.

### Smooth feel

Because `scroll_offset_rows` is a float and the rebuild fires only at integer
crossings, the panel snaps cleanly at row boundaries rather than continuously
re-spawning text. Within a single row step the content doesn't move — which is
intentional: the visual jump happens at the row boundary, giving a "click to
advance" feel rather than free-scroll.

If smooth free-scroll is desired later, the row transforms could be offset by the
fractional part of `scroll_offset_rows` each frame. This is a follow-up.

### Clamping and edge cases

- `scroll_offset_rows` is clamped to `[0, max_scroll]` on every drag move.
- `max_scroll = 0` when `total_nodes <= MAX_ROWS`; dragging does nothing.
- Selected node highlight stays correct: `find_highlighted` works on the full node
  list regardless of scroll window; the rebuild passes `selected` as before.

---

## Signal summary

| Signal | Emitter | When |
|---|---|---|
| `Click` | `GestureSystem` | `DragEnd` + displacement < threshold |
| `DragMove` | `GestureSystem` | Every frame during drag (unchanged) |

### EventSignal variant to add

```rust
Click {
    raycaster: ComponentId,
    renderable: ComponentId,
    hit_point: [f32; 3],
    screen_pos_px: Option<(f32, f32)>,
},
```

---

## Implementation checklist

- [ ] Add `EventSignal::Click` variant to `signal.rs`
- [ ] Add `SignalKind::Click` variant
- [ ] `GestureSystem`: store start position, emit `Click` at `DragEnd` if below threshold
- [ ] `WorldPanelComponent`: add `scroll_offset_rows: f32`
- [ ] `rebuild_world_panel`: accept `window_start: usize`, slice node list accordingly
- [ ] `InspectorSystem`: replace `DragStart` row handler with `Click` handler
- [ ] `InspectorSystem`: add `DragMove` scroll handler on panel anchor
- [ ] Update `inspector-panel.md` to note scroll state field
