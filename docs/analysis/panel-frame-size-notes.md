# World/Inspector Panel Frame Size Notes

Date: 2026-05-04

## Scope

This note only traces the current sizing/positioning path for the world panel and
inspector panel. No `src/` changes were made.

## Shared construction path

Both panels use the same frame builder in
`src/engine/ecs/system/inspector_system.rs`:

- `spawn_world_panel(...)` calls `spawn_panel_title_bar(...)` and then adds a styled
  `content_slot`.
- `spawn_inspector_panel(...)` does the same.

Relevant lines:

- `spawn_panel_title_bar(...)`: `src/engine/ecs/system/inspector_system.rs:170`
- world `content_slot`: `src/engine/ecs/system/inspector_system.rs:310`
- inspector `content_slot`: `src/engine/ecs/system/inspector_system.rs:399`

So if the header/background frame is oversized in both panels, the root cause is
almost certainly in the shared layout/background path, not the row content.

## What looks wrong

The symptom matches this split exactly:

- panel text content looks normal
- header background looks huge
- content background looks huge

That points to a transform-scale mismatch between:

- the text subtree, which is explicitly scaled by `TEXT_SCALE = 0.08`
- the layout-owned background quads, which are sized in glyph units

## Current sizing path

`spawn_panel_title_bar(...)` creates:

- `panel_t` with scale `1.0`
- `panel_layout` with `unit_scale = TEXT_SCALE`
- `header_slot` as a plain `TransformComponent::new()` with scale `1.0`

Then `spawn_world_panel(...)` / `spawn_inspector_panel(...)` create:

- `content_slot` as a plain `TransformComponent::new().with_position(...)`
- `content_style.background_color = Some(...)`
- `content_style.overflow = Overflow::Scroll`

This means the layout items that own the frame backgrounds are:

- `header_slot`
- `content_slot`

and both currently keep local scale `1.0`.

## Why the backgrounds become huge

`LayoutSystem` uses `unit_scale` only when positioning layout items:

- `block::layout` emits item translation as `content_origin_* * unit_scale`
- but it preserves each item's existing transform scale

Code:

- `src/engine/ecs/system/layout/block.rs:65`
- `src/engine/ecs/system/layout/block.rs:71`

At the same time, `sync_bg_quad(...)` sizes the `__bg` helper directly in glyph units:

- translation uses raw `box_width_gu` / `box_height_gu`
- scale is `[box_width_gu, box_height_gu, 1.0]`

Code:

- `src/engine/ecs/system/layout/block.rs:269`
- `src/engine/ecs/system/layout/block.rs:307`

That helper also documents the assumption that the item TC is already scaled to
roughly `TEXT_SCALE`:

- `src/engine/ecs/system/layout/block.rs:267`

But `header_slot` and `content_slot` are not scaled that way; they stay at `1.0`.

## Concrete numbers

Using the current constants:

- `TextComponent::DEFAULT_WRAP_AT = 40`
- `CHAR_WIDTH_GLYPH = 0.55`
- `TEXT_SCALE = 0.08`

Estimated panel widths:

- inspector panel width: `40 * 0.55 * 0.08 = 1.76` world units
- world panel width: `1.76 + (5 * 0.12) = 2.36` world units

But the layout root stores widths in glyph units:

- inspector `avail_width_gu = 1.76 / 0.08 = 22`
- world `avail_width_gu = 2.36 / 0.08 = 29.5`

So the generated `__bg` quads for `header_slot` / `content_slot` are effectively
scaled to about:

- `22` world units wide for inspector
- `29.5` world units wide for world panel

if their parent item transform remains at scale `1.0`.

That exactly explains:

- normal-looking text, because text children are scaled by `0.08`
- enormous frame rectangles, because background quads are left in glyph-space size

## Spec vs implementation mismatch

The background-color refactor note says the background quad should be scaled by
`unit_scale`:

- `docs/refactor/style-background-color.md:77`

But the current implementation in `block.rs` does not multiply the background quad
transform by `unit_scale`; it writes raw glyph-unit scale into `__bg`.

So there is already a documented expectation that differs from the code.

## Working conclusion

The oversized world/inspector panel header and content backgrounds are most likely
caused by a shared coordinate-space mismatch:

- layout item positions are converted from glyph units to world units via
  `LayoutComponent.unit_scale`
- layout item scales are preserved unchanged
- layout-owned `__bg` helpers are sized in glyph units
- `header_slot` and `content_slot` use scale `1.0` instead of `TEXT_SCALE`

The panel contents still look normal because their text/row transforms explicitly
apply `TEXT_SCALE`, so only the frame backgrounds are obviously wrong.

## Follow-up options for later

If we decide to fix it later, the two main directions appear to be:

- make panel layout items that rely on layout-owned backgrounds live in glyph-space
  by giving `header_slot` / `content_slot` an item scale of `TEXT_SCALE`
- or make `sync_bg_quad(...)` apply `unit_scale` itself instead of assuming the
  parent item transform already does

The second option is broader and would affect all layout-owned backgrounds, not just
these panels.
