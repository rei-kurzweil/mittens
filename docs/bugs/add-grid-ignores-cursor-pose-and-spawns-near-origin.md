# Add Grid ignores cursor pose and spawns near the origin

## Summary

The `Add Grid` button does not appear to respect the current cursor position or orientation. New
grids spawn near `[0, 0, 0]` instead of at the current cursor pose.

## Current observed behavior

- move or orient the editor cursor somewhere away from the origin
- press `Add Grid`
- the new grid does not inherit the cursor translation
- the new grid does not inherit the cursor rotation
- the resulting grid appears near the origin instead

## Expected behavior

- `Add Grid` should use the current editor cursor pose as its spawn source
- the new grid root should inherit both:
  - cursor translation
  - cursor orientation
- origin fallback should happen only when no valid cursor pose exists

## Notes

- This appears consistent with the older cursor/grid spawn gap that was already being tracked.
- Related docs:
  - [docs/task/grid-visibility-and-cursor-spawn.md](../task/grid-visibility-and-cursor-spawn.md)
  - [docs/task/shared-3d-cursor-and-selection-vs-surface-placement.md](../task/shared-3d-cursor-and-selection-vs-surface-placement.md)
