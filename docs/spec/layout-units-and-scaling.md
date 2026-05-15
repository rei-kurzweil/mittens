# Layout Units and Scaling

This doc explains the unit model shared by `LayoutComponent`, `StyleComponent`,
`TextComponent`, `LayoutSystem`, and `TextSystem`.

It is the high-level sizing contract for UI/layout trees.

For narrower rules, see also:

- [layout-block-sizing.md](./layout-block-sizing.md)
- [layout-intrinsic-text-measurement.md](./layout-intrinsic-text-measurement.md)
- [text-system.md](./text-system.md)

## Core idea

Layout is authored and measured in **glyph units**.

- `1.0` glyph unit means one monospace character cell.
- `available_width`, `available_height`, `Style.width`, `Style.height`,
  padding, and margin are all resolved in glyph units.
- `TextSystem` also lays out glyphs in glyph-local units.

Transforms are a separate concern.

- `LayoutSystem` decides box sizes and box-relative offsets in glyph units.
- `TransformComponent` decides where that subtree lands in local/world space.
- `LayoutComponent.unit_scale` is the bridge between those two spaces.

The easiest mental model is:

$$
\text{world-space layout offset} = \text{glyph-unit layout offset} \times \text{unit\_scale}
$$

If an ancestor transform also scales the subtree, that ancestor scale multiplies in
later like any other transform.

## What `available_width` means

`LayoutComponent.available_width` is the inline-axis budget for the root layout subtree,
measured in glyph units.

Examples:

- `available_width(30.0)` means the root can fit about 30 default-width glyph columns.
- `available_width(29.5)` is valid; layout uses floats throughout.

This width is the containing block width for root children.

- A block child with `width: Auto` fills the available inline content width.
- An inline-block child with `width: Auto` shrink-to-fits to its intrinsic width.
- A `%` width resolves against the containing block's content width, not against the
  child after subtracting its own padding.

Current implementation reference:

- [src/engine/ecs/system/layout/measure.rs](src/engine/ecs/system/layout/measure.rs)

The important current rule is:

- percent widths resolve against the parent content width passed into measurement
- margins reduce the max outer box width
- padding reduces the inner content width

So a `Style { width(100%) }` child under a `LayoutRoot { available_width(29.5) }`
really means “take the full 29.5 glyph-unit inline budget of the containing block”,
subject to the normal box model.

## What `available_height` means

`available_height` is an optional block-axis constraint, also in glyph units.

- `None` means unconstrained height.
- `Some(h)` is used mainly for overflow and clipping behavior.
- It does not mean “stretch children to fill this height”.

Block and inline-block auto height still follow intrinsic sizing rules.

See:

- [layout-block-sizing.md](./layout-block-sizing.md)

## What `unit_scale` means

`LayoutComponent.unit_scale` converts glyph-unit layout output into the local coordinate
system of the nearest transform space the layout subtree lives in.

It affects the transforms emitted by `LayoutSystem` for:

- item translations
- background quad transforms
- overflow helper topology
- text-alignment offsets inside styled boxes

It does not change the meaning of `available_width` or `Style.width` themselves.
Those stay in glyph units.

### Pattern A: layout subtree already lives in glyph-scaled transform space

If an ancestor `TransformComponent` already scales the whole subtree by `TEXT_SCALE`,
leave `unit_scale` at `1.0`.

Example:

```mms
T.scale(0.08, 0.08, 0.08) {
    LayoutRoot {
        available_width(30.0)
        // unit_scale omitted => 1.0
    }
}
```

In this pattern:

- layout emits local offsets in glyph units
- the ancestor transform converts the whole subtree into world space
- text, backgrounds, and child boxes all ride that same outer scale

### Pattern B: layout subtree lives directly in world/local units

If the parent transform is unscaled (`scale = 1.0`) but the layout is authored in
glyph units, set `unit_scale = TEXT_SCALE`.

Example:

```mms
LayoutRoot {
    available_width(29.5)
    unit_scale(0.08)
}
```

In this pattern:

- layout still reasons in glyph units
- emitted translations and helper geometry are scaled into world/local units
- this is the common panel pattern in the inspector/world panel code

### The common mistake

Do not apply both conversions to the same layout level unless you really want the
double scaling.

This is usually wrong:

```mms
T.scale(0.08, 0.08, 0.08) {
    LayoutRoot {
        available_width(30.0)
        unit_scale(0.08)
    }
}
```

That multiplies layout output by `0.08` twice.

## How text fits into this model

`TextSystem` builds glyphs in glyph-local units.

- default glyph advance is `1.0` per column
- rows advance by `1.0`
- glyph quads are centered on column/row positions

See:

- [text-system.md](./text-system.md)

### Text world size versus text measurement size

There are two distinct sizing knobs in the current system:

1. parent transform scale around a `TextComponent`
2. `TextComponent.font_size`

They are not identical.

#### Parent transform scale

Scaling a transform above the `TextComponent` scales the rendered glyph subtree in the
normal transform sense.

- good for placing default-size text into world space
- commonly used for UI text wrappers like `T.scale(TEXT_SCALE, TEXT_SCALE, TEXT_SCALE)`

But layout does not treat arbitrary ancestor transform scale as text measurement input.

#### `font_size`

`font_size` is text-specific sizing state.

- it changes glyph render scale
- it changes text measurement
- it changes derived wrap width in columns
- it participates in intrinsic width/height calculations

That means `font_size` is the knob layout understands when it reasons about how much
text space a box needs.

Current implementation references:

- [src/engine/ecs/component/text.rs](src/engine/ecs/component/text.rs)
- [src/engine/ecs/system/text_system.rs](src/engine/ecs/system/text_system.rs)
- [src/engine/ecs/system/layout/measure.rs](src/engine/ecs/system/layout/measure.rs)

### `authored_font_size` vs effective `font_size`

`TextComponent` keeps two related values:

- `authored_font_size`: the author-provided size
- `font_size`: the current effective size used to build glyphs

Layout may temporarily override the effective size from `Style.font_size` on the styled
container while preserving the authored value.

That allows layout-driven font-size changes to come and go without losing the original
authored size.

## How layout and text interact

The relevant order in layout is:

1. measure the item in glyph units
2. position the styled box using layout flow
3. apply effective text font size from `Style.font_size`
4. derive `wrap_at` from the resolved content width
5. apply text color helper state
6. apply text alignment inside the box

Important consequences:

- text wrap is derived from the resolved content width of the containing box
- the derived wrap width uses effective font size, not just raw character count
- text alignment offsets are box-relative layout offsets, so they must also go through
  `unit_scale`

This last point matters for panel UI:

- a centered label inside a `unit_scale(0.08)` layout root must not receive raw glyph-unit
  translations like `x = 1.5`
- it needs `x = 1.5 * 0.08` in the emitted transform for that layout level

## Current recommended authoring patterns

### Panel/layout trees

Prefer one of these two patterns and stay consistent within a subtree:

1. outer transform scales the layout subtree, `unit_scale = 1.0`
2. outer transform stays unscaled, layout root uses `unit_scale = TEXT_SCALE`

For inspector-style world-space panels, pattern 2 is the current common choice.

### UI text inside styled boxes

Today, two patterns exist:

1. wrap text in a positioned/scaled helper transform
2. use `font_size` / `Style.font_size`

Use them intentionally:

- use transform scale when you are placing default-size glyphs into world space
- use `font_size` when layout should treat the text itself as larger or smaller for
  measurement and wrapping

If you mix both, remember they multiply.

## Worked examples

### Example 1: full-width panel content

```mms
LayoutRoot {
    available_width(29.5)
    unit_scale(0.08)

    T {
        Style { width(100%) }
    }
}
```

Result:

- layout resolves the child width against `29.5` glyph units
- the child fills the containing block width in layout space
- emitted transforms and helper geometry are converted to local/world units by `0.08`

### Example 2: centered button label

```mms
T {
    Style {
        display("inline-block")
        width(6.875)
        height(2.4)
        text_align("center")
    }
    T.position(0.0, 0.0, 0.05) {
        T.scale(0.08, 0.08, 0.08) {
            Text { "Save" }
        }
    }
}
```

Result:

- the button box is measured in glyph units
- layout centers the direct text-bearing child within that box
- the centering offset is scaled by the layout root's `unit_scale`
- the nested text scale then shrinks the glyph subtree visually

### Example 3: larger text that should wrap earlier

```mms
T {
    Style {
        width(12.0)
        font_size(1.6)
    }
    Text { "inline 1.6 inline 1.6" }
}
```

Result:

- measurement uses the effective `font_size`
- wrap columns are derived from the larger glyph advance
- intrinsic width/height reflect the larger text

## Practical checklist

When a layout tree looks the wrong size, ask these in order:

1. Is `available_width` in glyph units what you think it is?
2. Is `%` width resolving against the containing block you think it is?
3. Is the subtree using exactly one glyph-units-to-world conversion path?
4. Is text size coming from transform scale, `font_size`, or both?
5. If text is centered or aligned, are those offsets happening in the same unit space as the rest of layout?

## Related references

- [src/engine/ecs/component/layout.rs](src/engine/ecs/component/layout.rs)
- [src/engine/ecs/component/text.rs](src/engine/ecs/component/text.rs)
- [src/engine/ecs/system/layout/block.rs](src/engine/ecs/system/layout/block.rs)
- [src/engine/ecs/system/layout/inline.rs](src/engine/ecs/system/layout/inline.rs)
- [src/engine/ecs/system/layout/measure.rs](src/engine/ecs/system/layout/measure.rs)
- [src/engine/ecs/system/text_system.rs](src/engine/ecs/system/text_system.rs)