---

# Panel Drag Plane & Layered Pointer Events ( ˘ω˘ )

## Problem

Scroll panels (world panel, inspector panel) need drag-to-scroll that:

1. **Continues across the full panel surface**, not just the narrow row item geometry.
2. **Doesn't break click selection** on the individual row items behind the drag surface.

The gizmo system solves (1) today via `StartPlaneProjection` — once drag starts, the
gesture system projects cursor movement onto a captured plane rather than requiring
continuous geometry intersection. This works well for gizmos because they start on clearly
defined handle geometry with reliable normals.

For panels, the drag surface is thin scattered text. `StartPlaneProjection` can help, but
an **explicit drag plane** is semantically better: there is one clear owner of the panel's
drag contract. Row items remain individually raycastable for click events — no arithmetic
hit-from-position routing.

The missing primitive is **per-renderable pointer event capture with depth-ordered
propagation stopping**. The raycast + gesture system currently uses a single "best hit"
for everything. We need a sorted hit list where each entry can stop propagation for
specific event types.

---

## Design

### `PointerEvents` — per-renderable capture flags

Add to `RaycastableComponent` (or as a companion `PointerEventsComponent`):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PointerEvents {
    /// Capture all pointer events; stops propagation for drag and click.
    /// Default for all current raycastable geometry.
    #[default]
    All,

    /// Capture drag events only; click propagates to the next hit behind this object.
    DragOnly,

    /// Capture click events only; drag propagates to the next hit behind this object.
    ClickOnly,

    /// Pass all pointer events through; purely visual / structural geometry.
    None,
}
```

"Capture" means: **stop depth-sorted propagation for that event type at this object**.
The object still receives the corresponding `DragStart`/`DragMove`/`DragEnd`/`Click`
signals for the types it captures.

---

### Multi-hit raycast

`RayCastSystem` currently finds the best (closest) BVH candidate and emits one
`RayIntersected`. Change:

- Collect **all** hits in depth order (front to back), not just the best.
- Emit one `RayIntersected` per hit (already-supported for narrow-phase multi-candidate
  resolution) or pass the ordered list directly to `GestureSystem`.
- `GestureSystem` walks the sorted list independently for drag and click resolution.

### Gesture system: event-type–filtered hit selection

**DragStart** — walk the sorted hit list, pick the first entry where
`pointer_events ∈ {All, DragOnly}`. That renderable becomes `active_renderable`.

**At DragStart**, also collect and store the **click-layer hit list**: all hits (in
depth order) where `pointer_events ∈ {All, ClickOnly}`. This list is used at Click time.

**DragMove / DragEnd** — unchanged; go to `active_renderable`.

**Click (DragEnd + displacement < threshold)** — walk the stored click-layer hit list
(from DragStart, not from current frame), pick the **first** entry, dispatch `Click` to
that renderable. If the click-layer list is empty, emit no Click.

This keeps Click semantics anchored to press time (consistent with the existing spec in
`click-and-panel-scroll.md`) and requires no re-raycast at DragEnd.

---

### Panel drag plane

A large invisible quad placed in front of the panel, tagged `DragOnly`:

```
SelectableComponent::off()
  OverlayComponent
    drag_plane_transform   TransformComponent   — panel-width + margin, slightly forward Z
      drag_plane_mesh      RenderableComponent  — invisible quad (alpha ≈ 0 / blend=none)
        drag_plane_rc      RaycastableComponent { pointer_events: DragOnly }
        drag_plane_shape   RaycastableShapeComponent { Quad2D }
    WorldPanelComponent
      ScrollingComponent
        rows_anchor
          row_0            — row items KEEP RaycastableComponent { pointer_events: All }
          row_1
          ...
```

The drag plane is a sibling of `WorldPanelComponent`, parented under the same overlay
node, placed at a slightly smaller Z (closer to camera). It covers:

- **Width**: panel width + `DRAG_MARGIN` on each side (e.g. `0.3` world units)
- **Height**: full panel height + `DRAG_MARGIN` top and bottom
- **Z offset**: `0.001` in front of panel content (overlay-space units)

Because its `pointer_events` is `DragOnly`, click events from the raycast list will skip
it and fall through to whichever row item is hit behind it. Row items keep
`pointer_events: All` (the default), so they stop click propagation at the row level.

### Drag handlers stay on the panel anchor

The drag plane emits `DragStart`/`DragMove`/`DragEnd` events to the drag plane renderable.
Because the drag plane is a descendant of the panel anchor, those events bubble up to the
panel anchor's existing `DragMove` handler. No change needed to handler registration.

Alternatively the handlers can be registered directly on `drag_plane_rc` if more
specificity is wanted.

---

## Propagation stopping semantics

Only the **first** hit in the depth-sorted list that captures an event type gets the
event. Objects behind a capturer are not checked for that type.

```
depth-sorted hits: [drag_plane (DragOnly), row_3 (All), row_2 (All), background (None)]

for drag:   → drag_plane captures, stops. row_3 and row_2 don't see DragStart.
for click:  → drag_plane passes (DragOnly). row_3 captures, stops. row_2 and background
              don't see Click.
```

This prevents spurious multi-target firing and matches the DOM / Unity EventSystem model
of explicit propagation control.

---

## Why not per-item arithmetic click routing

An alternative is: drag plane Click handler computes `row = (hit_y - top_y) / row_height`
and emits a synthetic selection event. This works for uniform-height rows today but:

- Breaks for variable-height rows, nested interactive widgets, or anything that deviates
  from the simple grid
- Duplicates the spatial logic already handled by BVH + raycasting
- Makes the panel "know" about its own layout in a second place

Keeping row items raycastable with proper propagation stopping means the existing
raycast/BVH infrastructure handles all of this correctly.

---

## Changes needed

### `RaycastableComponent`

```rust
pub struct RaycastableComponent {
    pub enabled: bool,
    pub pointer_events: PointerEvents,   // new field, default = All
}
```

### `RayCastSystem`

Instead of emitting a single `RayIntersected` (best hit), collect all hits sorted by
depth and pass the list to `GestureSystem`. Possible shapes:
- Emit `RayIntersected` per hit (gesture system accumulates within the frame)
- Or expose a direct `Vec` query path between the two systems

### `GestureSystem`

At DragStart:
- Walk sorted hits, pick first `pointer_events ∈ {All, DragOnly}` → `active_renderable`
- Store `click_layer_hits: Vec<ComponentId>` — hits where `pointer_events ∈ {All, ClickOnly}`

At DragEnd + small displacement:
- Dispatch `Click` to `click_layer_hits[0]` instead of `active_renderable`

### `InspectorSystem` — `spawn_world_panel` / `spawn_inspector_panel`

Spawn the drag plane as a sibling of the panel content:
- `TransformComponent` sized to `(panel_width + 2*DRAG_MARGIN, panel_height + 2*DRAG_MARGIN)`
- `RenderableComponent` with invisible quad mesh
- `RaycastableComponent { pointer_events: DragOnly }`
- `RaycastableShapeComponent { shape: Quad2D }`

Row items retain `RaycastableComponent` with default `pointer_events: All`.

---

## Open questions

- **Invisible quad mesh**: use a shared asset (unit quad, uploaded once at startup), a
  `TextBackgroundComponent`-style procedural quad scaled to size, or a new `InvisibleQuad`
  renderable primitive?
- **Z ordering in overlay**: does the overlay render phase respect component-tree order for
  depth, or is an explicit Z offset needed for the drag plane to be "in front"?
- **Dynamic panel height**: when `total_items` changes, the drag plane height may need to
  update. Should this be an `UpdateTransform` on scroll total change, or does the plane use
  a fixed oversized height?
- **VR / multi-pointer**: `active_renderable` is currently one global slot. Multi-pointer
  drag capture (two hands scrolling two panels simultaneously) will need per-pointer state.
  Out of scope for now.
