# World Panel Layout Analysis
## ヽ(＾▽＾)ノ what are we actually building, and how should the layout system own it?

---

## 1. Layout System Contract

The minimum unit for a layout item is a **Transform + Style sibling pair**:

```
Layout { width=50gu, unit_scale=0.08 }   // layout root; positions TC children
  Transform { name="header_slot" }        // layout item — LayoutSystem moves this
    Style { height=2gu, margin-bottom=0.5gu }
    // ... content
  Transform { name="content_slot" }       // next layout item
    Style { height=auto }                 // auto → intrinsic content height for display:block
    // ... content
```

`HtmlElement {}` is **not required**. Block is the default formatting context when
`Style.display` is unset. `HtmlElement` is only needed for non-default display modes
(`inline`, `flex`, etc.) or for semantic annotation in document-style rendering.
For ECS UI panels, `Transform + Style` is sufficient.

For `display:block`, unspecified height / `height:auto` should resolve to the smallest box that fits the content. It should not implicitly divide remaining parent height.

---

## 2. Current ECS Hierarchy (World Panel)

```
Selectable { enabled=false }                      // wpa — raycast scope root; panel excluded from picking
  Overlay {}                                       // wpo — always-on-top rendering layer
    Transform { name="panel_transform" }           // panel_t — world pos; GIZMO TARGET
      Layout { name="panel_layout"                 // layout root for header + content
               width=panel_w/TEXT_SCALE gu
               height=(2.0+0.5+content_h/TEXT_SCALE) gu
               unit_scale=TEXT_SCALE }
        Transform { name="header_slot" }           // header — LayoutSystem places at y=0
          HtmlElement { type=header }              // semantic annotation (kept; non-default role)
          Style { height=2gu, margin-bottom=0.5gu }
          Transform { name="panel_titlebar_t"      // title bar rect, world-unit offsets from header_slot
                      pos=(w/2, -0.08, 0.005)
                      scale=(w+0.30, 0.16, 1.0) }
            Color { ... } → Renderable { square }
          Transform { name="panel_titlebar_label_t"
                      pos=(0.02, label_y, 0.01)
                      scale=(0.08, 0.08, 0.08) }
            Color { ... } → Text { "World" }
        Transform { name="content_slot" }          // content — LayoutSystem places at y=-0.20wu
          Style { height=auto }                    // block default; auto height
          Transform { name="drag_plane_t"          // ⚠ hardcoded pos, not a layout item
                      pos=(w/2, -h/2, -0.015)
                      scale=(w+0.30, h+0.30, 1.0) }
            Color { ... } → Renderable { square }
              Opacity { 0.25 }
              Raycastable { drag_only }
              RaycastableShape { quad2d }
          WorldPanel { name="wpc" }
            Scrolling { row_height=0.090, page_size=30 }
              Transform { name="world_panel_rows" }   // wpr — ScrollingComponent moves this
                Layout { name="world_panel_rows_layout"  // ⚠ second layout root (interim)
                         width=panel_w/TEXT_SCALE gu
                         unit_scale=TEXT_SCALE }
                  Transform { name="wp_row_0"            // row — LayoutSystem places
                               scale=(0.08, 0.08, 0.08) }
                    Style { height=auto, margin-left=depth*1.5gu }
                    Color { ... } → Text { "Transform { name=..." }
                      Emissive {}
                      Raycastable { click_only }
                      TextBackground { ... }
                  Transform { name="wp_row_1" ... }
                  // ...
      TransformGizmo { scale=0.25 }                 // finds panel_t via ancestry walk
```

---

## 3. Dimension Constants

| Name | Value | Meaning |
|------|-------|---------|
| `TEXT_SCALE` | 0.08 | world units per glyph unit |
| `ROW_HEIGHT` | 0.090 | world units per row (scroll stride, not layout) |
| `TITLE_BAR_HEIGHT_GU` | 2.0 | glyph units |
| `TITLE_CONTENT_GAP_GU` | 0.5 | glyph units — `header_style.margin.bottom` |
| `INDENT_UNIT` | 0.12 | world units per depth level |
| `INDENT_UNIT_GU` | 1.5 | glyph units per depth level (= INDENT_UNIT / TEXT_SCALE) |
| `DRAG_MARGIN` | 0.15 | world units — symmetric extension (⚠ see Bug A) |
| `DRAG_PLANE_Z_OFFSET` | -0.015 | world units behind content |
| `PAGE_SIZE` | 30 | rows per scroll window |
| `CHAR_WIDTH_GU` | 0.55 | approx glyph width in glyph units (monospace estimate) |

Derived:
- `content_slot y` = `-(TITLE_BAR_HEIGHT_GU + TITLE_CONTENT_GAP_GU) * TEXT_SCALE` = **-0.20 wu**
- `wp_height` = 30 × 0.090 = **2.70 wu** (drag plane / scroll extent)
- `avail_w_gu` = `panel_width_wu / TEXT_SCALE`

---

## 4. What LayoutSystem Does Each Tick

`LayoutSystem::tick()` scans for dirty `LayoutComponent` nodes. Two dirty roots per panel:

### 4.1 `panel_layout` — title bar + content slot

Children with TC: `[header_slot, content_slot]`

```
header_slot:  height=2.0gu, margin_bottom=0.5gu
              → margin_box = 2.0 + 0.5 = 2.5gu
              → placed at y = 0

content_slot: height=auto, no text → is_auto_height=true
              → remaining = total_h - 2.5gu
              → placed at y = -(2.0 + 0.5) * 0.08 = -0.20wu
```

### 4.2 `rows_layout` — row TCs

Children: `[wp_row_0, wp_row_1, ...]` — marked dirty on each `rebuild_world_panel`.

For each row TC:
- `measure_item` reads `Style { height=auto }`, finds `Text {}` in subtree
- `text_intrinsic_height` → `TextSystem::measure(text, min(container_cols, tc.wrap_at))`
- `content_height_gu = line_count` (1.0 per line)
- `block::layout` places row at cursor, preserving TC scale `(0.08, 0.08, 0.08)`

---

## 5. Target: One LayoutComponent per Panel (´･ω･`)

### Current state — two LayoutComponents per panel

| Node | Positions |
|------|-----------|
| `panel_layout` | `header_slot`, `content_slot` |
| `rows_layout` (inside `wpr`) | row TCs |

`rows_layout` is an **interim workaround** for the absence of `overflow: scroll`
in the layout system. It exists because:

1. **Scroll translation** — `ScrollingComponent` moves `wpr` via raw `UpdateTransform`.
   If rows were children of `panel_layout`, scroll would need to be a layout concept.
2. **Row virtualization** — only `PAGE_SIZE` rows live at once; rebuilds need to re-layout
   only the row subtree, not the whole panel.

### Target architecture — one LayoutComponent

```
Transform { name="panel_transform" }
  Layout { name="panel_layout", width=..., unit_scale=TEXT_SCALE }   // THE single root
    Transform { name="header_slot" }
      HtmlElement { type=header }        // kept — semantic role, non-default
      Style { height=2gu, margin-bottom=0.5gu }
      // title bar visuals
    Transform { name="content_slot" }
      Style { height=auto, overflow=scroll }   // scroll container within layout tree
      // drag plane as layout item, not hardcoded
      Transform { name="wp_row_0" }       // rows are children of content_slot
        Style { height=auto, margin-left=depth*1.5gu }
        // ...
  TransformGizmo {}
```

`ScrollingComponent` becomes data-only: stores scroll offset, does NOT emit
`UpdateTransform`. LayoutSystem reads `scroll_offset_gu` from the scroll container
and shifts child placement. `rows_layout` disappears.

### Prerequisites

| Prerequisite | Description |
|---|---|
| `Style { overflow=scroll }` | Makes a block a scroll container within the layout tree |
| Scroll offset in layout pass | `block::layout` (or a scroll variant) offsets child Y by `scroll_offset_gu` |
| Height-based scroll math | Per-row height tracking; scroll stride = actual row height, not fixed `ROW_HEIGHT` |
| Drag plane as layout item | `spawn_drag_plane` replaced by a styled `Transform + Style { width=100%, height=100% }` |

---

## 6. Outstanding Bugs (ノ°▽°)ノ

### Bug A — Drag plane partially overlaps title bar

**Root cause — arithmetic**:

```
// drag plane spawned at pos=(0,0,0) in content_slot local space
h_extended = panel_height + 2 * DRAG_MARGIN   // = wp_height + 0.30wu
cy = -(h_extended / 2)
top_edge_in_content_slot = cy + h_extended/2 = +DRAG_MARGIN = +0.15wu
```

`content_slot` is at y = **-0.20wu** in `panel_t` space, so:

```
drag plane top in panel_t = -0.20 + 0.15 = -0.05wu
title bar spans y = 0 to y = -0.16wu
→ drag plane overlaps 0.11wu of the title bar
```

Hence: partially overlapping the title — not completely over it, not completely below.

**Root cause — structural**: The drag plane is a plain TC child of `content_slot`,
hardcoded at spawn time. LayoutSystem has no knowledge of it; it sits outside the
layout flow entirely.

**Fix**: Asymmetric extent — zero upward margin, extend only downward and sideways:

```rust
let top    = 0.0_f32;
let bottom = -(panel_height + DRAG_MARGIN);
let h      = bottom.abs();
let cy     = top - h * 0.5;
let w      = panel_width + 2.0 * DRAG_MARGIN;
```

---

### Bug B — Scroll culls multi-line rows by index, not height

**Observed**: Scrolling a 2-line (wrapped) row out of view culls it after `ROW_HEIGHT`
(0.090wu) of drag, but the row visually occupies `2 * TEXT_SCALE` = 0.16wu. The second
wrapped line is removed at the same time as the first. A gap appears at the top of the
panel and all rows jump up by one line.

**Root cause**: `ScrollingComponent` counts scroll in **row indices** with fixed stride
`ROW_HEIGHT`. `rebuild_world_panel` renders the window `[start .. start + PAGE_SIZE]`
and LayoutSystem places rows at their measured heights. But `ScrollingComponent` doesn't
know those measured heights — it always treats every row as `ROW_HEIGHT` tall.

When `window_start` increments after `ROW_HEIGHT` of drag:
- The outgoing row was actually `2 * TEXT_SCALE = 0.16wu` tall
- Sub-row offset (`sub_row_y_offset`) snaps to compensate only for `ROW_HEIGHT`
- The snap is `0.090wu` instead of `0.16wu` → visible jump

**Fix** (non-trivial): Store per-row heights in `WorldPanelComponent.row_heights: Vec<f32>`
after each layout pass. Drive `ScrollingComponent` with actual cumulative heights instead
of `index * ROW_HEIGHT`. The scroll extent = sum of all row heights; `window_start`
advances when the cumulative drag exceeds the outgoing row's actual height.

---

## 7. Status Summary ヽ(＾▽＾)ノ

| Issue | Status | Fix |
|-------|--------|-----|
| Gizmo moves only title bar | ✅ Fixed | Gizmo moved to child of `panel_t` |
| Row 0 overlaps title bar | ✅ Fixed | `TITLE_CONTENT_GAP_GU=0.5` as `header_style.margin.bottom` |
| Row positions ignore text wrap height | ✅ Fixed | `rows_layout` LayoutComponent + `text_intrinsic_height` |
| Wrap measurement overestimates cols | ✅ Fixed | `min(container_cols, tc.wrap_at)` in `text_intrinsic_height` |
| Drag plane partially overlaps title (Bug A) | ⚠ Open | Asymmetric `spawn_drag_plane`: zero upward margin |
| Scroll culls multi-line rows wrong (Bug B) | ⚠ Open | Per-row height storage; height-based scroll math |
| Two LayoutComponents per panel | ⚠ Interim | Requires `Style { overflow=scroll }` in layout system |
