# Shared 3D Cursor and Selection-vs-Surface Placement

Date: 2026-06-14

Status: open

## Problem statement

The editor `3D Cursor` is currently not behaving like a single shared workspace cursor.

Observed in `bisket-vr-demo`:

- arbitrary scene renderables / surfaces still do not reliably drive `3D Cursor`
- cursor behavior still appears to depend on the bisket bone-marker path
- recent placement regressions in `Grid Tool` and `Free Draw` raise the possibility that cursor placement is inheriting the same broken surface-placement math

This note narrows the design questions and records what the current code is actually doing.

## Current code behavior

### 1. `Select + Cursor` is currently surface-placement driven

Current implementation:

- `Cursor3dSystem` listens to `DragStart`
- it resolves `resolve_editor_scene_hit(world, renderable)`
- then calls:
  - `resolve_surface_placement_frame(world, target_renderable, hit_point, None)`
  - `resolve_surface_aligned_pose_for_subtree(world, target_renderable, hit_point, marker_root, None)`
- it writes the resulting pose into:
  - `EditorContextState.cursor_translation`
  - `EditorContextState.cursor_rotation`
  - `EditorContextState.cursor_frame`

Implication:

- `Select + Cursor` does **not** currently move the cursor to the selected transform/gizmo pose
- instead, it runs the same surface-hit / bounding-aware placement logic as `3D Cursor`

That is not the intended behavior for combined select+cursor interaction.

## Intended behavior

### `Select`

- selects the target transform
- moves/attaches that editor's transform gizmo
- does not move the shared 3D cursor unless explicitly requested by another action

### `3D Cursor`

- resolves a scene hit
- places the shared 3D cursor using surface-aware placement logic
- can use surface normal / tangent-frame / bounds-aware offset logic as needed

### `Select + Cursor`

- selects the target transform exactly as `Select` does
- moves the 3D cursor to exactly the selected transform / gizmo pose
- should **not** run the surface-placement solver for the cursor in this mode

Practical rule:

- in `Select + Cursor`, the cursor should land exactly where the gizmo is
- if there is any disagreement between "surface-aligned cursor placement" and "selected transform pose", `Select + Cursor` should prefer the selected transform pose

## Shared cursor requirement

There should be exactly one logical `3D Cursor` for the whole workspace.

That is separate from transform gizmos:

- transform gizmos are correctly per-editor
- the 3D cursor should be shared across editor trees

## Current topology concern

The current editor-context code still treats the cursor marker as editor-root-owned:

- `sync_editor_cursor_visual(...)` reads `state.active_editor`
- `ensure_cursor_marker(...)` creates/fetches `editor_cursor_marker` as a child of that editor root
- `Cursor3dSystem::update_editor_cursor_from_surface(...)` looks for the marker under the triggering `editor_root`

Implications:

- the visual cursor is not modeled as a single workspace-global object
- cursor existence/visibility can depend on which editor root is currently active
- this may explain why the cursor appears to "wake up" only after certain editor-root-specific interactions

## Likely relation to current placement bugs

Because `Cursor3dSystem` currently uses:

- `resolve_surface_placement_frame(...)`
- `resolve_surface_aligned_pose_for_subtree(...)`

it is coupled to the same general placement/frame-resolution path that is currently suspect for:

- `Grid Tool` vertical orientation failures
- `Free Draw` preview drift / outward jumping
- other surface-aligned placement regressions

So there are two plausible failure classes here:

1. cursor routing / ownership is wrong
2. cursor routing is fine, but the placement solver it uses is currently wrong

Both need to stay in scope.

## Open questions

1. Should pure `3D Cursor` still be surface/bounds aware?
2. In `Select + Cursor`, should the cursor always snap exactly to the selected transform origin and rotation, or should there be any optional offset mode?
3. Should the single shared cursor live under a workspace/runtime root rather than under any editor root?
4. Which system should own that shared cursor:
   - `EditorContextSystem`
   - `Cursor3dSystem`
   - or a small dedicated workspace-cursor system?
5. Do any panel flows currently assume the cursor is editor-local rather than workspace-global?

## Recommended next steps

1. Split the conceptual behavior:
   - `3D Cursor` mode uses surface-placement logic
   - `Select + Cursor` uses selected transform/gizmo pose directly
2. Refactor cursor ownership so there is one shared cursor marker for the workspace, not one marker per editor root
3. Audit all reads/writes of:
   - `active_editor`
   - `cursor_translation`
   - `cursor_rotation`
   - `cursor_frame`
   to ensure shared cursor state is not accidentally treated as editor-local visual state
4. Verify whether the current failure on arbitrary surfaces is:
   - missing scene-hit routing
   - marker ownership/visibility
   - or bad surface-frame math inherited from the broader placement regressions
