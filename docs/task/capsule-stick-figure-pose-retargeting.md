# Capsule stick figure XR pose-retargeting laboratory

Status: proposed.

## Purpose

[`examples/capsule-stick-figure.mms`](../../examples/capsule-stick-figure.mms)
is the visual and interaction laboratory for an imported humanoid GLB whose
bind pose is an A-pose.  It contains one static
`assets/models/capsule_stick_figure.glb` subject in a lit XR studio and one
live Bisket XR avatar.  Bisket retains the complete established stack:
AvatarControl, humanoid mapping, morph-target mapping, colliders, secondary
motion, eye animation, XR hands, and a grabbable mirror.

The capsule is deliberately **not** an XR avatar yet, and it has neither AVC
nor `PoseCapture`.  The first acceptance check is simply that it imports at
the expected scale, remains in its authored A-pose, and can be inspected in
the mirror while the Bisket avatar moves normally.

## Current pose-panel behavior

The panel represents a `PoseCaptureComponent` as a capture-library section.
Its Capture action always captures that section's owner.  A capture appends a
new `pose_N` entry; it does not replace or recapture an existing pose.

Apply does have an implicit destination rule, but no explicit UI control:

1. the currently selected glTF (or a selected descendant of it), when one is
   selected;
2. otherwise, the original glTF that owns the capture library.

Apply validates that every stored joint query resolves exactly once on that
destination before mutating it.  Consequently selecting the capsule could
already attempt an apply, but it is neither discoverable nor a retargeting
solution: Bisket's captured joint names do not match the capsule skeleton.

The present panel also has no delete action for a pose.  Rename and save exist,
but neither gives an operator a direct replacement/recapture workflow.

## Phase 0 — XR scene baseline

- [ ] Start `capsule-stick-figure.mms` in XR; no desktop camera/window is
  required by the scene.
- [ ] Inspect the one capsule subject under all four spotlights and in the
  movable mirror.  Confirm its imported A-pose, scale, facing, floor contact,
  and armature labels.
- [ ] Confirm Bisket's head, hands, locomotion, eye/morph behavior, colliders,
  and secondary motion still work.
- [ ] Capture a few Bisket poses in the panel only to exercise the source
  library.  Do not expect Apply to pose the capsule during this phase.
- [ ] Record the actual capsule joint names, hierarchy, local axes, rest
  rotations, and a proposed humanoid slot map.

## Phase 1 — ComponentPicker and one explicit apply destination

A pose-library section already has a useful and stable meaning: it is the
capture source. Its `Capture` button samples the `PoseCaptureComponent` that
owns the section. Do not add per-section "apply target" toggles; several
checked sections would silently turn a normally singular action into a
fan-out action, and would add persistent controls to every library row.

The first implementation task is to create the reusable `ComponentPicker` and
install exactly one instance at the top of Pose-panel content, before `New
Pose Library` and every library section. Do not add an extra UI element to
each section.

That field is one optional panel-level apply-destination override:

```text
Apply pose to   [ Pick component ]  Capsule stick figure · ComponentId(…)
                                     [ × ]
```

This is a reusable **ComponentPicker** field (specified in
[component-picker.md](./component-picker.md)). Its value is a component
reference, not a new kind of game-object identity. The Pose panel may accept a
picked glTF root or a picked descendant, resolving its owning glTF as the
actual apply destination.

When the field has a valid value, every `Apply` action in every library section
uses that destination in preference to editor selection and the source-library
fallback. The panel must show the destination's readable name/type and runtime
component ID so this redirection is never hidden. `×` clears the override. An
empty field keeps the existing selection-then-source fallback during migration,
and should visibly say `Auto (editor selection / source)` rather than looking
like an omitted control.

This gives the desired two streams without a second capture picker:

| Stream | Owner | Initial laboratory value |
| --- | --- | --- |
| Capture source | each pose-library section | Bisket XR avatar |
| Apply destination | one optional Pose-panel ComponentPicker | capsule stick figure |

The panel must reject an unresolved reference, an object with no owning glTF,
or an ambiguous destination before it emits a pose update. Its normal pose
compatibility preflight still applies after resolving the destination.

Applying one pose to multiple armatures is a legitimate future operation, but
must be designed explicitly as a destination list/batch action with a per-item
preflight result. It is not part of this singular picker slice.

## Phase 2 — retargeting contract

Name-based captured joint queries are appropriate for applying a pose back to
the exact same GLB, but are not sufficient across these two armatures.  Add a
separate mapping layer between a source skeleton and a destination skeleton.
At minimum it needs:

- source and destination humanoid slots (not raw matching joint names);
- each joint's imported rest local rotation and a calibrated joint basis;
- a policy for missing/extra joints and unmapped fingers;
- clear translation policy (normally keep destination bone translations and
  retarget rotation only, with explicit root/hips handling);
- a preview/apply mode that does not interfere unpredictably with Bisket AVC
  or the capsule's future animation/IK producers.

The initial result may be a deliberately small upper-body mapping.  It should
be possible to diagnose each mapped slot and its rest-basis correction before
claiming general humanoid retargeting.

## Phase 3 — pose-library editing follow-up

Treat this as a related panel usability slice, not a prerequisite for the
scene:

- [ ] allow selecting an existing pose as the target of Recapture/Replace;
- [ ] add a delete action with confirmation and unsaved-state handling;
- [ ] clarify duplicate pose names and stable identity in the saved manifest;
- [ ] preserve library ordering through rename, replace, delete, and save;
- [ ] add UI and system tests for these actions, including a hydrated library.

## Related tasks

- [Component picker](./component-picker.md) — reusable scene-component field,
  pick mode, reference persistence, clear action, and selection feedback.
- [Skinned-mesh grounding and static floor contact](./skinned-mesh-grounding-and-floor-contact.md)
  — place the Bisket reference and A-pose capsule on one measured ground plane
  before interpreting their visual height difference.

## Out of scope for this first example

- Making the capsule figure the live XR avatar.
- Automatically inferring a humanoid map or joint basis solely from names.
- Applying a Bisket capture directly to the capsule without a mapping.
- Changing the existing implicit Apply behavior before the explicit two-stream
  design and its migration path are agreed.
