# LayoutSystem Implementation Plan ٩(◕‿◕｡)۶
## Checklist for full box-model block layout

Tracks what's done, what's next, and what's deferred.

---

## Current state

`LayoutSystem::layout_flex_column` is a **position-only pass** with no box model:
- reads `style.height` (GlyphUnits only)
- reads `style.flex_grow` / `display: Block + height: Auto` → fill remaining
- emits `UpdateTransform` with `y = -(cursor_gu * unit_scale)`
- cursor advances by raw `content_height` only

No padding. No margin. No horizontal axis. No measurement struct.

---

## Axis model

| Axis | Block layout | Flex-row layout |
|------|-------------|-----------------|
| **Vertical (block axis)** | cursor moves down per item | items share available height by flex-grow |
| **Horizontal (inline axis)** | each block stretches to fill width; no cursor | cursor moves right per item |

**Block layout has no horizontal cursor.** Width is resolved per-element:

```
content_width = available_width
              - margin.left - margin.right
              - padding.left - padding.right
```

Each block starts at `x = (margin.left + padding.left) * unit_scale` relative to the
container origin. No cursor needed — all blocks in a column share the same x origin.

A horizontal cursor only appears in **flex-row** or **inline** contexts.

---

## Implementation checklist

### Phase A — Measurement struct ( Pass 1 )

- [ ] Define `MeasuredItem` in `layout_system.rs`:
  ```rust
  struct MeasuredItem {
      tc_id:             ComponentId,
      // vertical
      content_height_gu: f32,   // from style.height or intrinsic
      padding_top_gu:    f32,
      padding_bottom_gu: f32,
      margin_top_gu:     f32,
      margin_bottom_gu:  f32,
      box_height_gu:     f32,         // padding_top + content_height + padding_bottom
      margin_box_height_gu: f32,      // margin_top + box_height + margin_bottom
      is_auto_height:    bool,        // true → gets a share of remaining space
      // horizontal
      padding_left_gu:   f32,
      padding_right_gu:  f32,
      margin_left_gu:    f32,
      margin_right_gu:   f32,
  }
  ```

- [ ] Write `fn measure_item(world, tc_id, avail_w_gu) -> MeasuredItem`:
  - find `StyleComponent` among `tc_id`'s children
  - read `padding`, `margin`, `height`, `display`
  - compute `content_width_gu = avail_w_gu - margin.left - margin.right - padding.left - padding.right`
  - for `height: GlyphUnits(n)`: `content_height = n`, `is_auto = false`
  - for `height: Auto` + `display: Block`: `content_height = 0`, `is_auto = true`
  - fill all `MeasuredItem` fields

- [ ] Write `fn measure_items(world, layout_id) -> (Vec<MeasuredItem>, f32, f32)`:
  - returns `(items, avail_w_gu, avail_h_gu?)` from `LayoutComponent`
  - iterates direct TC children, calls `measure_item` for each
  - sums `total_fixed_margin_box_gu` for non-auto items

### Phase B — Space distribution ( Pass 1 → 2 bridge )

- [ ] After measuring, resolve auto heights:
  ```
  remaining_gu = avail_h - total_fixed_margin_box_gu
  auto_each_margin_box = remaining_gu / count_auto_items
  ```
  For each auto item, set:
  ```
  margin_box_height_gu = auto_each_margin_box
  box_height_gu        = margin_box_height_gu - margin_top - margin_bottom
  content_height_gu    = box_height_gu - padding_top - padding_bottom
  ```

### Phase C — Layout pass ( Pass 2 )

- [ ] Replace `layout_flex_column` with `fn layout_items(items, avail_h, unit_scale, emit)`:
  ```
  vertical_cursor = 0.0

  for item in items:
      vertical_cursor += item.margin_top_gu

      content_origin_y = vertical_cursor + item.padding_top_gu
      content_origin_x = item.margin_left_gu + item.padding_left_gu

      emit UpdateTransform {
          tc_id: item.tc_id,
          translation: [
              content_origin_x * unit_scale,
              -(content_origin_y * unit_scale),
              0.0,
          ],
          scale: [1.0, 1.0, 1.0],
      }

      vertical_cursor += item.box_height_gu
      vertical_cursor += item.margin_bottom_gu
  ```

- [ ] Remove `flex_grow` from `flex_item_style` / rename to `measure_item`
- [ ] Update `tick` to call measure then layout

### Phase D — Inspector system cleanup

- [ ] Remove `content_slot` pre-set `with_position(0.0, -TITLE_BAR_HEIGHT, 0.0)`
      (LayoutSystem is now the single source of truth for position)
- [ ] Remove `TITLE_BAR_HEIGHT` arithmetic from `spawn_panel_title_bar`'s avail_height
      calculation — let the measure pass handle it via StyleComponent
- [ ] Add `margin.bottom` to `header_style` for visual gap between title bar and content

---

## Horizontal layout (flex-row) — deferred

When `LayoutComponent` itself is `display: Flex; flex_direction: Row`, a
horizontal cursor is needed:

```
horizontal_cursor = 0.0

for item in items:
    horizontal_cursor += item.margin_left_gu
    content_origin_x = horizontal_cursor + item.padding_left_gu
    horizontal_cursor += item.box_width_gu + item.margin_right_gu
```

Width distribution with `flex_grow`:
```
total_fixed_w = sum of fixed-width items' margin_box_width
remaining_w   = avail_w - total_fixed_w - sum(column_gap)
auto_w_each   = remaining_w / total_grow  (weighted by flex_grow)
```

This is symmetric to the current vertical logic. Not needed for panels yet —
panels stack vertically. Workspace-level side-by-side layout would use it.

---

## Not in scope (noted for future)

- **Margin collapse** between adjacent block siblings — additive for now
- **Inline layout** (mixed text + inline elements on the same line)
- **Percent heights** when container height is unconstrained
- **min-height / max-height** clamping in measure pass
- **Absolute/fixed positioning** (out-of-flow; measured separately)
- **Intrinsic auto height** (recursing into children to sum their heights)
  — currently auto = fill remaining; content-driven auto is phase 2
