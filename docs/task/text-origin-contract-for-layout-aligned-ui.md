# Text origin contract for layout-aligned UI

## Why

The world panel list rows exposed a weak contract between layout and text rendering.

Today the layout system measures text as a rectangular block, places the styled row at its content origin, and then applies an additional half-glyph inset when positioning the inner text-bearing transform. That means the final visual result depends on a correction term in layout rather than on a clean text-origin contract.

The font atlas detail matters here: the glyph art in the atlas is already centered inside each 16x16 cell. That makes it less likely that the apparent low placement is caused by the atlas itself. The more likely issue is that the engine currently treats the text node origin as if it lives at the center of glyph cell 0 and then asks layout to compensate for that with `half_glyph_wu` offsets.

Goal: change the text origin contract so layout can align text blocks with plain box math and no half-glyph special case.

## 1. Problem statement

Current shape:

1. `TextSystem::measure` reports width/height in whole glyph-cell units.
2. `block` / `inline` layout place the styled item at its content origin.
3. `apply_text_align` then offsets the inner text wrapper by `half_glyph_wu` so the first glyph cell edge appears flush with the content-box edge.

That is the wrong ownership boundary.

Layout should not need to know that glyph instances are centered on their local origins. The text system should expose a local coordinate contract that already means "top-left of the text block".

## 2. Desired contract

New contract:

- a text-bearing transform's local origin means the top-left corner of the text block
- the first glyph cell center is spawned at `(0.5 * font_size, -0.5 * font_size)` relative to that origin
- subsequent glyphs advance by whole glyph-cell widths/heights from that block origin
- `TextSystem::measure` continues to report block width/height in the same cell-based units
- layout aligns that block origin directly inside the content box with no extra half-glyph correction

Under this contract:

- top alignment is `y = 0`
- left alignment is `x = 0`
- center alignment is half of the remaining content-space delta
- bottom alignment is the full remaining delta

This keeps padding entirely a parent-box concern and keeps text anchoring entirely a text-system concern.

## 3. Why this direction is better

### A. Single source of truth

Measurement and rendering keep using the same block geometry.

The current model splits responsibility:

- text measurement says how large the block is
- layout spends padding at the parent transform
- layout also compensates for glyph-centered children with `half_glyph_wu`

Changing the origin contract removes that last correction layer.

### B. Atlas-independent layout

Because the atlas glyphs are already centered in each 16x16 square, layout should not need atlas-specific nudges.

If a future font atlas changes artwork placement inside the cell, that should still be a text-rendering concern. Layout should only care about the cell/block bounds it is promised.

### C. Simpler alignment math

`apply_text_align` can become straightforward box alignment:

- left/top: zero offset
- center/middle: `(content - text) * 0.5`
- right/bottom: `content - text`

That is easier to reason about and easier to test.

## 4. Proposed implementation shape

### 4.1 Text system

In [src/engine/ecs/system/text_system.rs](../../src/engine/ecs/system/text_system.rs):

- keep text measurement semantics unchanged
- change glyph spawning so glyph transforms are placed relative to a block-top-left origin instead of a glyph-center origin
- the first glyph for row 0 / col 0 should be placed at half a cell inward from the block origin

The key change is conceptual:

- today the text node origin behaves like "glyph cell center space"
- after this task it should behave like "text block top-left space"

### 4.2 Layout system

In [src/engine/ecs/system/layout/block.rs](../../src/engine/ecs/system/layout/block.rs):

- remove `half_glyph_wu` from `apply_text_align`
- compute text wrapper translation from content-box dimensions and measured text-block dimensions only
- preserve the existing `Auto` behavior policy where appropriate, but make the offsets plain block-alignment offsets

If `inline.rs` depends on the same helper, it should inherit the new behavior automatically once `apply_text_align` changes.

### 4.3 Tests

Update or replace tests that currently assert the half-glyph inset behavior.

Examples:

- `auto_aligned_text_is_inset_by_half_glyph` should be replaced with a test for zero-offset top-left block anchoring
- centering tests should assert plain remaining-space centering, not centering plus half-glyph correction
- world-panel-like row coverage should assert that a single-line row with padding places the text block origin at the content-box top-left before any explicit center/middle alignment is applied

## 5. Acceptance criteria

1. Text local origin is documented and implemented as text-block top-left.
2. `apply_text_align` no longer uses a half-glyph inset correction.
3. Existing layout measurement remains consistent with rendered text bounds.
4. World-panel rows no longer look vertically low inside padded list items.
5. Centered button/title text still centers visually after the contract change.
6. No atlas-specific special case is needed for the current centered 16x16 font atlas.

## 6. Verification

1. Update focused layout tests in [src/engine/ecs/system/layout/block.rs](../../src/engine/ecs/system/layout/block.rs).
2. Run `cargo test` once unrelated background compile breakage is resolved.
3. Run `cargo run --release --example world-panel` and confirm row labels no longer read as bottom-heavy.
4. Sanity-check other examples that use `text_align("center")` / `vertical_align("middle")`, especially panel chrome in [assets/components/world_panel.mms](../../assets/components/world_panel.mms).

## 7. Non-goals

- introducing per-font ascent/descent metrics in this task
- changing atlas artwork or reauthoring the current font texture
- adding author-side MMS nudges as the primary fix

Those remain fallback options only if the origin-contract cleanup proves insufficient.

## 8. Related docs

- [docs/bugs/world-panel-row-text-appears-low-inside-padding.md](../bugs/world-panel-row-text-appears-low-inside-padding.md)