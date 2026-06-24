# Grid tool can leave a grid as the only selectable target, and dragging the grid rotates the gizmo

## Summary

After using the Grid Tool, one of the most recently created grids can become the only thing that is
selectable. In that state, dragging directly on the grid rotates the gizmo/selection target, even
though only dragging dedicated gizmo handles should transform the selected object.

## Current observed behavior

- use the Grid Tool to place one or more grids
- afterwards, a recent grid becomes the only selectable scene target, or the only reliably
  selectable one
- dragging directly on the grid surface rotates the gizmo target
- transform input appears to be armed from grid-surface dragging instead of from gizmo-handle
  dragging only

## Expected behavior

- placing a grid should not make that grid monopolize future selection
- grid surfaces may be selectable as scene objects, but they should not behave like gizmo handles
- dragging on a selected grid surface should not rotate or otherwise transform the target unless a
  gizmo handle itself was the drag hit

## Notes

- This likely overlaps with grid raycast / selectable routing and gizmo drag arming.
- Related docs:
  - [editor-cursor-3d-gltf-and-grid-alignment.md](./editor-cursor-3d-gltf-and-grid-alignment.md)
  - [docs/task/grid-panel-select-delete-hide-and-gizmo.md](../task/grid-panel-select-delete-hide-and-gizmo.md)
  - [docs/task/gizmo-drag-regression-and-lock-toggle.md](../task/gizmo-drag-regression-and-lock-toggle.md)
