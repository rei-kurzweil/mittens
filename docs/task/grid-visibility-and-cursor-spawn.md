# Grid visibility and cursor-based spawn

Date: 2026-06-11

Status: open

## Goal

Track the remaining work needed to make editor-created grids visible and to spawn them from the editor's effective 3D cursor pose instead of always at the editor root origin.

## Current state

- `Add Grid` creates:
  - `grid_N` transform
  - `grid_N_component`
  - `grid_visual` helper subtree
- The new grid is parented directly under the active editor root.
- The spawned `grid_N` transform currently uses `TransformComponent::new()`.

Implication:
- grids currently spawn at translation `[0, 0, 0]`
- grids currently spawn with identity rotation
- grids do not yet use the active transform gizmo pose as a spawn cursor

## Known bugs

- Grid visuals still do not appear reliably in the scene even though the authored subtree is present.
- Deleting a grid from `grid_panel` freezes the editor.
- Refreshing the world panel from the grid add path freezes the editor.
- Editor panel setup leaks phantom panel/content roots into the authored world root.

## Questions to resolve

### 1. What should count as the editor's 3D cursor?

Recommended answer:
- treat the active editor transform gizmo pose as the cursor pose
- if the gizmo is attached to a selected transform, spawn the grid root at that transform's world translation and rotation
- if no gizmo/selection is active yet, fall back to the editor root origin

Open detail:
- decide whether spawn should inherit full rotation always, or only align to the gizmo's plane-relevant axes

### 2. Why is the grid invisible?

Likely buckets:
- the new `GRID_MESH` material path is not actually what reaches `VisualWorld` / renderer at draw time
- the helper subtree is registered, but its transform / draw category keeps it out of the expected scene pass
- the shader output is valid but effectively invisible because of depth, fade, or other render-state assumptions

## Next debugging steps

1. Confirm the registered `VisualWorld` instance for `grid_visual_renderable`:
   - mesh handle
   - material handle
   - world transform
   - opacity / background / overlay flags

2. Isolate material-vs-placement failure:
   - temporarily force the helper visual to `UNLIT_MESH`
   - if that appears, the failure is in `GRID_MESH` pipeline/shader path
   - if that still does not appear, the failure is in transform/registration/pass placement

3. Once visibility is restored, change grid spawning to use the active gizmo pose:
   - resolve the active editor gizmo or selected target transform
   - copy translation + rotation into the new `grid_N` root transform
   - keep scale at identity

## Acceptance

- adding a grid makes a visible scene object appear immediately
- that object is authored under the active editor root
- its root transform spawns at the active gizmo/cursor translation and rotation
- add-grid still rerenders only the grid panel and does not reintroduce the world-panel freeze
