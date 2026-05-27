# TextBackgroundComponent — Layout Overlap and TextLayout Design

## The Problem

`TextBackgroundComponent` specifies its padding in **glyph-space units** — the coordinate
space of the `TextComponent` before the parent `TransformComponent`'s scale is applied.
When the inspector and world panels stack rows of text, each row's `TextComponent` gets its
own background that extends `padding_top` glyph-units above and `padding_bottom` glyph-units
below the text.

### Overlap Geometry

Row positions are set in **world space** by the panel at intervals of `ROW_HEIGHT = 0.045`.
Text is rendered at `TEXT_SCALE = 0.04` (the scale on the row's parent TransformComponent).

For two adjacent rows *i* and *i+1*:

```
bottom edge of row i   = row_i.y  −  TEXT_SCALE × (0.5 + padding_bottom)
top    edge of row i+1 = row_i.y  −  ROW_HEIGHT  +  TEXT_SCALE × (0.5 + padding_top)
```

Backgrounds **overlap** when `bottom_i > top_{i+1}`:

```
ROW_HEIGHT < TEXT_SCALE × (1 + padding_top + padding_bottom)
```

With the current defaults (`padding_top = padding_bottom = 0.35`):

```
0.045 < 0.04 × (1 + 0.35 + 0.35) = 0.04 × 1.70 = 0.068  ✗  overlap
```

The maximum safe sum of vertical padding at these constants:

```
padding_top + padding_bottom  ≤  ROW_HEIGHT / TEXT_SCALE − 1
                               =  0.045 / 0.04 − 1  =  0.125  glyph-units
```

So each side can only be **≤ 0.0625** glyph-units with the current spacing — barely visible.

---

## Option A — Constrain Padding (Quick Fix)

Reduce the default `padding_top` and `padding_bottom` in `TextBackgroundComponent` to safe
values, or document that callers must not exceed `0.125 / 2` per side for a given `scale` /
`row_height` combination.

**Downside:** no overlap is only guaranteed at the current specific constants. Any other
`TEXT_SCALE` or `ROW_HEIGHT` will need recalculation. Also zero visual gap between
backgrounds, making the panel look like a solid strip.

---

## Option B — Half-Gap Clamping in the Panel

Instead of uniform padding, the panel clamps per-side:

- **First row** (top of panel): use full `padding_top`, clamp `padding_bottom` to half the
  available gap.
- **Middle rows**: clamp both `padding_top` and `padding_bottom` to half the gap.
- **Last row**: clamp `padding_top` to half the gap, use full `padding_bottom`.

The half-gap in glyph-space: `half_gap = (ROW_HEIGHT − TEXT_SCALE) / (2 × TEXT_SCALE)`.

With current constants: `(0.045 − 0.04) / 0.08 = 0.005 / 0.08 ≈ 0.0625`.

Each background then exactly touches but does not overlap its neighbour's background.

**Downside:** requires the panel to know the intended padding AND to create one
`TextBackgroundComponent` per row with different per-side values. Still tied to specific
constants.

---

## Option C — TextLayoutComponent (Recommended)

Introduce a `TextLayoutComponent` as a **parent** of the row `TransformComponent`s. It owns
the vertical spacing logic, decoupled from the fixed constants in `InspectorSystem`.

### Proposed struct

```rust
pub struct TextLayoutComponent {
    /// Vertical spacing between the bottom of one row and the top of the next,
    /// in glyph-space units (relative to the common `text_scale`).
    pub row_gap: f32,

    /// Uniform text scale applied to all rows. Used to convert between
    /// glyph-space padding and world-space positions.
    pub text_scale: f32,

    /// Whether to suppress padding_top on non-first rows and padding_bottom on
    /// non-last rows so backgrounds share a clean boundary without overlap.
    pub merge_adjacent_backgrounds: bool,

    // runtime
    row_ids: Vec<ComponentId>,      // row TransformComponent ids, in order
    built: bool,
}
```

### How it works

When `init` fires (`RegisterTextLayout` intent), `TextLayoutComponent` scans its children
for `TransformComponent + TextComponent` pairs (in child-order) and:

1. Computes the height of each row in glyph-space: `1 + padding_top + padding_bottom`
   (where `padding_*` come from the sibling `TextBackgroundComponent`, if any).
2. Places each row at a cumulative Y offset:
   `y_i = −(sum of previous row heights + row_gap × i)` in glyph-space,
   then multiplies by `text_scale` to get world-space positions.
3. If `merge_adjacent_backgrounds = true`, rewrites `TextBackgroundComponent.padding_top`
   on rows 1..N and `padding_bottom` on rows 0..N−1 to `row_gap / 2`.

This means the caller can specify `padding = 0.5` on each row, and `TextLayoutComponent`
will enforce the boundary constraint automatically.

### Panel usage

```rust
// InspectorSystem / WorldPanel row builder:

let layout = world.add_component(TextLayoutComponent {
    row_gap: 0.1,          // visible gap between rows in glyph-space
    text_scale: TEXT_SCALE,
    merge_adjacent_backgrounds: true,
    ..Default::default()
});
world.add_child(rows_anchor, layout);

for (i, line) in lines.iter().enumerate() {
    let row_t = world.add_component(TransformComponent::new()
        // x-position and indentation still set by the panel:
        .with_position(depth * INDENT_UNIT, 0.0, 0.0)
        .with_scale(TEXT_SCALE, TEXT_SCALE, TEXT_SCALE));
    world.add_child(layout, row_t);  // ← child of layout, not rows_anchor directly

    let text = world.add_component(TextComponent::new(line));
    world.add_child(row_t, text);

    let bg = world.add_component(TextBackgroundComponent::new().with_padding(0.5));
    world.add_child(text, bg);
}
world.init_component_tree(layout, emit);
```

### Indentation support

Indentation (the existing `depth * INDENT_UNIT` x-offset per row) is orthogonal to the
vertical layout; it stays on the per-row `TransformComponent.position.x`. The background
width is sized to the text content, not the full panel width, so indented rows get
appropriately-sized backgrounds.

For a "full-width" background that aligns all rows to the same right edge, a separate
`panel_width: Option<f32>` field on `TextBackgroundComponent` (in glyph-space, before
`text_scale`) could override the computed width.

---

## Near-Term Recommendation

While `TextLayoutComponent` is being designed, the panels can use Option B (half-gap
clamping) with a helper that computes safe padding per position, or simply set
`padding_top = padding_bottom = 0` (no vertical padding) and keep only horizontal padding
(`padding_left = padding_right = 0.5`). This makes each background exactly one glyph-unit
tall and flush with adjacent rows, which looks like a clean solid strip — readable while
avoiding any overlap.
