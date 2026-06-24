# Intermittent gizmo selection path only hits the small block under the XR rig

## Summary

Selection sometimes regresses into a state where almost nothing in the scene can be selected while
the transform gizmo is present. The only reliably selectable object in that state appears to be the
small horizontal block under the XR rig, not the ground cube or the other ordinary scene objects.

This is intermittent. The exact trigger is not yet known.

## Current observed behavior

- transform gizmo is visible / attached as usual
- ordinary scene objects stop responding to expected selection clicks
- the small horizontal block under the XR rig remains selectable
- the larger ground cube does not appear to be the surviving selectable target
- the issue can return after previously being fixed

## Expected behavior

- scene selection should continue to work normally while gizmos are visible
- gizmo presence should not narrow selection to one unrelated fallback object
- whichever object is under the pointer and is otherwise selectable should be eligible for
  selection

## Notes

- This may overlap with:
  - [gizmo-clicks-lose-to-scene-geometry-behind-selected-object.md](./gizmo-clicks-lose-to-scene-geometry-behind-selected-object.md)
  - [docs/task/gizmo-drag-regression-and-lock-toggle.md](../task/gizmo-drag-regression-and-lock-toggle.md)
- Because the repro is intermittent, any future fix should log the last selected target, active
  gizmo target, and the raycast winner when the failure occurs.
