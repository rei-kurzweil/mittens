# Flex item padding does not expand background quads

Date: 2026-07-03

Status: root cause identified / engine fix in progress

## Summary

The original visual read was slightly wrong. The background quad math itself is already driven from
the measured padding box, but flex items with `height: auto` were being measured as if their
content height were `0`.

That means the visible result can look like "padding moved the text but did not expand the
background", when the actual failure is:

- the flex item background quad uses `box_height = padding_top + padding_bottom`
- nested text still renders below that because the item's real intrinsic content height was never
  included in measurement
- so the background appears too short even though the quad is honoring the measured box

This is specific to the flex auto-height measurement path, not the background quad placement code.

## Observed in

- [`examples/table-field-reassign.mms`](/home/rei/_/cat-engine/examples/table-field-reassign.mms:1)

The clearest repro shape was a flex-column table made of flex-row items with alternating row
background colors, where each cell/row needed padding to make the text sit correctly.

This example is currently the best visible repro for the issue.

In particular, the `table field reassignment` text near the top of the demo is a good marker:

- its top and left placement appear to respect padding
- but the text is pushed down toward the bottom edge of the colored background
- some of the glyphs appear to extend past the bottom of the background quad

That makes the padding effect easy to see: the text position changes, but the visible background
box does not fully track the padded bounds.

## Confirmed cause

Current code path:

- `sync_bg_quad(...)` in [src/engine/ecs/system/layout/block.rs](../../src/engine/ecs/system/layout/block.rs)
  sizes the quad from `box_width_gu` / `box_height_gu`
- `sync_layout_bounds(...)` in the same file stores a matching `padding_local` AABB
- the actual bad value comes earlier in
  [src/engine/ecs/system/layout/measure.rs](../../src/engine/ecs/system/layout/measure.rs)

Before the fix, the vertical auto-size branch was:

- `display: block` or `inline-block` -> use `intrinsic_block_height(...)`
- any other `display` with `height: auto` -> use `(0.0, padding_v)`

So `display:flex` items with nested text were taking the fallback branch and collapsing their
content height to zero.

## Symptoms

- row text appears to sit low or escape the colored row
- adding padding makes the mismatch easier to see
- the background quad height matches only the padding band
- glyphs render below the visible background because the flex row's intrinsic text height was
  dropped during measurement

## Expected

For a flex item with background color and padding:

- auto height should include intrinsic child content height
- the generated background quad should match that measured padding box
- text should render inside the same measured box

## Actual

Flex auto-height measurement omitted intrinsic content height for `display:flex` items, so layout
handed background generation an undersized box.

## Fix direction

Treat `display:flex` items with `height: auto` the same way block items are treated:

- measure intrinsic content height from descendant text/renderable/nested layout content
- then add vertical padding on top of that content height

That keeps background sizing code unchanged and fixes the bad measurement upstream.

## Why this matters

This shows up immediately in table/list UIs and could affect future editor-style lists even if the
current editor panels are mostly okay:

- table-like data views
- future flex-based editor rows with explicit padding and alternating backgrounds
- narrow cases like wrapped multi-line labels in settings-style panels

It makes it harder to visually center text in rows without breaking the visible block sizing.

## Validation

A targeted regression test was added in
[src/engine/ecs/system/layout/measure.rs](../../src/engine/ecs/system/layout/measure.rs) to cover
an auto-height flex row with nested text and padding.

Focused `cargo test flex_auto_height_includes_nested_text_height --lib` validation is currently
blocked by unrelated repo-wide test compile failures in other modules, primarily `RenderAssets`
mutability fixes already pending elsewhere.

## Related

- [world-panel-row-text-appears-low-inside-padding.md](/home/rei/_/cat-engine/docs/bugs/world-panel-row-text-appears-low-inside-padding.md:1)
