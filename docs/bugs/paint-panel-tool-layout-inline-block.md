= 🎨 Paint Panel — Tool Selection Should Be Horizontal / Inline-Block

## Problem

The paint panel's tool options are rendered as a vertical list (one row per
tool) by `spawn_panel_ui_row_tree()`, which wraps each tool in a block-level
container.

Before the MMS→DataRendererSystem switch, the tools were laid out inline with
icons via the MMS template's `paint_item` component, which used `display:
inline-block` (or equivalent) to place the tool buttons side-by-side in a
horizontal row.

## Root Cause

`rerender_paint_panel_content()` uses `render_list()` + `PAINT_TOOL_ROW_SPEC`,
which spawns each tool as a block-level `PanelUiRow`. No styling or layout
wrapper exists to make them inline.

## Desired Behaviour

Tool items should appear in a horizontal row (or wrapped grid) at the top of
the paint panel's content slot, not stacked vertically.

## Location

- `src/engine/ecs/system/editor/paint_panel.rs` — `rerender_paint_panel_content()`
- `src/engine/ecs/system/editor/panel_ui.rs` — `spawn_panel_ui_row_tree()` (always block)
- `src/engine/ecs/system/data_renderer_system.rs` — `render_list()` (no layout customisation)

## Possible Fixes

1. **Custom `render_list` variant** — Add a parameter or new method to
   `DataRendererSystem` that accepts a horizontal/inline layout mode for the
   container.
2. **Style container manually** — After `render_list()`, find the generated
   container and apply `StyleComponent { display: InlineBlock }` etc.
3. **`spawn_panel_ui_row_tree` variant** — Add a `display` option to
   `PanelUiRowSpec` so rows can be inline-block within a flex-like parent.
