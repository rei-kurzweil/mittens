# World Panel Layout Analysis
## ヽ(＾▽＾)ノ what are we actually building, and how should the layout system own it?

---

## 1. Current ECS Hierarchy

### 1.1 Full static tree (World Panel)

```
SelectableComponent::off()           [wpa — raycast scope root]
  OverlayComponent                   [wpo]
    TransformComponent               [panel_t — world pos, scale=1. GIZMO TARGET]
      LayoutComponent                [layout_root — flex-column manager, dirty=true on spawn]
        TransformComponent           [header_slot — flex item #1]
          StyleComponent             [height: GlyphUnits(2.0), flex_grow: 0.0]
          TransformComponent         [bar_t — pos=(w/2, -0.16/2, 0.005), scale=(w+0.30, 0.16, 1)]
            ColorComponent → RenderableComponent::square()   [title bar rect]
          TransformComponent         [label_t — pos=(0.02, label_y, 0.01), scale=(0.08,0.08,0.08)]
            ColorComponent → TextComponent "World"
        TransformComponent           [content_slot — flex item #2, pre-set pos=(0,-0.16,0)]
          StyleComponent             [flex_grow: 1.0, height: Auto → 0 gu]
          TransformComponent         [drag_plane_t — pos=(w/2, -h/2, -0.015), scale=(w+0.30, h+0.30, 1)]
            ColorComponent → RenderableComponent::square()
              OpacityComponent { opacity: 0.25 }
              RaycastableComponent::drag_only()
              RaycastableShapeComponent::Quad2D
          WorldPanelComponent        [wpc — non-TC]
            ScrollingComponent       [wsc — non-TC]
              TransformComponent     [wpr / rows_anchor — pos=(0,0,0)]
                TransformComponent   [row_0 — pos=(indent, 0, 0), scale=(0.08,...)]
                  ColorComponent → TextComponent
                    EmissiveComponent
                    RaycastableComponent::click_only()
                    TextBackgroundComponent { padding_top: 0, padding_bottom: ROW_GAP_FILL }
                      ColorComponent
                TransformComponent   [row_1 — pos=(indent, -0.090, 0), scale=(0.08,...)]
                  ...
      TransformGizmoComponent        [panel_gizmo — finds panel_t via ancestry walk]
```

### 1.2 Dimension constants

| Name | Value | Meaning |
|------|-------|---------|
| `TEXT_SCALE` | 0.08 | world units per glyph unit |
| `ROW_HEIGHT` | 0.090 | world units per row |
| `TITLE_BAR_HEIGHT` | 2 × 0.08 = **0.16** | world units |
| `TITLE_BAR_HEIGHT_GU` | **2.0** | glyph units |
| `DRAG_MARGIN` | **0.15** | world units — how far the drag plane extends past content edges |
| `DRAG_PLANE_Z_OFFSET` | -0.015 | world units behind content |
| `PAGE_SIZE` | 30 | rows per page |
| `ROW_GAP_FILL` | ROW_HEIGHT/TEXT_SCALE − 1 = **0.125** | gu padding_bottom per row |
| `PANEL_V_PADDING` | 0.35 | gu (unused as top padding now) |

Derived:
- `wp_height` = 30 × 0.090 = **2.7 wu**
- `wp_width` = `estimate_panel_width(DEFAULT_WRAP_AT, 0.08, 5 × 0.12)` ≈ varies
- `avail_height_gu` = 2.0 + 2.7 / 0.08 = **35.75 gu**

---

## 2. What LayoutSystem Does Each Tick

`LayoutSystem::tick()` scans for dirty `LayoutComponent` nodes and calls `layout_flex_column`.

For `layout_root`:
- Children with TC: `[header_slot, content_slot]`
- `header_slot` style: `height=GlyphUnits(2.0)`, `flex_grow=0`  → fixed item
- `content_slot` style: `height=Auto(→0)`, `flex_grow=1.0`  → flex item

Calculation:
```
total_fixed_gu = 2.0
total_grow     = 1.0
remaining_gu   = 35.75 - 2.0 = 33.75

cursor=0 → header_slot: y_local = 0 * 0.08 = 0.0   → UpdateTransform [0, 0.0, 0]
cursor=2 → content_slot: y_local = -2 * 0.08 = -0.16 → UpdateTransform [0, -0.16, 0]
```

After `queue.flush`, `content_slot.transform.translation = [0, -0.16, 0]` and
`transform_changed` propagates this through the non-TC chain down to `wpr` and all rows.

So in **panel_t local space**:
- Title bar rect: spans y = 0 to y = **−0.16**
- `content_slot` origin: y = **−0.16**
- Row 0 origin: y = **−0.16** (wpr at [0,0,0] relative to content_slot)
- Row 1 origin: y = −0.16 − 0.090 = **−0.250**
- etc.

---

## 3. Bug #1 — Rows Overlap the Title Bar

### Root cause

Row origins are computed in **world units** relative to `wpr`. `wpr` is at [0,0,0] relative
to `content_slot`. So row 0 starts at the SAME y as `content_slot`.

That's correct: title bar bottom = −0.16, content top = −0.16. They should just touch.

The apparent visual overlap has two candidate explanations:

**A. First-frame world matrix stale (most likely)**
`spawn_world_panel` pre-sets `content_slot` local pos to [0, −0.16, 0].
`world.init_component_tree(wpa, emit)` emits `RegisterTransform` for every TC including
`content_slot`. If `RegisterTransform` resets or re-initialises the local TRS before
propagating, content_slot's world matrix could be identity for one frame.
`rebuild_world_panel` is called **after** `init_component_tree`, so the row TCs are built
while content_slot's world matrix may already be wrong.
LayoutSystem fixes it on the next tick, but any visual caching done between
`init_component_tree` and the first `layout.tick` will see y=0 for `content_slot`.

**B. TextBackgroundComponent top-of-row z-fighting**
`TextBackgroundComponent` default `z_offset = -0.1` gu × TEXT_SCALE = −0.008 wu.
Row 0 at y = −0.16 wu in panel space renders its background slightly behind the row text,
but the TITLE BAR background `bar_t` has z = +0.005 relative to `header_slot`.
Both nodes share the same parent chain up to `panel_t`. If the title bar z-fights
with the row background the title bar wins visually, which looks like overlap.

**The deeper structural issue**
The title bar rect (`bar_t`) is authored at position `(w/2, -TITLE_BAR_HEIGHT/2, 0.005)`
**relative to `header_slot`**. `header_slot` is positioned at y=0 by LayoutSystem.
This means the bar visually spans from y=0 down to y=−0.16 in panel_t space.

Row 0's `TextBackgroundComponent` background has `padding_top=0` and its TC is at y=−0.16.
The background quad is at z = row_t.z + z_offset × TEXT_SCALE. This is *just barely*
adjacent to the title bar bottom edge — any sub-pixel rounding or padding inconsistency
causes a one-pixel visual overlap.

The fix requires a small vertical gap: either `TITLE_BAR_HEIGHT` should be slightly larger
than `2 × TEXT_SCALE`, or `content_slot` should be positioned at `y = -(TITLE_BAR_HEIGHT + gap)`.

---

## 4. Bug #2 — Drag Plane Covers the Title Bar

### Root cause — arithmetic

`spawn_drag_plane` is called with `parent=content_slot`, `pos=(0,0,0)`, height=`wp_height=2.7`:

```
cx = 0 + wp_width / 2
cy = 0 - wp_height / 2  = -1.35
cz = 0 + DRAG_PLANE_Z_OFFSET = -0.015

w  = wp_width + 2 * DRAG_MARGIN = wp_width + 0.30
h  = wp_height + 2 * DRAG_MARGIN = 2.7 + 0.30 = 3.0
```

In **content_slot local space**, the drag plane quad spans:
```
y_top    = cy + h/2 = -1.35 + 1.50 = +0.15
y_bottom = cy - h/2 = -1.35 - 1.50 = -2.85
```

The drag plane's top edge is at y = **+0.15** relative to `content_slot`.

Since `content_slot` is at y = −0.16 in `panel_t` space:
```
drag plane top in panel_t space = -0.16 + 0.15 = -0.01
```

The title bar occupies y = 0 to y = −0.16 in panel_t space.
The drag plane **extends 0.01 wu below the title bar top** — covering 15/16 of the title bar.

### Why this happens

`DRAG_MARGIN = 0.15` was chosen to give comfortable drag affordance around panel edges.
But it is symmetric: it extends equally upward (into the title bar) and downward (below content).

For the content slot the upward extension should be **zero** (or a tiny epsilon) since:
1. The title bar has its own drag affordance via `TransformGizmoComponent`.
2. The drag plane is a scroll/translation capture quad, not a panel-move handle.
3. Extending it into the title bar area means scroll drags intercept hits that should
   go to the gizmo drag handles.

---

## 5. The Layout System Architecture We're Building Toward

### 5.1 Components in play

| Component | Role |
|-----------|------|
| `TransformComponent` | Position anchor in 3D space; world matrix propagation root |
| `LayoutComponent` | Flex-column container; marks the root of a CSS-like layout subtree |
| `StyleComponent` | Per-item CSS box model properties (`height`, `flex_grow`, margins, padding) |
| `HtmlElementComponent` | Semantic/structural role (`Header`, `Body`, `Div`, `Span`, ...). Currently **defined but not used** in panel spawning |
| `TextComponent` | Inline text content |
| `TextBackgroundComponent` | Inline background quad behind text, using glyph-space padding |
| `RenderableComponent` | Mesh-backed geometry |

### 5.2 What the layout system owns (intended contract, now implemented)

```
LayoutComponent         ← "I am the containing block"
  TransformComponent    ← block item — LayoutSystem moves this via UpdateTransform
    StyleComponent { display: Block, height: GlyphUnits(N) }   ← fixed height
    [content...]
  TransformComponent    ← next block item
    StyleComponent { display: Block }   ← height: Auto → fills remaining space
    [content...]
```

`LayoutSystem::tick()`:
- Finds dirty LayoutComponents
- Reads each TC child's StyleComponent for `display` and `height`
- `display: Block` + `height: GlyphUnits(N)` → fixed height item
- `display: Block` + `height: Auto` in a fixed-height container → fills remaining space
  (semantically: a block element in a constrained column expands to fill available space,
   analogous to CSS block formatting context behaviour)
- Computes vertical cursor positions (top-to-bottom in glyph units × unit_scale)
- Emits `UpdateTransform` for each TC child

`flex_grow` is still available for explicit proportional distribution but should be
considered an escape hatch — prefer `display: block` + `height: auto` for the common
"fill remaining space" case.

This is analogous to CSS `display: flex; flex-direction: column` on the layout root,
with each direct TC child being a flex item.

### 5.3 What `HtmlElementComponent` should add (not yet wired)

`HtmlElementComponent` provides semantic HTML-like roles. The intended integration:

- `HtmlElementComponent::Header` → corresponds to `header_slot` — LayoutSystem treats it
  as a block with intrinsic height from StyleComponent
- `HtmlElementComponent::Body` → corresponds to `content_slot` — LayoutSystem treats it
  as a flex-grow block

Currently these components exist but are NOT read by `LayoutSystem`. `LayoutSystem` only looks
for TC children with a sibling `StyleComponent`. The `HtmlElementComponent` layer is
preparatory scaffolding for a richer layout algorithm that would also compute inline layout,
handle wrapping, margin collapsing, etc.

**Short-term plan**: keep using TC + StyleComponent as flex items. `HtmlElementComponent` can
be attached for future semantics without breaking current behavior.

### 5.4 What row layout owns

Row positions are **NOT driven by LayoutSystem** — they are manually placed by
`rebuild_world_panel` / `rebuild_inspector_panel`:

```rust
row_t.position = [depth * INDENT_UNIT, -(panel_i as f32) * ROW_HEIGHT, 0.0]
row_t.scale    = [TEXT_SCALE, TEXT_SCALE, TEXT_SCALE]
```

These positions are relative to `wpr` (rows_anchor), which is a TC at [0,0,0] inside
`content_slot`. This is intentional: rows are rebuilt on scroll and use the scroll
offset to produce a continuous sub-row y translation.

This layer should remain manually managed for now — it is essentially a virtualized list
renderer, and LayoutSystem's flex-column pass is not the right abstraction for a virtual
scroll window with N×30 items.

---

## 6. Required Fixes

### Fix A — Drag plane: remove upward DRAG_MARGIN

The drag plane should not extend above the content area. Change `spawn_drag_plane` call
(or internally) so that the top edge sits exactly at y=0 relative to `content_slot`:

```
cy = -(panel_height + DRAG_MARGIN) / 2   →  top = DRAG_MARGIN/2, wrong
```

Instead: only extend downward and sideways, not upward:

```rust
// Only extend down and sideways — not up into title bar territory.
let h_extended = panel_height + DRAG_MARGIN;   // DRAG_MARGIN only below
let cy = -(h_extended / 2.0);                  // center below content origin
```

Or more explicitly: clamp the top of the drag plane to y=0 in content_slot space:

```rust
let top    = 0.0_f32;
let bottom = -(panel_height + DRAG_MARGIN);
let h      = top - bottom;
let cy     = (top + bottom) / 2.0;
```

### Fix B — Sub-pixel title bar gap

Add a 1–2 pixel gap between the title bar bottom edge and the content top edge.
Either:
- Increase `TITLE_BAR_HEIGHT` by a small epsilon (e.g. add 1 px in glyph units)
- Or emit `content_slot` at y = `-(TITLE_BAR_HEIGHT + GAP)` and add the same GAP
  to `avail_height_gu` so LayoutSystem accounts for it

Suggested constant: `const TITLE_CONTENT_GAP: f32 = 0.005;` (5 mm in world space).
Update `TITLE_BAR_HEIGHT_GU` accordingly or adjust `layout_flex_column` cursor.

### Fix C — Eliminate first-frame stale world matrix

The pre-set `with_position(0.0, -TITLE_BAR_HEIGHT, 0.0)` on `content_slot` is fragile —
it depends on `RegisterTransform` not resetting the local TRS.

Audit: does `RegisterTransform` / `UpdateTransformWorld` preserve the existing local TRS
or reset it? If it resets, the pre-set is useless and rows will be at y=0 on the first frame.

Options:
1. **Force an immediate layout pass** during `spawn_world_panel`, before `rebuild_world_panel`.
   Call `LayoutSystem::layout_flex_column(world, emit, layout_id)` directly (needs it to be
   pub or extracted), then process those intents, THEN call `rebuild_world_panel`.
2. **Accept one-frame stale** and rely on LayoutSystem's first tick to fix it.
   This is usually invisible at 72+ Hz VR but can cause a flicker on initial spawn.
3. **Don't pre-set `content_slot` position** — let LayoutSystem be the single source of
   truth. Rows will be at y=0 until LayoutSystem runs, but since rows_anchor_base_pos=[0,0,0]
   and `wpr` is in content_slot space, this means rows will visually appear at the title bar
   bottom until the first layout tick.

Option 1 is cleanest but requires a small refactor to expose or extract the flex pass.

---

## 7. Structural Simplification (Medium Term)

The current manual hierarchy has several sources of coupling:
- `bar_t` is positioned relative to `header_slot` using hard-coded offsets in world units
- `label_t` position is computed from `TITLE_BAR_HEIGHT` and `TEXT_SCALE`
- `drag_plane_t` is positioned relative to `content_slot` using content dimensions

A cleaner architecture using HtmlElementComponent + LayoutSystem fully:

```
panel_t (TC, world pos)
  layout_root (LayoutComponent, unit_scale=TEXT_SCALE)
    header_slot (TC + HtmlElementComponent::Header)
      StyleComponent { height: GlyphUnits(2.0) }
      [title bar rect — authored in glyph space relative to header_slot]
      [title label — authored in glyph space]
    content_slot (TC + HtmlElementComponent::Body)
      StyleComponent { flex_grow: 1.0 }
      [drag_plane — authored in content-slot-local space, no upward margin]
      [scroll content]
  gizmo (TransformGizmoComponent)
```

Within `header_slot` (which LayoutSystem positions to y=0 and height=2 gu), all children
should be authored in **glyph units** (i.e. with scale=1, since TEXT_SCALE is the unit_scale).
This means `bar_t` scale = (width_gu, 2.0, 1.0) with position at the glyph-unit midpoint.

This would eliminate the world-unit/glyph-unit impedance mismatch that causes the
sub-pixel overlap issues described in section 3B.

---

## 8. Summary

| Issue | Root Cause | Fix |
|-------|------------|-----|
| Rows appear to overlap title bar | `content_slot` pre-set pos may be stale on frame 0; sub-pixel adjacency with z-fighting | Immediate layout flush during spawn + small gap constant |
| Drag plane covers title bar | `DRAG_MARGIN` extends symmetrically upward into title bar | Asymmetric drag plane: no upward extension |
| Gizmo moves only title bar | ✅ **Fixed** — gizmo moved to child of `panel_t` | — |
| Row 0 background overlaps bar | ✅ **Fixed** — `padding_top=0.0` for all rows | — |
