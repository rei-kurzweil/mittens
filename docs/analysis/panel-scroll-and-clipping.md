# Panel Scroll & Clipping — Issue Analysis

ヽ(＾▽＾)ノ Analysis of problems in `InspectorSystem` / world-panel / inspector-panel.
Updated with render-phase investigation and exact drag-plane geometry.

---

## 1. Drag Plane: Visible, Wrong Render Phase, and Wrong Geometry

### 1a — Visible blue quad (debug artifact)

`spawn_drag_plane` (inspector_system.rs:489) creates a quad with:

- `ColorComponent::rgba(0.3, 0.5, 1.0, 1.0)` — solid blue
- `OpacityComponent { opacity: 0.25, multiple_layers: false }` — semi-transparent
- `RenderableComponent::square()` — **renders visually**

This is leftover debug coloring. In production the drag plane only needs to exist as a raycasting target (`RaycastableComponent::drag_only()`), not as a visible colored quad.

### 1b — Render phase: both drag plane and title bar land in `transparent_single`

Phase selection in `visual_world.rs` (`VisualWorld::rebuild_draw_order`):

```rust
fn is_transparent(inst: &VisualInstance) -> bool {
    inst.opacity < 0.999 || inst.color[3] < 0.999
}

// ...
} else if !Self::is_transparent(inst) {
    self.draw_order.push(...);          // opaque
} else if !inst.multiple_layers {
    self.transparent_single_draw_order.push(...);  // ← HERE
}
```

**Drag plane:** `color[3] = 1.0`, `opacity = 0.25` → `is_transparent = true`, `multiple_layers = false`
→ **transparent_single**

**Title bar background** (`TITLE_BG_COLOR[3] = 0.95`): `color[3] = 0.95`, `opacity = 1.0`
→ `is_transparent = true`, `multiple_layers = false` → **transparent_single**

The `transparent_single` pass is sorted by `(material, mesh, texture, ...)` for batching — **not by depth**:

```rust
self.transparent_single_draw_order.sort_by_key(|&i| {
    (r.material.0, r.mesh.0, tex, ...)  // no Z component
});
```

So whichever quad happens to sort later in material-batch order renders on top, regardless of Z. The drag plane at `DRAG_PLANE_Z_OFFSET = -0.015` is not guaranteed to render behind the title bar at `Z = +0.005` — and in practice it renders on top.

**Fix for rendering**: Make the drag plane fully invisible (`opacity = 0` or no `RenderableComponent`). Then phase classification is moot. Alternatively, to keep a debug visualization, use `multiple_layers = true` so it enters the per-eye depth-sorted `transparent_multi` pass instead.

**Longer-term note on title bar**: `TITLE_BG_COLOR[3] = 0.95` keeps the title bar in `transparent_single` (not opaque). Setting it to `1.0` would move it to the opaque pass (depth-write enabled), which would correctly occlude any transparent quads behind it via depth test.

### 1c — Vertical overlap: drag plane bleeds into title bar zone

Exact geometry (all coordinates in `panel_t` local space, Y-up):

| Node | Y position | Notes |
|------|-----------|-------|
| Title bar top | `0.0` | top of `header_slot` |
| Title bar bottom | `-TITLE_BAR_HEIGHT = -0.16` | = `-2.0 × TEXT_SCALE` |
| Gap bottom (content start) | `-0.20` | = `-(TITLE_BAR_HEIGHT_GU + TITLE_CONTENT_GAP_GU) × TEXT_SCALE` = `-(2.5 × 0.08)` |
| `content_slot` Y (after LayoutSystem) | `-0.20` | LayoutSystem positions correctly |
| Drag plane center Y | `-0.20 + (0 − wp_height/2)` = `-1.55` | `cy = 0 − 1.35` in content_slot local |
| **Drag plane top edge** | `-0.20 + DRAG_MARGIN = -0.20 + 0.15 = -0.05` | **inside title bar zone** |

The drag plane's top edge is at **Y = −0.05**, which is inside the title bar region (Y = 0 to −0.16). The overlap is **0.11 world units** (= 1.375 gu) of title bar area covered by the drag plane.

Root cause: `DRAG_MARGIN = 0.15` extends the drag plane 0.15 world units above `content_slot`'s origin. Since `content_slot` is only 0.04 world units (one `TITLE_CONTENT_GAP`) below the title bar bottom, extending 0.15 up reaches well into the title bar.

**Fix for geometry**: Do not add upward margin. The drag plane should start exactly at `content_slot` origin (Y=0 in content_slot local). At most allow `min(DRAG_MARGIN, TITLE_CONTENT_GAP) = 0.04` upward margin. Alternatively remove DRAG_MARGIN entirely from the top edge — only extend on the bottom and sides.

### Overlay status

Both component doc-comments (`world_panel.rs:9-13`, `inspector_panel.rs:9-13`) describe this topology:

```
SelectableComponent::off()
  OverlayComponent             ← always-on-top
    WorldPanelComponent
```

**The actual spawned topology has no `OverlayComponent`.** Neither panel is in overlay. The stale comments describe a design that was not implemented.

---

## 2. Content Containers Lack `overflow: Scroll` (No Clipping)

### What's happening

Both `spawn_world_panel` and `spawn_inspector_panel` create a `content_slot` TC with:

```rust
let content_style = world.add_component_boxed_named(
    "content_style",
    Box::new(StyleComponent::new()), // ← overflow: Overflow::Visible (default)
);
```

`StyleComponent::new()` defaults to `overflow: Overflow::Visible`. No stencil clip is ever attached to the content area, so:

- Text rows overflow the panel bottom and right edges visually.
- Long identifiers/labels run past the panel boundary (right edge).
- No `StencilClipComponent` is ever spawned for these containers.

### The clip mechanism exists but is not triggered

`block::sync_bg_quad` (layout/block.rs:76) already handles `overflow: Hidden | Scroll`:

```rust
let needs_clip = bg_style
    .map(|(_, _, ov)| matches!(ov, Overflow::Hidden | Overflow::Scroll))
    .unwrap_or(false);
```

When `needs_clip = true` it calls `sync_stencil_clip(world, emit, bg_id, true)` which attaches a `StencilClipComponent` to the `__bg` quad. This requires that the StyleComponent has `background_color: Some(...)` OR that the `needs_clip && no background_color` branch fires (spawns a transparent quad for stencil geometry).

### Fix required

Set `overflow: Scroll` (or `Hidden`) on `content_style` in both panel spawn functions:

```rust
let mut s = StyleComponent::new();
s.overflow = Overflow::Scroll;
// also set background_color so __bg geometry exists for the stencil:
s.background_color = Some([0.0, 0.0, 0.0, 0.0]); // transparent clip mask
```

This will cause `sync_bg_quad` → `sync_stencil_clip` to attach a `StencilClipComponent` to the content area's background quad, clipping all children within the panel bounds.

Note: the `LayoutComponent` for `layout_root` already sets `available_height` correctly (`avail_height_gu = TITLE_BAR_HEIGHT_GU + TITLE_CONTENT_GAP_GU + content_height / TEXT_SCALE`), so the bounding box geometry is available. The clip quad just needs to be triggered by setting overflow.

---

## 3. `ScrollingComponent` Is Still In Use — Should Be Replaced

### Current state

Both panel spawn functions still create a `ScrollingComponent`:

```rust
let wsc = world.add_component_boxed_named(
    "world_panel_scroll",
    Box::new(ScrollingComponent::new(ROW_HEIGHT, PAGE_SIZE)),
);
```

`ScrollingComponent` implements a *virtual window* approach:
- Only `PAGE_SIZE = 30` rows are rendered at any time.
- `apply_drag` advances `scroll_offset` in item-units.
- `sub_row_y_offset = fract(scroll_offset) * item_height` is applied to `rows_anchor` for smooth sub-row interpolation.
- When `window_start` changes, rows are **torn down and rebuilt** (`rebuild_world_panel` / `rebuild_inspector_panel`).

### Intended approach

The layout system now has `overflow: Scroll` support and `LayoutComponent` with `available_height`. The intended model is:
- Render **all rows** inside a layout container with `overflow: Scroll`.
- The scroll state (`scroll_offset_y` in world units) is owned by a `StyleComponent` / `LayoutComponent` field, not a separate component.
- `LayoutSystem` handles row positioning; the scroll container's transform (or a dedicated scroll-offset value in the style) handles the viewport.
- No virtual windowing: rows are all present, clipped by stencil.

`ScrollingComponent` is deprecated under this model. The DragMove handler should update a scroll-position field (in world/glyph units) and mark the container's `LayoutComponent` dirty, rather than calling `apply_drag` and rebuilding a row window.

---

## 4. Y-Offset Drift Bug (Items Shift Upward at Bottom of List)

### Symptom

When scrolling to the very bottom of a long world-panel list (several pages), all rows appear shifted upward by several glyph units. Scrolling back to the top restores correct alignment.

### Root cause: `ROW_HEIGHT` vs LayoutSystem row spacing mismatch

`ScrollingComponent` uses `item_height = ROW_HEIGHT = 0.090` world units for all scroll math. The actual row height placed by `LayoutSystem` is:

```
1.0 glyph unit × TEXT_SCALE = 1.0 × 0.08 = 0.080 world units
```

This is a **0.010 world unit (0.125 gu) discrepancy per row**.

The `sub_row_y_offset`:
```rust
pub fn sub_row_y_offset(&self) -> f32 {
    self.scroll_offset.fract() * self.item_height  // uses 0.090
}
```
uses the wrong item height. At fractional offsets near 1.0 the error peaks at 0.09 − 0.08 = 0.01 world units = 0.125 gu per row. Over a 30-row window this accumulates to ~3.75 gu of misalignment between what the scroll math expects and where LayoutSystem actually places the rows.

### Secondary cause: `init_component_tree(rows_anchor, emit)` after rebuild

`rebuild_world_panel` and `rebuild_inspector_panel` both call:
```rust
world.init_component_tree(rows_anchor, emit);
```
This re-initializes the `rows_anchor` TC subtree. If `TransformComponent::init` emits an `UpdateTransform` or `UpdateTransformWorld` for `rows_anchor` with its *stored* local position (which may have been mutated by the scroll handler's earlier UpdateTransform), it could race with or overwrite the sub_y offset set by the DragMove handler — or the stored position may have drifted from [0,0,0] if the scroll handler's intent was processed and persisted to the component before the rebuild.

### Correct fix

Under the new layout-based scroll model (Issue 3), `sub_row_y_offset` and the virtual window are eliminated. Instead:

1. All rows live in the DOM permanently (or are rebuilt once per selection change, not per scroll step).
2. The scroll container's `overflow: Scroll` stencil clips the visible area.
3. Scroll position is a single world-unit Y offset applied to the inner TC that parents all rows (analogous to CSS `transform: translateY`). This offset is computed directly from drag delta without any `fract()` or `item_height` conversion — just `rows_anchor.y = -drag_accumulated_y`, clamped to `[-(total_height - visible_height), 0]`.
4. No `window_start`, no row rebuild on drag.

Until that migration happens, the immediate workaround is to align `ROW_HEIGHT` with the actual LayoutSystem row height:
```rust
// Was: const ROW_HEIGHT: f32 = 0.090;
const ROW_HEIGHT: f32 = TEXT_SCALE; // 0.080 — one glyph row
```

---

## 5. LayoutSystem Block Layout — Verified Correct

The outer `layout_root` (`panel_layout`) uses `LayoutComponent` with:
- `available_width = panel_width / TEXT_SCALE` (glyph units)
- `available_height = TITLE_BAR_HEIGHT_GU + TITLE_CONTENT_GAP_GU + content_height / TEXT_SCALE`
- `unit_scale = TEXT_SCALE = 0.08`

Block pass positions `header_slot` at cursor=0 → Y=0, advances cursor by `box_height(2.0) + margin_bottom(0.5) = 2.5 gu`. Positions `content_slot` at cursor=2.5 → Y=`-(2.5 × 0.08) = -0.20`. This is correct.

The "layout may be off" impression comes from the **drag plane** extending into the title bar zone (§1c), not from LayoutSystem computing wrong positions. LayoutSystem positions content_slot correctly. The pre-set `with_position(0.0, -TITLE_BAR_HEIGHT, 0.0)` (= -0.16) is close but not exact — LayoutSystem corrects it to -0.20 on the next tick, so there's a **one-frame flicker** on initial spawn where the gap is 0.04 world units too small.

---

## Summary Table

| # | Issue | Location | Status |
|---|-------|----------|--------|
| 1a | Drag plane has visible blue color | `spawn_drag_plane` | Debug artifact, needs `opacity=0` |
| 1b | Both drag plane and title bar in `transparent_single` (no depth sort) | `visual_world.rs` phase sort | Drag plane opacity=0 fixes it; title bar alpha=1.0 would move it to opaque |
| 1c | Drag plane top bleeds 0.11 world units into title bar zone | `spawn_drag_plane` `DRAG_MARGIN` | Remove upward margin or cap it at `TITLE_CONTENT_GAP = 0.04` |
| 1d | `OverlayComponent` in doc-comments but never spawned | `world_panel.rs`, `inspector_panel.rs` | Stale comments |
| 2 | `content_style.overflow = Visible` → no stencil clip | `spawn_world_panel`, `spawn_inspector_panel` | Needs `overflow: Scroll` + `background_color` |
| 3 | `ScrollingComponent` still drives scroll | Both spawn fns + DragMove handlers | Deprecated; migrate to layout-based scroll |
| 4 | Y-drift at bottom of long lists | `sub_row_y_offset`, `ROW_HEIGHT ≠ TEXT_SCALE` | Root fix is §3 migration |
| 5 | LayoutSystem block layout | `layout/block.rs` | Verified correct; 1-frame flicker on first spawn is separate cosmetic issue |
