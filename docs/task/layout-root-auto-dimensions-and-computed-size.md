# LayoutRoot should accept `Auto` for available width/height and expose computed size as output

Date: 2026-06-06

Status: planning only.

This is a `docs/task` note only. No `src/` changes are proposed here yet.

## Goal

1. `LayoutRoot { available_width: Auto }` — treat `SizeDimension::Auto` as an unbounded
   inline-axis budget so inline-block children advance the cursor without wrapping.
2. The computed size (`computed_size_wu`) after layout is the actual extent of laid-out content,
   **not** fed back into `available_width`.
3. Downstream systems use the computed size for bounding boxes, clip geometry, parent sizing,
   and layout-root-level shift compensation — not to constrain the same root's input.

## Problem

`SizeDimension::Auto` is the default variant of `SizeDimension` but is currently rejected
at the LayoutRoot level with a `debug_assert` (in `measure.rs` and `LayoutComponent`).
This forces every layout root to pick a concrete width up front, which means:

- You have to guess how wide the content will be before layout runs.
- The stopgap adapter multiplies the guess by 10× as a fudge factor.
- There's no way to say "just let the children determine the width naturally."

Meanwhile, `computed_size_wu` already exists on `LayoutComponent` (added alongside
`LayoutRootSizeAvailable`), but it's not yet derived from the true content extent
in the inline layout path (which returns `avail_w_gu` instead of `total_x_gu`).

## Key distinction: input vs output

`available_width` / `available_height` are **input constraints** — they tell children
how much room they have. They belong to the layout root as a *containing block*.

The computed size after layout (`computed_size_wu`) is an **output measurement** —
the actual extent of laid-out content. It does *not* need to be written back into
`available_width` for correctness. Other systems want it for:

- bounding boxes of the layout root node
- clip/stencil geometry sizing
- parent layout roots that contain this one
- Y-axis shift compensation (already wired in the stopgap adapter via `LayoutRootSizeAvailable`)

These are consumers of the computed size, not feedback into the same root's constraint.

## Auto semantics

When `LayoutRoot { available_width: Auto }`:

- The layout root passes `f32::MAX` (or sufficiently large sentinel) as the available width
  to child measurement.
- Inline-block children place on one line — no wrapping, cursor advances to the sum of
  margin-box widths.
- Block children stretch to `f32::MAX` (far off-screen). Auto width for a block child of
  an Auto LayoutRoot is probably a mistake; inline-block is the expected use case.
- After layout, `computed_size_wu` reflects the real total extent (e.g. `cursor_x_gu * unit_scale`).

When `LayoutRoot { height: Auto }`:

- No block-axis constraint.
- Height grows to contain all children.
- `computed_size_wu.1` reflects the actual height.

Nothing writes back into `available_width` or `available_height`. The constraints stay
`Auto` forever. The computed size is the public output.

## Changes needed

### 1. `measure.rs` — accept `Auto` for layout root length resolution

```rust
SizeDimension::Auto => f32::MAX,  // unbounded
SizeDimension::Percent(_) => { debug_assert!(false, "..."); 0.0 }
```

Also update `LayoutComponent::resolve_layout_length_gu` (component/layout.rs) to match.

### 2. `inline.rs:47` — return actual cursor extent, not `avail_w_gu`

```rust
// current:
(avail_w_gu, total_y_gu)

// should be:
(total_x_gu, total_y_gu)
```

`_total_x_gu` is already computed by `layout_items()` at line 36; it's just ignored.

### 3. No feedback loop

Do **not** add code that writes the computed width back into `available_width` on the same
LayoutRoot. The old `editor-workspace-width-from-post-layout-bounds.md` proposed that, but
it conflates input constraint with output measurement. The two are separate.

The existing `LayoutRootSizeAvailable` event and `computed_size_wu` field are the correct
output path. The stopgap adapter already uses `height_wu` from that event to shift the
mount transform — that is a legitimate consumer of computed size, not a constraint update.

## What this unblocks

- The shared editor workspace layout root can use `Auto` width, eliminating the fragile
  pre-layout guess and the 10× fudge multiplier.
- Panel layout (world, inspector strip, asset, paint) determines the workspace extent
  naturally through inline-block cursor advance.
- `LayoutRootSizeAvailable` carries the real measured extent to any consumer that needs it
  (Y-shift, bounding boxes, clip rects, parent layout roots).

## Acceptance criteria

- `LayoutRoot { available_width: auto }` compiles and does not assert.
- Inline-block children under an Auto LayoutRoot sit on one line with no wrapping.
- `computed_size_wu` on the LayoutRoot reflects the true total extent after layout.
- `LayoutRootSizeAvailable` event carries the correct measured width and height.
- No code feeds computed width back into the same root's `available_width`.
- The editor workspace can switch to `Auto` and panels lay out correctly without the
  10× multiplier.

## Related

- [`docs/task/layout-root-computed-size-and-shift-event.md`](./layout-root-computed-size-and-shift-event.md)
  — established `computed_size_wu` and `LayoutRootSizeAvailable` (already implemented).
- [`docs/task/editor-workspace-width-from-post-layout-bounds.md`](./editor-workspace-width-from-post-layout-bounds.md)
  — earlier proposal that (incorrectly) fed width back into `available_width`.
  Superseded by this doc for the Auto case.
- [`src/engine/ecs/system/layout/mod.rs`](../../src/engine/ecs/system/layout/mod.rs)
- [`src/engine/ecs/system/layout/inline.rs`](../../src/engine/ecs/system/layout/inline.rs)
- [`src/engine/ecs/system/layout/measure.rs`](../../src/engine/ecs/system/layout/measure.rs)
- [`src/engine/ecs/component/layout.rs`](../../src/engine/ecs/component/layout.rs)
- [`src/engine/ecs/component/style.rs`](../../src/engine/ecs/component/style.rs) — `SizeDimension::Auto`
