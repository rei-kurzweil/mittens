# Free Draw paint placement does not snap to grid while Grid Tool placement does

## Summary

Grid snapping appears to work when placing grids, but not when using the `Free Draw` tool from the
`paint_panel`.

## Current observed behavior

- Grid Tool placement snaps as expected
- `Free Draw` placement appears unsnapped even when an active / visible grid is present
- this suggests the paint placement path is no longer resolving the same snap source that grid
  placement uses

## Expected behavior

- if grid snapping is active for editor paint placement, `Free Draw` should honor the same grid
  snapping contract as Grid Tool placement
- paint placement should either:
  - snap to the active grid consistently, or
  - clearly fall back to unsnapped behavior only when no valid snap grid exists

## Notes

- This may be a regression in the paint placement snap-source path rather than in the grid math
  itself.
- Related docs:
  - [paint-panel-free-draw-special-case.md](./paint-panel-free-draw-special-case.md)
  - [docs/task/shared-3d-cursor-and-selection-vs-surface-placement.md](../task/shared-3d-cursor-and-selection-vs-surface-placement.md)
