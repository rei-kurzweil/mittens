# Task: Gizmo drag regression and workspace lock toggle

Date: 2026-06-16

This task tracks a broad editor/gizmo regression that is not clearly VR-specific, plus a short-term
workspace mitigation: a `Lock Gizmos` toggle that keeps gizmos visible and attached but prevents
them from responding to drags.

---

## Regression summary

Selecting a new object can immediately modify the transform that was previously selected.

Current confirmed repro:

- happens in `Select`
- happens in `Select + Cursor`
- does not reproduce in `3D Cursor`
- reproduces even if the bisket editor has not been interacted with first
- reproduces by clicking the glowing animated cubes in a separate editor tree
- is more noticeable when the previously selected transform and newly selected transform are
  farther apart
- appears to move or otherwise modify the previously selected transform when the new selection is
  made

This reproduces both within a single editor tree and across different editor trees.

---

## What this weakens

The first theory was that a freshly attached gizmo was consuming the same `DragStart` that caused
selection.

That is now likely only part of the story, or not the main cause, because:

- the bug still reproduces after a first narrow gizmo-local guard attempt
- the bug is not confined to one editor tree
- the wrong transform appears tied to previous selection state, not just freshly attached gizmo
  state

More likely remaining causes:

- stale active gizmo drag state surviving retargeting
- `DragMove` still applying to a previously selected target after selection changed
- active raycaster / drag ownership not being cleared when selection changes
- an ordering mismatch between selection, attach, `ParentChanged`, and gizmo target resolution

---

## Immediate mitigation

Add a workspace-wide `Lock Gizmos` toggle in the editor settings panel.

V1 behavior:

- gizmos still spawn and attach to the selected object
- selection behavior is unchanged
- gizmo visuals still follow selection
- while locked, gizmo drag handlers do not arm and do not apply transform changes

This is a testing and safety mitigation, not a root-cause fix.

Default:

- unlocked

Scope:

- workspace-wide shared editor context
- not serialized
- not per-editor

---

## Next investigation steps

1. Add targeted tracing around selection changes, gizmo retargeting, and drag lifecycle:
   - `select_editor_target`
   - `TransformGizmoSystem::on_parent_changed`
   - `TransformGizmoSystem::on_drag_start`
   - `TransformGizmoSystem::on_drag_move`
   - `TransformGizmoSystem::on_drag_end`
2. Run a temporary experiment that disables all gizmo attachment logic, specifically the attach
   path, to confirm whether every repro still depends on attachment.
3. In parallel, land the workspace `Lock Gizmos` mitigation so VR and desktop testing can proceed
   without accidental gizmo drags.

---

## Related docs

- `docs/task/vr-pointer-and-controller-followups.md` — XR/controller-specific follow-ups
- `docs/task/editor_selection_and_paint_perf.md` — existing selection/gizmo performance notes
