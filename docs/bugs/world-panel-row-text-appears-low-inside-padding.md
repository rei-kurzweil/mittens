# World panel row text appears low inside padded list items

## Status

Open bug / investigation note.

## Symptom

In the world panel content list, the component-name rows look like their text is rendering down in the lower padding band rather than sitting visually between the top and bottom padding.

The clearest repro is the list rows authored in [assets/components/world_panel_content.mms](../../assets/components/world_panel_content.mms):

- each row is a styled block
- each row has `padding_xy(0.55, 0.45)`
- each row has `font_size(1)`
- the label is authored as a child `T { Text { ... } }`

Visually, the row background and overall height look correct, but the glyphs themselves appear too low.

## Expected behavior

The visible text should sit comfortably inside the row's content area, with the top and bottom padding reading as balanced.

More concretely:

- the row transform should honor padding before placing text
- the text wrapper should align inside the row's content box, not the padding box
- the visible letterforms should not look bottom-heavy inside a single-line padded row

## Current repro

Primary repro:

- [examples/world-panel.mms](../../examples/world-panel.mms)
- [assets/components/world_panel.mms](../../assets/components/world_panel.mms)
- [assets/components/world_panel_content.mms](../../assets/components/world_panel_content.mms)

The issue is most obvious in the scrollable list portion of the panel where each item name is rendered inside a pale green row.

## What currently looks correct

The box-model math itself does not currently look obviously wrong.

Observed code path:

- block layout places each styled item transform at `margin + padding`, i.e. at the content origin, in [src/engine/ecs/system/layout/block.rs](../../src/engine/ecs/system/layout/block.rs)
- the row's auto height is measured from descendant text intrinsic height in [src/engine/ecs/system/layout/measure.rs](../../src/engine/ecs/system/layout/measure.rs)
- there is already a regression test asserting that a single-line styled row with `font_size = 1gu` gets `content_height_gu = 1.0`, in [src/engine/ecs/system/layout/block.rs](../../src/engine/ecs/system/layout/block.rs)

That means the old mixed-unit bug where text physically spilled into padding does not appear to be the active issue here.

## What looks out of place

The suspicious part is the split between measurement, parent placement, and final text-child placement.

Current data flow:

1. The row's content height is measured from descendant text metrics.
2. The row `TransformComponent` is then moved to the content origin, so padding is already "spent" at the parent level.
3. Later in the same layout pass, `apply_text_font_size_for_item`, `apply_text_wrap_for_item`, and `apply_text_align` mutate the descendant text subtree.
4. `apply_text_align` computes a half-glyph inset from the content-box top-left and emits a transform update for the inner text-bearing child.

That means padding and text placement are handled in two different places:

- padding affects the parent styled item transform
- text alignment affects the child transform inside that already-shifted parent

This is internally consistent, but it makes the visual result depend on whether the text system's measured cell bounds actually match the visible glyph art.

## Likely cause

The likely mismatch is not "padding ignored" but "text aligned to glyph cell metrics rather than visible glyph ink/baseline".

Relevant details:

- `TextSystem::measure` reports text height as whole rows of `font_size`, in [src/engine/ecs/system/text_system.rs](../../src/engine/ecs/system/text_system.rs)
- glyphs are spawned at row/column origins and scaled by `font_size`, also in [src/engine/ecs/system/text_system.rs](../../src/engine/ecs/system/text_system.rs)
- `apply_text_align` uses `half_glyph_wu = font_size_wu * 0.5` and places the text wrapper at `y = -half_glyph_wu` for top/auto alignment, in [src/engine/ecs/system/layout/block.rs](../../src/engine/ecs/system/layout/block.rs)

So layout is aligning the text block as if the full glyph cell is the visual content. If the atlas glyphs are baseline-biased inside those cells, the letters will read low even though the cell itself is correctly inside the content box.

## Why timing / flow feels suspicious

Even if the root cause is font-metric related, the timing still deserves scrutiny because measurement and final visual placement are not driven from one single source of truth.

Specifically:

- measurement uses text intrinsic size derived from `TextSystem::measure`
- row placement consumes padding before text alignment runs
- final child placement is applied afterward by `apply_text_align`

This split is safe only if all three stages agree on what the top of the rendered text actually means.

Right now they appear to agree on glyph-cell bounds, not on visible letterform bounds.

## Investigation targets

- [src/engine/ecs/system/layout/block.rs](../../src/engine/ecs/system/layout/block.rs)
- [src/engine/ecs/system/layout/measure.rs](../../src/engine/ecs/system/layout/measure.rs)
- [src/engine/ecs/system/text_system.rs](../../src/engine/ecs/system/text_system.rs)
- [assets/components/world_panel_content.mms](../../assets/components/world_panel_content.mms)

Questions to answer:

- does the row content box visualization show the text wrapper fully inside the content area while the glyph art still looks low?
- is `apply_text_align` intentionally aligning to glyph-cell edges rather than font ascent/descent?
- does the font atlas itself contain extra empty space above glyphs that makes cell-centered alignment look bottom-heavy?
- should text alignment use visible font metrics instead of the current half-glyph inset rule?

## Likely fix direction

Most likely engine-side fix:

- make text vertical alignment derive from explicit font metrics or resolved ink bounds rather than the current generic half-glyph inset

Possible shorter-term workaround:

- author a small upward offset on the row label wrapper in MMS for panel rows that use this font atlas

## Validation notes

Focused Rust test execution was blocked during this investigation by unrelated compile failures in [src/meow_meow/tests.rs](../../src/meow_meow/tests.rs) while other work was in progress.

That did not block static inspection of the current layout/text code path, but it did block adding or running a narrow regression test right now.