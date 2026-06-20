# Grid Panel Select Delete Hide and Gizmo

Date: 2026-06-14

Status: open

Related:

- `docs/task/grid-panel-and-grid-inspector.md`
- `docs/task/grid-visibility-and-cursor-spawn.md`

## Goal

Make `grid_panel` behave like a normal editor object-management panel for grids:

- selecting a grid should select its owning transform
- that selection should place the normal transform gizmo on the grid transform
- hide/show should work from the panel
- enable/disable should work from the panel
- delete should work from the panel

This task is intentionally narrower than the broader `grid-panel-and-grid-inspector` note.

## Current problems

Known or recently re-verified behavior:

- grid panel selection is not yet reliable as a normal editor selection path
- deleting a grid from `grid_panel` has been freezing or otherwise failing
- hide/show behavior is incomplete
- enable/disable behavior is not yet modeled as distinct from hide/show
- a selected grid should behave like other editor-scene selections, but does not yet consistently attach the gizmo

## Intended behavior

### Select

Clicking a grid row in `grid_panel` should:

- resolve the grid's owning transform
- set normal editor selection to that transform
- update `EditorComponent.selected`
- attach/move the transform gizmo to that transform

The selected semantic target for panel purposes can still be the grid entry, but the scene/editor target should be the transform root that owns the grid.

### Hide / Show

Clicking the visibility control should:

- toggle only the grid's visible state
- rerender the panel row immediately
- update the scene render path without requiring unrelated interaction

This should not be the same thing as disabling the grid runtime entry.

### Enable / Disable

Clicking the enabled control should:

- disable:
  - remove the live grid from the world/runtime/BVH
  - preserve the stored grid state so it can be re-enabled later
- enable:
  - recreate or reattach the live grid from the stored grid state
- rerender the panel row immediately
- update hit-testing/snapping/render participation without requiring unrelated interaction

### Delete

Clicking delete should:

- remove the entire owning grid subtree
- clear selection if the deleted grid was selected
- rerender the grid list immediately
- avoid world-panel refresh paths that are already known to freeze

## Required topology rule

Every grid row needs a stable mapping:

- row -> grid component
- grid component -> owning transform

That mapping should be explicit and reusable, not reconstructed ad hoc in each button handler.

Recommended helpers:

- `grid_owner_transform(world, grid_component) -> Option<ComponentId>`
- `grid_component_under_transform(world, transform) -> Option<ComponentId>`

## Gizmo contract

This task should follow the existing editor selection path rather than inventing a grid-specific gizmo path.

Practical rule:

- `grid_panel` selection should call into the same editor-selection mechanism used for ordinary scene objects

If the selected transform is the grid owner, the existing gizmo attach behavior should do the rest.

## Suggested implementation shape

1. Keep repeated grid rows in the Rust-side grid panel renderer.
2. Add row actions for:
   - select
   - visibility toggle
   - enable/disable toggle
   - delete
3. Ensure row-body select resolves the transform owner, not just the leaf grid component.
4. Reuse existing editor selection/gizmo attachment flow.
5. Keep rerenders scoped to `grid_panel` until broader panel-refresh freezing is solved elsewhere.

## Open questions

1. Should hidden grids still participate in snapping, or should snap eligibility require both enabled and shown?
2. Should deleting the selected grid move selection back to the editor root, or clear it entirely?
3. Should clicking hide/show or enable/disable preserve current selection?

## Acceptance

- selecting a grid row places the transform gizmo on the grid's transform
- toggling hide/show updates both panel state and scene visibility
- toggling enable/disable removes or restores live world/BVH participation without losing stored grid state
- deleting a grid removes its subtree and updates the panel immediately
- these flows do not require switching to another panel to recover correct editor behavior
