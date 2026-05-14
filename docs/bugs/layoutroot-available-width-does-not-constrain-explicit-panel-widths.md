# LayoutRoot available width does not constrain explicit child widths in panel chrome

## Status

Open bug / investigation note.

No source changes made yet.

## Symptom

In the MMS world-panel repro, changing the layout root width does not make the visible panel narrower.

Current repro state in [assets/components/world-panel.mms](../../assets/components/world-panel.mms):

- `WORLD_PANEL_WIDTH_GU = 9.5`
- the panel still renders much wider than that
- the save/load buttons stay positioned relative to the title label as if the old wider title-row budget still exists

From user repro notes:

- setting `WORLD_PANEL_WIDTH_GU` to `19` or `9` looks the same
- buttons are now in the correct local relationship to the `World` label
- the overall panel chrome is still far too wide

## Repro

- [assets/components/world-panel.mms](../../assets/components/world-panel.mms)
- [examples/world-panel.mms](../../examples/world-panel.mms)
- [examples/world-panel.rs](../../examples/world-panel.rs)

Run:

```bash
cargo run --release --example world-panel
```

Then vary `WORLD_PANEL_WIDTH_GU` in [assets/components/world-panel.mms](../../assets/components/world-panel.mms) between values like `29.5`, `19`, and `9.5`.

## Expected behavior

When a panel root sets:

- `LayoutRoot.available_width(...)`

that width should be the effective inline-axis budget for the visible panel chrome.

At minimum, shrinking the root width should visibly shrink the panel body and title bar.

If authored child widths exceed the root budget, the layout system should choose a coherent constraint behavior instead of silently preserving a wider visible panel:

- clamp child box widths to the available width
- or wrap/reflow them in a way that makes the narrower root visible
- or clip/overflow them while keeping the panel container itself narrow

## Actual behavior

Today, explicit child widths remain authoritative even when they already exceed the layout root width.

That is consistent with the current measurement rule in [src/engine/ecs/system/layout/measure.rs](../../src/engine/ecs/system/layout/measure.rs):

- explicit `width(...)` is treated as the outer box width
- inline-block items with explicit width keep that width

Relevant test coverage already documents that behavior:

- [src/engine/ecs/system/layout/measure.rs](../../src/engine/ecs/system/layout/measure.rs#L857) `inline_block_with_explicit_width_keeps_that_width`

The title row in the repro currently hard-codes widths that already exceed the root width:

- title label width `14.5`
- save button width `6.875`
- load button width `6.875`
- plus button margins

So even with `WORLD_PANEL_WIDTH_GU = 9.5`, the authored title-row children still ask for substantially more width than the root budget.

The inline layout code in [src/engine/ecs/system/layout/inline.rs](../../src/engine/ecs/system/layout/inline.rs) uses `avail_w_gu` as a wrapping budget, but it does not clamp explicit-width items before placement.

## Likely root cause

`LayoutComponent.available_width` currently behaves more like:

- a measurement/wrapping budget

than like:

- a hard constraint on descendant visual width

That is fine for auto-width text wrapping, but it breaks the mental model needed for authored panel chrome where the layout root is supposed to define the panel's visible width.

In particular:

- block items with auto width fill the available width
- inline-block items with explicit width preserve that explicit width
- no higher-level pass appears to clamp or clip explicit-width descendants back to the layout-root width

## Why this matters

For MMS-authored panels, `LayoutRoot.available_width(...)` needs to be a trustworthy top-level panel sizing control.

Without that:

- authored panel modules cannot be scaled by changing one root constant
- title bars and other chrome must be manually re-derived from internal child widths
- future Rust-side panel integration cannot rely on the MMS panel root as the single source of truth for panel width

## Open questions

- Should `LayoutRoot.available_width` hard-constrain descendant width, or only the immediate block container width?
- If explicit-width inline-block children exceed the root width, should they wrap, clamp, overflow, or be clipped?
- Should panel-style containers get a stricter width model than generic layout roots?
- Is the too-wide visible panel coming entirely from descendant overflow, or is some generated background/stencil geometry also preserving an older wider width?

## Relevant files

- [assets/components/world-panel.mms](../../assets/components/world-panel.mms)
- [examples/world-panel.mms](../../examples/world-panel.mms)
- [examples/world-panel.rs](../../examples/world-panel.rs)
- [src/engine/ecs/system/layout/mod.rs](../../src/engine/ecs/system/layout/mod.rs)
- [src/engine/ecs/system/layout/inline.rs](../../src/engine/ecs/system/layout/inline.rs)
- [src/engine/ecs/system/layout/measure.rs](../../src/engine/ecs/system/layout/measure.rs)