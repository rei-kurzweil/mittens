= 🎨 Paint Panel — Free Draw Hardcoded as Special Case

## Problem

Several places in the codebase treat `PaintTool::FreeDraw` as a special case
instead of treating all tools uniformly as branches of the `Selection` with an
`OptionComponent` + `DataComponent` that identifies which tool is selected.

### Locations

1. **`is_paint_active()`** (`editor/paint_panel.rs`) checks
   `paint_state.selected_tool == PaintTool::FreeDraw` — this bakes in the
   assumption that only FreeDraw can be active.

2. **`rerender_paint_panel_content()`** (`editor/paint_panel.rs`) hardcodes
   the initial selection to Free Draw via
   `SelectionEntry { index: Some(0), item: Some(FREE_DRAW_LABEL), ... }`.

3. **`reduce_paint_state()`** (`editor/paint_panel.rs`) handles
   `PaintEvent::ToolSelectionChanged` but doesn't use the selected item's
   payload to derive the tool — it relies on the `PaintTool` enum directly.

## Root Cause

The old MMS template had `paint_panel_item(...)` calls that each carried a
label and an icon, but the tool identity was implicit in the label text. The
DataRendererSystem migration preserved this label-based approach rather than
giving each tool a `DataComponent` payload that directly encodes the tool kind.

## Desired Behaviour

Each tool row should be tagged with an `OptionComponent` and a `DataComponent`
containing a key like `"tool"` → `DataValue::Text("FreeDraw")` (or `"Line"`,
`"SprayCan"`, etc.). The selection system then resolves which tool is active
from the `SelectionComponent.selected_payload` rather than via a hardcoded
enum match on a label string.

## Location

- `src/engine/ecs/system/editor/paint_panel.rs` — `is_paint_active()`,
  `rerender_paint_panel_content()`, `paint_tool_row_render_fn()`
- `src/engine/ecs/system/editor_paint_system.rs` — reads `PaintState.selected_tool`

## Possible Fixes

1. **Add `OptionComponent` + `DataComponent` payload** to each tool row in
   `paint_tool_row_render_fn()` with a `"tool"` key carrying the tool name.
2. **Use `SelectionComponent.selected_payload`** to derive the `PaintTool`
   instead of matching on label strings.
3. **Remove the `PaintTool::FreeDraw` special case** from `is_paint_active()`
   — check `selected_payload.is_some()` instead (any active tool suffices).

## Related Task

`docs/task/selection-entry-payload-refactor.md` — once `SelectionEntry`
carries `payload: Option<ComponentId>` directly, the paint panel can rely
on per-entry payloads instead of the single `selected_payload` field.
