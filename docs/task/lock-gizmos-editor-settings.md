# Plan: Document the Gizmo Drag Regression and Add a Workspace `Lock Gizmos` Gate

## Summary

Update the docs to reflect the latest repro facts and create a new focused task for this bug, since it is no longer clearly VR-specific. The new task should track both the broader regression investigation and a mitigation path: a workspace-wide `Lock Gizmos` toggle that still allows selection and gizmo placement, but blocks gizmo drag response unless the shared editor workspace context explicitly allows it.

Chosen defaults:

- Track this in a new dedicated task doc, and cross-reference it from the existing VR follow-up doc.
- The new toggle is workspace-wide, stored in shared editor workspace/context state rather than on each `EditorComponent`.
- The toggle defaults to `Unlocked`, so current behavior is preserved until the user turns locking on.

## Key Changes

### 1. Docs: broaden and split tracking

- Create a new task doc dedicated to the regression, likely under `docs/task/` with a name centered on gizmo drag regression / lock toggle.
- In that new doc, record the current confirmed repro:
  - happens in `Select`
  - happens in `Select + Cursor`
  - does not happen in `3D Cursor`
  - reproduces even without interacting with the bisket editor first
  - reproduces by clicking the glowing animated cubes in a separate editor tree
  - seems more noticeable when the previously selected transform and newly selected transform are farther apart
  - appears to modify the previously selected transform when selecting a new one
- Explicitly note that this weakens the “freshly attached gizmo consumed the same drag start” theory as a complete explanation.
- Add a short “next investigation step” note in the new doc:
  - try a temporary no-attach experiment for all gizmo attachment paths
  - in parallel, add a workspace `Lock Gizmos` mitigation so the bug can be suppressed during testing
- Update `docs/task/vr-pointer-and-controller-followups.md` to replace the editor/gizmo section with a short summary plus a link to the new dedicated task, keeping the VR doc focused on XR/controller follow-ups.

### 2. Workspace state: add a gizmo-drag gate

- Extend `EditorContextState` with a workspace-wide boolean such as `gizmos_unlocked` or `lock_gizmos`.
- Keep this state in the shared editor context path, not on `EditorComponent`, so all editor trees observe the same drag-enable flag.
- Preserve existing behavior for:
  - selection updates
  - active editor tracking
  - gizmo spawn / placement / retargeting
- The new flag only gates whether gizmo drag handlers may arm and apply transform changes.

Recommended semantic shape:

- Store `lock_gizmos: bool` in `EditorContextState`
- Default `lock_gizmos = false`
- Treat `true` as “gizmos are visually present but non-draggable”

### 3. Settings panel: expose `Lock Gizmos`

- Extend the editor settings panel model/options to include a new row for `Lock Gizmos`.
- Follow the existing editor settings selection/payload path rather than introducing a separate click handler.
- Add a new settings option payload/value for the lock toggle, alongside the current interaction-mode options.
- Update the shared editor-context event reduction so a settings selection can toggle the workspace lock flag without affecting interaction mode unless that row was selected.
- Update the settings panel sync logic so the selected/highlighted row still reflects the current interaction-mode rows, while the `Lock Gizmos` row reflects its own on/off state visually.
- Do not make the lock row change selection semantics for scene objects; it is only a workspace setting.

Implementation expectation:

- interaction mode remains modeled exactly as it is today
- `Lock Gizmos` is an additional settings action, not a fourth `EditorInteractionMode`

### 4. Gizmo behavior: gate drag response only

- Add a single early guard in the transform gizmo drag path so gizmo drags do nothing while the workspace lock flag is on.
- Apply the guard at the earliest safe point, preferably before `active_raycaster` is armed or any drag state is mutated.
- Ensure the lock affects:
  - `on_drag_start`
  - any subsequent drag move path if a drag was somehow already armed before lock engaged
- Keep these behaviors unchanged while locked:
  - clicking scene objects still selects them
  - gizmo still spawns / reattaches / visually follows selection
  - `3D Cursor` mode remains unaffected except insofar as it already avoids gizmo drag
- Do not use the lock toggle to suppress gizmo creation or attachment in v1; it is a drag gate only.

## Test Plan

- Doc review:
  - new task clearly states the broader repro and links back to the VR doc
  - VR follow-up doc points readers to the dedicated task for the editor/gizmo regression
- Behavior checks:
  - in `Select`, selecting an object still updates selection and moves the gizmo while locked
  - in `Select + Cursor`, same as above
  - while locked, dragging gizmo handles does not move transforms
  - while unlocked, current gizmo drag behavior is unchanged
  - `3D Cursor` mode still avoids the regression path as before
  - selecting in one editor tree and then another still places the gizmo correctly, but drags are blocked when locked
- Settings checks:
  - editor settings panel shows the new `Lock Gizmos` row
  - toggling it updates shared workspace context immediately
  - the toggle persists for the current runtime session across editor trees
- Investigation support:
  - the new task doc lists the follow-up “disable all gizmo attachment paths” experiment as a separate diagnostic step, not as part of the lock-toggle implementation

## Assumptions

- The bug is broad editor/gizmo behavior, not primarily a VR-only issue, so a dedicated task doc is the right source of truth.
- Workspace-wide lock is the intended UX because the bug reproduces across editor trees and the request explicitly preferred workspace context.
- Default is `Unlocked` to preserve current baseline behavior until the user opts into the mitigation.
- V1 does not require persistence/serialization of the lock toggle; runtime shared editor context is sufficient.
