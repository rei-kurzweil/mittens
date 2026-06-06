# Bug: Paint Panel Oversized Icons and Incorrect Brush Selection

## Status
- **Reproduction:** Confirmed via codebase analysis.
- **Severity:** Medium (UI polish and functional regression).
- **Assigned to:** N/A (Documentation only).

## Issue 1: Oversized Icons in `paint_panel`

### Description
The icons in the `paint_panel` (e.g., Free Draw, Line, Spray Can) appear approximately 4x larger than intended.

### Observations
- In `assets/components/panel_items.mms`, `PAINT_PANEL_ICON_SCALE` is set to `1.25`.
- The `paint_panel_item` container for the icon has a `height(4.0)`.
- The icons themselves (e.g., `pencil_icon` in `assets/components/icons.mms`) use `R.cube()` with specific scales like `(0.3, 1.2, 0.1)`.
- If `R.cube()` defaults to a unit size that is larger than expected in the current coordinate system, or if `FitBounds` was recently applied to these items, it might be causing an unintended expansion.
- Documentation in `docs/task/fit-bounds-layout-container-and-presentational-subtree.md` mentions that `FitBounds` in `paint_panel_item` caused breakage in other panels, suggesting it might be interfering with scaling logic.

### Potential Root Cause
- `FitBounds` might be scaling the icon to fill its container (4.0 height) while `PAINT_PANEL_ICON_SCALE` is also applied.
- The base size of `R.cube()` might be 2.0 (standard for some engines) rather than 1.0, leading to a 2x base size that is then scaled up.

---

## Issue 2: Incorrect Brush Selection in `editor_paint_system`

### Description
The `editor_paint_system` (or `paint_panel`) paints the entire contents of the asset panel (all assets and some text) instead of just the selected asset when in Free Draw mode.

### Observations
- `EditorPaintSystem` (in `src/engine/ecs/system/editor_paint_system.rs`) resolves the "paint brush" by looking up a `PaintAssetTemplate` using the `item` string (the title) from the `SelectionChanged` event.
- `SelectionSystem::find_selected_item_text` (in `src/engine/ecs/system/selection_system.rs`) attempts to find a child named `#selection_item_label` (ID selector).
- `asset_item.mms` defines the label with `name = "selection_item_label"` (name selector, not ID).
- Consequently, `SelectionSystem` falls back to `find_descendant_by_type(world, item_id, "text")`.
- If an asset module (like `panels.mms`) is scanned, its exports (like `asset_panel`) become available as assets. 
- If the selection resolves to the wrong component (e.g., the whole content area) or the wrong label string is picked up, `EditorPaintSystem` may spawn the wrong template.
- The user's note suggests that the system might be giving `editor_paint_system` the wrong component entirely, or that it should be using the component tree of the selected asset directly rather than spawning a new instance from a template.

### Potential Root Cause
- **Selection Mismatch:** `SelectionSystem` is likely picking up a different text component as the label because `#selection_item_label` (ID) is missing, falling back to the first available `Text` component.
- **Template Mis-selection:** If the selected item's label matches a high-level component (like `asset_panel`), the paint system spawns that entire component.
- **Component vs. Template:** There may be a conceptual mismatch between "painting a template" and "painting a selected component". If the user selects a complex component in the asset panel, they expect that specific component to be the brush.

---

## Note for Developers
- Check `assets/components/panel_items.mms` for `FitBounds` usage.
- Verify `SelectionSystem::find_selected_item_text` logic against `asset_item.mms` (change `name` to `id` or update the selector).
- Ensure `AssetSystem` filtering for `paint_templates` excludes large UI components like panels if they shouldn't be paintable assets.
- Consider if `EditorPaintSystem` should support painting a cloned subtree of the selected component when no template matches.
