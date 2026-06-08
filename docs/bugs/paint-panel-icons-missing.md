= 🎨 Paint Panel — Icons Missing After MMS→DataRendererSystem Switch

## Problem

The paint panel's tool list used MMS templates (`panels.mms`) that imported icon
components from `icons.mms` (pencil, line, spray can, fill, erase) and rendered
each tool button as a styled `paint_panel_item(...)` with the icon + label.

After switching the content slot to `DataRendererSystem`, the tools are rendered
via `PAINT_TOOL_ROW_SPEC` → `paint_tool_row_render_fn()` which calls
`spawn_panel_ui_row_tree(...)` — a plain text row with no icon.

The result: tool buttons are plain text lines instead of icon+label buttons.

## Root Cause

`rerender_paint_panel_content()` in `editor/paint_panel.rs` uses
`spawn_panel_ui_row_tree()` from `editor/panel_ui.rs`, which only produces a
text label. No icon child is attached.

## Desired Behaviour

Each tool row should include its corresponding icon (pencil, line, spray can,
fill, erase) from `icons.mms` or a Rust-side equivalent.

## Location

- `src/engine/ecs/system/editor/paint_panel.rs` — `rerender_paint_panel_content()`, `paint_tool_row_render_fn()`
- `src/engine/ecs/system/editor/panel_ui.rs` — `spawn_panel_ui_row_tree()` (needs icon support)
- `assets/components/icons.mms` — icon definitions (currently only consumed by MMS)

## Possible Fixes

1. **MMS hybrid** — Keep the icon row as an MMS template that `paint_tool_row_render_fn()` calls via `MeowMeowRunner::materialize_mms_module_component_from_file()`.
2. **Rust-side icons** — Add icon assets (textures, SDF glyphs, or baked geometry) that the Rust renderer can spawn alongside the label.
3. **Extend `PanelUiRowSpec`** — Add an optional `icon_name: Option<&'static str>` field and have `spawn_panel_ui_row_tree()` load+attach the icon from `icons.mms` when set.
