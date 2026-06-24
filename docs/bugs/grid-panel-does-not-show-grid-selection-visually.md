# Grid panel does not show grid selection visually

## Summary

Grids do not appear visually selectable in the `grid_panel`, even when grid-related actions are
otherwise available.

## Current observed behavior

- grid rows can exist in the `grid_panel`
- expected visual selection/highlight state does not appear on the chosen grid row
- from the panel alone, it is not obvious which grid is currently selected / active

## Expected behavior

- selecting a grid in `grid_panel` should visibly mark that row as selected
- if the panel supports a clearable active-grid selection, the row highlight should appear and
  disappear in sync with that state
- the visual state in `grid_panel` should reflect the actual active grid used by editor tools

## Notes

- This is specifically about panel-side visual feedback, not only scene-side grid selection.
- Related docs:
  - [grid-panel-does-not-refresh-after-grid-tool-placement.md](./grid-panel-does-not-refresh-after-grid-tool-placement.md)
  - [docs/task/grid-panel-select-delete-hide-and-gizmo.md](../task/grid-panel-select-delete-hide-and-gizmo.md)
