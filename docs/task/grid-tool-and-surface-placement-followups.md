# Grid Tool and Surface Placement Follow-ups

Date: 2026-06-14

Status: open

Related:

- `docs/task/shared-3d-cursor-and-selection-vs-surface-placement.md`

## Preface: fix cursor observability first

Before spending more time on `paint_panel` / `Grid Tool` placement math, get `3D Cursor` working reliably and visibly across the same editor roots and scene surfaces.

Reason:

- right now the cursor is the easiest interactive probe we have for the resolved placement frame
- if cursor placement is driven by the same or related surface-frame logic, it can show:
  - which direction the system thinks the surface normal points
  - which way the fitted tangent/binormal basis is oriented
  - whether a floor/wall surface is being interpreted as "up", "forward", or something else
- that makes it much easier to distinguish:
  - wrong hit routing
  - wrong surface-normal extraction
  - wrong tangent-frame construction
  - wrong asset/grid local-plane assumptions

Concrete debugging approach:

- make `3D Cursor` visible and functional without requiring the current bisket bone-selection workaround
- click the same back wall / ground box / other failing surfaces with the cursor
- compare cursor orientation against:
  - `Free Draw` preview orientation
  - `Grid Tool` preview orientation
  - expected surface-aligned behavior
- use that comparison to decide whether the main bug is:
  - shared frame resolution
  - paint/grid-specific pose application
  - or grid visual basis mismatch

## Latest verification notes

Verified again on 2026-06-14 after an initial stabilization pass:

- `Free Draw` preview is somewhat more stable at close range than before, but it is still incorrect on some surfaces.
- On the back wall:
  - the preview appears in the expected place for roughly one frame at drag start / first drag move
  - it then jumps outward away from the wall by a large distance
  - the jump size looks correlated with the target wall's largest dimension rather than the painted asset's size
  - the same outward jump happens from either side of the wall
- `Grid Tool` on the ground box still produces a grid that goes straight up rather than lying on the ground surface.
- The current grid visual also lacks the expected horizontal stripe/readable plane appearance once misoriented, which makes the result clearly unusable even if the math were otherwise "consistent".
- Preview transparency still remains on committed painted component trees.
- `3D Cursor` still appears absent/invisible until entering `Select` mode and clicking one of bisket's armature bone markers.
- After that "wake up" step:
  - `3D Cursor` works, but only for bisket
  - `Select + Cursor` also works, but only for bisket

These observations narrow several issues from "general instability" to more specific transform-frame, grid-basis, and editor-root-routing failures.

## Summary

The editor now has two distinct grid placement paths:

- `Add Grid` in `grid_panel` uses cursor-based placement
- `Grid Tool` in `paint_panel` uses preview-driven surface placement

That split is correct, but there are still several unresolved issues in the preview, snapping, routing, and panel-refresh paths.

This note tracks those follow-ups in one place.

## Current intended behavior

- `grid_panel`:
  - `Add Grid` should create a grid immediately from the effective cursor / cursor-backed spawn pose.
  - It should remain the cursor-style, non-preview path.
- `paint_panel`:
  - `Grid Tool` should create a preview grid on drag start, update it while dragging over valid scene hits, and commit the same object on drag end.
  - `Free Draw` should keep doing the same preview/commit pattern for non-grid object placement.

## Open issues

### 1. Grid Tool placement is not refreshing the grid list

Observed behavior:

- placing a grid through `paint_panel` does not reliably update the `grid_panel` list after commit
- this suggests either:
  - the placed grid is not being seen by `GridSystem` registry helpers after commit, or
  - the grid is registered correctly but `grid_panel` is not rerendered on drag end

Expected behavior:

- when `Grid Tool` placement commits on drag end:
  - the grid should be visible in `GridSystem::enumerate_grids_for_editor(...)`
  - the `grid_panel` content should rerender immediately
  - the newly placed grid should appear in the list without requiring unrelated editor interaction

Likely work:

- audit whether preview-created grid subtrees become ordinary registered grid subtrees after preview markers are removed
- confirm `GridSystem` dirtying / registry refresh is triggered by preview attach/commit flow
- rerender `grid_panel` on preview commit, not only on direct `grid_panel` button paths

### 2. Preview opacity is not being removed on commit

Observed behavior:

- previewed objects sometimes remain semi-transparent after placement
- this still happens after an attempted recursive preview-marker cleanup pass
- that strongly suggests the bug is not just "we forgot to remove one preview marker child"

Expected behavior:

- preview-only opacity should be attached only for the preview phase
- on commit:
  - the preview root should return to normal opacity
  - placed objects should become fully opaque
- on cancel:
  - the preview object should be removed entirely

Likely work:

- audit `object_placement_preview.rs`
- confirm where the visible opacity is actually coming from:
  - preview-only helper nodes
  - authored asset opacity descendants
  - inherited opacity propagation cached elsewhere
  - or a render-side/material-side preview flag that is not topology-local
- do not assume recursive topology scanning at commit is the right fix; the failed workaround suggests the ownership model is wrong earlier in the preview lifecycle
- add explicit tests for both paint assets and grid previews

### 3. Free Draw preview drifts or diverges while dragging

Observed behavior:

- non-grid object placement with `Free Draw` sometimes starts correct
- while dragging, the preview can drift into what looks like:
  - a sinusoidal pattern
  - a divergent series
  - or repeated incorrect transforms relative to the actual drag hit point
- the effect is especially visible while dragging over a grid, where the preview can separate from the pointer path
- a more specific reproduction now exists on the back wall:
  - initial placement is briefly correct
  - then the preview jumps outward from the wall by a large distance
  - the magnitude appears tied to the wall's size, not just the placed asset bounds
- because the first frame is roughly correct, the bug likely happens in the drag-update path rather than the initial preview creation path

Expected behavior:

- the preview should stay anchored to the current resolved placement frame
- dragging over a surface should update the preview in a stable one-to-one way from the latest hit
- grid snapping, if active, should move the preview to snapped positions only, without accumulating drift

Likely work:

- inspect whether preview transform updates are applying an already-offset pose repeatedly
- confirm the preview root transform is updated directly from world-space placement, not from compounded local transforms
- audit whether subtree bounds / min-z offsets are being reapplied against already-moved preview roots
- audit whether target-surface bounds or target local-space extents are accidentally being mixed into placement offset math
- confirm grid snap and surface-frame resolution are using the current hit point each frame rather than transformed prior state

### 4. Snapping behavior is not yet clearly defined in the UI

Observed behavior:

- snapping appears to work sometimes
- it is not yet obvious what turns it on or off

Questions that need explicit answers:

- is snapping active whenever an authored grid exists?
- only when a grid is selected?
- only when dragging over a grid?
- only when the editor has an active grid via `GridSystem::active_grid_for_editor(...)`?

Expected behavior:

- snapping rules should be explicit to the user
- `paint_panel` should expose a small grid-settings row between tool content and status:
  - `Snap?`
  - selection options: `yes` and `no`

Phase recommendation:

- keep the current active-grid-based implementation if it is otherwise correct
- add UI that exposes whether snapping is enabled for paint/grid-tool placement
- route that UI through a dedicated paint/grid settings state rather than burying it in implicit editor selection state

### 5. Grid Tool orientation still goes vertical too often

Observed behavior:

- grids placed via `paint_panel` often end up vertical
- this happens even on surfaces that should visually behave like floor placement, such as the ground box in `bisket-vr-demo`
- on the ground box specifically, the grid still goes straight up instead of lying on the ground plane
- the rendered line pattern is also wrong for the intended use:
  - there are no useful horizontal stripes in the placed result
  - the current grid line basis appears to still correspond to world/grid-authored axes rather than the fitted surface plane

Expected behavior:

- the flat face of the previewed/placed grid should sit flush against the dragged surface
- dragging over a floor-like surface should orient the grid so its plane lies on that surface, regardless of any confusing authored local axes on the hit object

Important nuance:

- this should be derived from the resolved placement frame / surface normal
- it should not depend on the selected transform’s authored orientation if that diverges from the actual hit surface

Likely work:

- audit the grid preview path to ensure it uses the same surface-aligned frame semantics as the intended placement model
- verify the grid visual plane basis matches the assumption made by `resolve_surface_aligned_pose_from_frame(...)`
- verify the grid mesh/material shader assumes the same local plane axis as the placement code
- confirm fallback surface normals for unsupported renderables are conservative and face-like rather than arbitrary transform-axis copies

### 6. `Select + Cursor` vs `Cursor` vs `Select` interaction with Grid Tool is unclear

Question:

- should `Grid Tool` in `paint_panel` depend on the workspace interaction mode at all?

Current concern:

- it is not obvious whether:
  - `Grid Tool` always uses panel-local placement rules
  - or whether workspace mode changes where / how the tool resolves placement

Expected design:

- `Grid Tool` should behave as a panel-local override
- while the paint panel is focused and `Grid Tool` is active:
  - workspace `Select`
  - workspace `3D Cursor`
  - workspace `Select + Cursor`
  should not materially change how the tool resolves placement preview and commit

Follow-up:

- document this explicitly
- if current behavior differs, fix routing so `Grid Tool` owns scene drag interpretation while active

Code note confirmed on 2026-06-14:

- `Select + Cursor` currently uses the same surface-hit placement path as pure `3D Cursor`
- it does not currently move the cursor to the exact selected transform / gizmo pose
- for intended semantics, see `docs/task/shared-3d-cursor-and-selection-vs-surface-placement.md`

### 7. Cursor mode currently seems editor-root specific

Observed behavior:

- `3D Cursor` mode appears to work in `bisket-vr-demo`
- it does not appear to work consistently for other editor roots
- normal selection and `Select + Cursor` selection behavior does work for those editor roots
- a stronger clue now exists:
  - the cursor appears non-existent or invisible until entering `Select` mode and clicking one of bisket's GLTF armature bone markers
  - after selecting one of those bones, cursor-related modes start working, but only for bisket
- this suggests the problem may involve cursor visual spawning / attachment / activation being gated by a selection path that bone markers satisfy and ordinary scene roots do not

This suggests:

- the issue is likely editor-root routing / scoped handler installation
- not necessarily a GLTF-only problem
- there may also be a hidden dependency on selection target kind, gizmo/cursor visual attachment ancestry, or an observer route that only becomes valid after the bisket bone selection path runs

Expected behavior:

- `Cursor3dSystem` should work for every editor root where selection already works
- cursor updates should resolve against the correct editor subtree, not just one specific demo/editor instance

Likely work:

- audit per-editor handler installation and routing precedence
- confirm `active_editor`, focused panel state, and scene-hit editor-root resolution are not suppressing cursor updates for non-primary editors
- audit cursor visual spawn / reveal conditions and whether they are incorrectly coupled to selection/gizmo setup
- compare the event path for selecting a bisket bone marker against selecting ordinary scene geometry
- root cause confirmed in code on 2026-06-14:
  - `EditorSystem::materialize_editor_raycastables(...)` skipped any immediate editor child branch whose subtree contained a `GLTFComponent`
  - `GLTFSystem` only adds explicit raycastable proxies for transform-only nodes when `with_visualized_transforms` is active
  - result: imported mesh renderables in editor trees were not editor-pickable by default, while bone/joint viz markers were
- fix direction:
  - make GLTF-bearing editor branches inherit the same default editor auto-raycast wrapper as other editor children
  - keep transform-viz bone markers as additive helpers, not the only raycastable surfaces in imported editor content
- additional root-cause thread confirmed in code on 2026-06-14:
  - the cursor marker is currently created/fetched under `active_editor`
  - `Cursor3dSystem` also resolves and updates the marker by searching under the triggering `editor_root`
  - so the current implementation is not yet modeling one shared workspace-global cursor
- add coverage with multiple editor roots, not just the bisket fixture

## Recommended next steps

1. Add tests around preview commit:
   - grid preview commit updates `GridSystem`
   - grid preview commit rerenders `grid_panel`
   - preview opacity is removed on commit
2. Audit transform math in `Free Draw` preview updates:
   - especially repeated offset application, target-size leakage, and grid-snap interaction
3. Add a small `paint_panel` grid-settings row:
   - `Snap?`
   - `yes`
   - `no`
4. Clarify panel override semantics:
   - `Grid Tool` should be panel-driven, not workspace-mode-driven
5. Expand cursor multi-editor coverage tests and compare against the bisket bone-marker activation path

## Acceptance for this follow-up batch

- `Grid Tool` committed grids appear immediately in `grid_panel`
- committed previews return to full opacity
- `Free Draw` preview follows drag hits without divergence
- snapping rules are both explicit in code and visible in `paint_panel`
- grid plane orientation sits flush to floor/wall/table surfaces rather than frequently flipping vertical
- `Grid Tool` behavior is independent of workspace mode while the paint panel owns interaction
- `3D Cursor` works consistently across editor roots, not just the bisket demo case
