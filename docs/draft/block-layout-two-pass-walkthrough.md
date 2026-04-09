# Block Layout: Two-Pass Walkthrough ( ˘ω˘ )
## Concrete algorithm for N block elements with margin + padding

Companion to `box-model-layout-system-flexbox-and-layout-root.md`.
That doc covers the full architecture; this one shows the exact arithmetic
for a **column of block elements** — the common panel case.

---

## Setup

```
LayoutComponent {
    available_width:  W      (glyph units)
    available_height: H?     (Some(H) = constrained, None = unconstrained)
    unit_scale:       S      (glyph units → world units, e.g. 0.08)
}
  TransformComponent A     ← block item 1 (title bar, fixed height)
    StyleComponent {
        display: Block
        height:  GlyphUnits(2.0)
        margin:  { top: 0, bottom: 0.25, left: 0, right: 0 }
        padding: { top: 0.25, bottom: 0.25, left: 0.5, right: 0.5 }
    }
  TransformComponent B     ← block item 2 (content, fills remaining)
    StyleComponent {
        display: Block
        height:  Auto
        margin:  { top: 0, bottom: 0, left: 0, right: 0 }
        padding: { top: 0.5, bottom: 0.5, left: 0, right: 0 }
    }
```

These are the TC nodes that `LayoutSystem` will emit `UpdateTransform` for.
Each TC's `StyleComponent` lives among its ECS children (not the TC itself).

---

## Pass 1 — Measure (bottom-up)

**Goal**: determine how much block-axis (Y) space each item needs,
including its own padding and margin. Width is resolved first; height
depends on either the explicit `height` property or the measured content.

### Width resolution (top-down, but simple for blocks)

Block elements with `width: Auto` stretch to fill the containing block:

```
A.content_width = W - A.padding.left - A.padding.right
B.content_width = W - B.padding.left - B.padding.right
```

The TC gets `scale.x = content_width * S` (or the TC width stays 1 and
content is authored to fill it — depends on implementation choice).

### Height resolution

For each item, compute `box_height` = content area + vertical padding.

```
A (height: GlyphUnits(2.0)):
  A.content_height  = 2.0
  A.box_height      = padding.top + content_height + padding.bottom
                    = 0.25 + 2.0 + 0.25
                    = 2.5 gu
  A.margin_box_height = margin.top + box_height + margin.bottom
                      = 0 + 2.5 + 0.25
                      = 2.75 gu

B (height: Auto, container H is known):
  — defer; B gets remaining space after fixed items are measured.
  B.content_height  = DEFERRED
  B.box_height      = DEFERRED
  B.margin_box_height = DEFERRED
```

### Classify items

```
fixed items:  [A]   total_fixed_margin_box = 2.75 gu
auto items:   [B]   count_auto = 1
```

### Resolve auto heights (only when container height H is known)

```
remaining = H - total_fixed_margin_box
          = H - 2.75

B.margin_box_height = remaining / count_auto   (equal share; one auto item → all of it)
B.box_height        = B.margin_box_height - B.margin.top - B.margin.bottom
                    = remaining - 0 - 0
                    = remaining
B.content_height    = B.box_height - B.padding.top - B.padding.bottom
                    = remaining - 0.5 - 0.5
                    = remaining - 1.0
```

If `H` is `None` (unconstrained height), auto items shrink to their
intrinsic content height instead — computed by recursing into children.

---

## Pass 2 — Layout (top-down)

**Goal**: emit `UpdateTransform` for each TC with the resolved position
in the parent's coordinate space.

All positions are in glyph units; the final world translation is
`position_gu * unit_scale`.

### Cursor walk

The cursor starts at the top of the containing block's content area (y = 0 gu).
The engine's +Y is up, so downward flow = decreasing Y.

```
cursor = 0.0 gu   ← start at top of content area

── Item A ──────────────────────────────────────────────────────────
  cursor += A.margin.top                     → cursor = 0.0
  A.origin_y = cursor                        → 0.0 gu   (top of box, INSIDE margin)

  [content origin = A.origin_y + A.padding.top]
  A.content_origin_y = 0.0 + 0.25           → 0.25 gu

  TC position emitted: [0, -(A.content_origin_y * S), 0]
  (TC sits at content-box origin; padding is implemented by offsetting
   child nodes relative to it, or by using TC.scale for background quads)

  cursor += A.box_height                     → cursor = 0.0 + 2.5 = 2.5
  cursor += A.margin.bottom                  → cursor = 2.5 + 0.25 = 2.75

── Item B ──────────────────────────────────────────────────────────
  cursor += B.margin.top                     → cursor = 2.75
  B.origin_y = cursor                        → 2.75 gu

  B.content_origin_y = 2.75 + B.padding.top → 2.75 + 0.5 = 3.25 gu

  TC position emitted: [0, -(3.25 * S), 0]

  cursor += B.box_height                     → cursor = 2.75 + remaining
  cursor += B.margin.bottom                  → cursor = 2.75 + remaining + 0
```

### Summary of emitted transforms

```
UpdateTransform { tc: A_id, translation: [0.0, -(0.25 * S),  0.0], scale: [1,1,1] }
UpdateTransform { tc: B_id, translation: [0.0, -(3.25 * S),  0.0], scale: [1,1,1] }
```

A is placed near the top (offset by its top padding).
B starts 3.25 gu below the top of the containing block, after A's full
margin-box height.

---

## Concrete panel example

With `H = 40 gu`, `S = 0.08`, no margin/padding (panel simplified):

```
A (title bar): content_height=2, box=2, margin_box=2   → y =  0.0 * 0.08 = 0.0
B (content):   remaining = 40-2 = 38, box=38           → y = -2.0 * 0.08 = -0.16 wu
```

With `margin.bottom=0.25` on A (a small gap):

```
A: margin_box = 2.25
B: remaining = 40 - 2.25 = 37.75
   cursor at B start = 2.25 gu → y = -2.25 * 0.08 = -0.18 wu
```

The 0.02 wu gap (≈ 1.5 px at typical VR scale) separates title bar and
content visually without any magic constants.

---

## What LayoutSystem needs to implement this

Currently `LayoutSystem::layout_flex_column` / `flex_item_style` does a
simplified version of pass 2 only, with no margin or padding support.

### Pass 1 additions

```rust
struct MeasuredItem {
    tc_id:             ComponentId,
    content_height_gu: f32,      // resolved explicit height or intrinsic
    box_height_gu:     f32,      // content + padding_top + padding_bottom
    margin_top_gu:     f32,
    margin_bottom_gu:  f32,
    margin_box_gu:     f32,      // margin_top + box + margin_bottom
    is_auto:           bool,     // height: Auto
}

fn measure_items(world: &World, layout_id: ComponentId, avail_h: Option<f32>)
    -> Vec<MeasuredItem>
```

### Pass 2 additions

```rust
fn layout_items(
    items: &[MeasuredItem],
    avail_h: Option<f32>,
    unit_scale: f32,
    emit: &mut dyn SignalEmitter,
)
```

Cursor walks items; emits `UpdateTransform` with `translation[1] = -(content_origin_gu * unit_scale)`.

### What stays unchanged

- `LayoutComponent` fields (`available_width`, `available_height`, `unit_scale`, `dirty`)
- The intent: `UpdateTransform` for each TC child
- `world.children_of(layout_id)` as the source of items
- StyleComponent lookup among TC children

---

## Margin collapse (not implemented, noted for future)

In CSS, adjacent block margins collapse to the larger of the two:

```
A.margin_bottom = 0.25 gu
B.margin_top    = 0.25 gu
collapsed gap   = max(0.25, 0.25) = 0.25 gu   (not 0.5)
```

This is NOT implemented in phase 1. Margins are additive.
Margin collapse adds significant complexity and is not needed for panels.
```
