# XR hand laser is selectable and its origin is past the fingertip

Date: 2026-07-28

Status: code fixed / automated coverage passing / VR verification pending

2026-10-14 follow-up: the [XR color tool can still color the beam and avatar](xr-color-tool-targets-own-laser-and-avatar.md).
The selection marker implemented here does not disable ray intersection, and
paint currently fails to honor Selectable-off sidecars consistently with the
scene-selection resolver. Track that separately from this selection/origin fix.

Primary reproduction: `examples/bisket-vr-demo.mms`

Related:

- `docs/task/vr-pointer-and-controller-followups.md`
- `docs/task/refactor/pointer-system.md`
- `docs/draft/pointer.md`
- `docs/bugs/transform-gizmo-screen-size-varies-with-camera-distance.md`
- `docs/task/grid-gizmo-paint-end-to-end-ux-and-test-matrix.md`

## Summary

The runtime XR hand laser has two observed problems:

1. Its visual/renderable can sometimes be selected by the editor.
2. Its origin appears too far beyond the hand/fingertip, roughly one extra
   finger segment or about twice the expected offset.

When the laser is selected, the shared transform gizmo attaches to it and moves
around with the tracked hand. The laser is runtime presentation for a pointer
and should never be an editor selection target.

The origin problem affects more than appearance. The authored `Pointer` is
reparented to the same avatar-finger mount as the laser, so both the visible
beam and the pointer ray begin at the extrapolated position.

## Reproduction

### Accidental selection

1. Run `bisket-vr-demo` in VR.
2. Use either hand laser around selectable scene content.
3. Select repeatedly, including when the laser crosses the selection ray or
   overlaps the intended target.
4. Observe that a laser component/renderable is sometimes selected.
5. Observe a transform gizmo following the moving hand laser.

### Offset origin

1. Hold either tracked hand where the fingers and laser origin are visible.
2. Compare the laser's near face/ray origin with the intended emitting
   fingertip or final selected finger joint.
3. Observe that the laser begins noticeably too far beyond the hand.
4. Repeat on both hands and at different hand rotations.

## Expected behavior

### Selection

- no node in the runtime laser presentation subtree is selectable
- the laser must not win or redirect scene selection
- clicking/raycasting through the laser should select the intended scene target
- a transform gizmo must never attach to:
  - `xr_pointer_laser`
  - `xr_pointer_laser_mesh`
  - the laser `RenderableComponent`
  - `xr_avatar_finger_laser_mount`
  - the laser-owned pointer/raycaster plumbing
- the laser can remain visible and emissive without becoming editor content

### Origin

- the visible beam's near face and the pointer ray origin should agree
- the default avatar-finger laser should begin at the agreed fingertip anchor
- the result should be stable across left/right hands and hand rotation
- any intentional extension beyond a selected joint should be explicit and
  configurable rather than hard-coded to one full segment

## Current runtime topology

For an avatar-finger laser, the relevant runtime shape is approximately:

```text
AVC corrected hand target
  xr_avatar_finger_laser_mount Transform
    Serialize.off
    Pointer                         <- reparented here
      RayCast
    xr_pointer_laser Transform
      Serialize.off
      xr_pointer_laser_mesh Transform
        Renderable.cube
          Color
          Opacity
          Emissive
```

The cube mesh is centered, so the mesh transform is:

```text
translation = [0, 0, -5]
scale       = [0.002, 0.002, 10]
```

Because the cube primitive spans `-0.5..+0.5`, the corrected transform makes
the near face start at laser-local `z = 0` and extends the beam ten units along
local `-Z`.

The former `z scale = 5` assumption was incorrect: it produced a five-unit beam
from `z = -2.5` to `z = -7.5`, leaving a 2.5-unit gap at the pointer origin.
That explained why the laser still appeared both short/stubby and too far away
after the avatar mount itself was moved back to the selected tip joint.

Relevant code:

- `src/engine/ecs/system/pointer_system.rs`
- `src/engine/ecs/component/controller_xr.rs`
- `src/engine/ecs/system/avatar_control_system.rs`
- `examples/bisket-vr-demo.mms`

## Finding 1: the visual subtree lacks an explicit selection barrier

The laser runtime subtree currently adds:

- `SerializeComponent::off()`
- transforms
- renderable
- color/opacity/emissive presentation

It does not add:

- `SelectableComponent::off()`
- an equivalent editor-selection exclusion marker

The existing test is named
`xr_laser_is_single_runtime_noninteractive_visual_aligned_to_negative_z`, but
its noninteractive assertion only verifies that the laser root has no direct
`RaycastableComponent` child. That is insufficient if:

- raycastability is inherited from an ancestor
- scene selection considers registered renderables by default
- semantic target resolution walks upward into selectable editor content
- the pointer's own runtime raycast topology makes sibling/ancestor presentation
  eligible

The runtime subtree should express selection exclusion directly rather than
depending on the absence of one local component.

## Finding 2: the mount extrapolates one full segment past the selected tip

For the three configured avatar finger joints:

```text
root   = first selector
middle = second selector
tip    = third selector
```

the current mount calculation is:

```text
final_segment = tip_position - middle_position
direction     = normalize(final_segment)
origin        = tip_position + final_segment
```

The origin is therefore one complete `middle -> tip` segment beyond the third
joint.

In `bisket-vr-demo`, the configured chains are:

- `J_Bip_L_Middle1`, `J_Bip_L_Middle2`, `J_Bip_L_Middle3`
- `J_Bip_R_Middle1`, `J_Bip_R_Middle2`, `J_Bip_R_Middle3`

The extrapolation may have been intended to estimate the physical fingertip
from a terminal joint pivot. The current test explicitly encodes that behavior:
with the third joint at rest-space `x = 0.3`, it expects the mount at `x = 0.4`.

The VR observation indicates that one full previous-segment length is too much
for the bisket skeleton, or that `Middle3` already provides the desired emission
anchor.

## Priority and dependency order

### P0: prevent runtime presentation selection

- [x] `XR-LASER-01` Add an explicit selection-exclusion marker at the highest
  laser presentation root that covers all descendants.
- [ ] `XR-LASER-02` Verify the marker survives runtime creation, reparenting,
  shared-editor selection routing, and world-panel projection.
- [x] `XR-LASER-03` Add a selection regression test which attempts to resolve a
  laser renderable and asserts that it cannot become the semantic editor target.
- [ ] `XR-LASER-04` Verify clicking through the laser still reaches intended
  scene content.
- [ ] `XR-LASER-05` Verify neither the pointer/raycaster nor the fingertip mount
  can receive a transform gizmo.

This is first because selecting a hand-driven runtime node causes a highly
disruptive moving gizmo and can interfere with subsequent editing.

### P1: define and correct the emission anchor

- [ ] `XR-LASER-10` Capture left/right mount, `Middle2`, `Middle3`, visible
  fingertip, beam near face, and ray origin in world space.
- [x] `XR-LASER-11` Decide the default anchor:
  - exactly `Middle3`
  - a fractional extension beyond `Middle3`
  - an explicit fourth fingertip/end selector
  - geometry-derived distal endpoint
- [x] `XR-LASER-12` Replace the unconditional full-segment extrapolation with
  the chosen policy.
- [x] `XR-LASER-13` Keep pointer ray origin and visible beam near face identical.
- [x] `XR-LASER-14` Update the existing unit test so it asserts the chosen
  anchor rather than hard-coding `tip + final_segment`.
- [ ] `XR-LASER-15` Verify fallback controller-space lasers remain rooted at
  their controller/pointer-driving transform.

Recommended first experiment:

- [x] use `tip_position` directly for the avatar-finger mount
- compare both hands in `bisket-vr-demo`
- only introduce a fractional/configurable extension if the beam visibly begins
  inside the fingertip mesh

## Implemented fix

`pointer_system` now places `Selectable.off` on:

- every `xr_pointer_laser` root, covering the beam mesh/renderable in both
  controller-space and avatar-finger modes
- every `xr_avatar_finger_laser_mount`, covering the reparented pointer and its
  raycaster as well as the beam

The avatar-finger mount now uses the selected third joint position directly.
The final `middle -> tip` segment is still used to orient local `-Z`, but is no
longer added to the origin.

The centered unit-cube beam now uses a Z scale of `10` at local Z `-5`, placing
its near and far faces at `0` and `-10` respectively. Previously its Z scale was
`5`, which placed those faces at `-2.5` and `-7.5`.

Automated coverage verifies:

- the real world-scene hit resolver rejects both controller-space and
  avatar-finger beam renderables
- the avatar-finger pointer is below a selection-exclusion marker
- left and right mounts are at the selected tip position (`0.3` in the fixture),
  not the former extrapolated position (`0.4`)
- the beam root remains at zero relative to the shared pointer mount, keeping
  the visible near face and ray origin coincident by construction
- the unit-cube beam bounds are exactly `z = 0..-10`

## Diagnostic trace

Add a temporary, rate-limited trace per laser:

```text
hand
controller pose kind
laser mode: controller | avatar-finger
root/middle/tip ids
middle rest/world position
tip rest/world position
final segment length
configured extension factor
mount world position
pointer ray world origin/direction
beam near-face world position
selected semantic target when laser renderable is hit
```

The beam near-face position and pointer ray origin should be equal within a
small tolerance.

## Automated test matrix

| ID | Layer | Scenario | Required assertion |
|---|---|---|---|
| XL-01 | topology | spawn controller-space laser | one runtime laser, `Serialize.off`, selection excluded |
| XL-02 | topology | spawn avatar-finger laser | mount, pointer, and beam share the intended anchor |
| XL-03 | selection | hit laser renderable | no semantic editor selection target resolves to laser subtree |
| XL-04 | selection | scene object directly behind laser | scene object remains selectable |
| XL-05 | gizmo | attempt selection through every laser node | no transform gizmo attaches to runtime laser/pointer nodes |
| XL-06 | origin | direct-tip policy | mount equals third joint position |
| XL-07 | origin | configured extension policy, if retained | extension is explicit and matches configured fraction |
| XL-08 | parity | left and right chains | mirrored hands use equivalent anchor semantics |
| XL-09 | transform | rotate/move tracked hand | ray origin and beam near face remain coincident |
| XL-10 | fallback | avatar binding unavailable | controller-space fallback remains correctly rooted and nonselectable |

## VR manual verification matrix

| ID | Action | Expected |
|---|---|---|
| XV-01 | Sweep left laser over selectable terrain and editor objects | Laser never becomes selected |
| XV-02 | Sweep right laser over the same targets | Laser never becomes selected |
| XV-03 | Repeatedly click while laser overlaps intended target | Intended target wins; no hand-following gizmo |
| XV-04 | Inspect left beam origin beside visible fingers | Near face begins at agreed fingertip anchor |
| XV-05 | Inspect right beam origin beside visible fingers | Same anchor policy as left |
| XV-06 | Rotate wrists through a wide range | No positional orbit/gap appears between fingertip and beam |
| XV-07 | Compare visible beam and actual hit direction | Beam and pointer ray remain coincident |
| XV-08 | Trigger avatar-finger fallback, if reproducible | Fallback origin is correct and nonselectable |

## Definition of done

- runtime laser presentation cannot become an editor selection target
- transform gizmos never attach to laser, mount, pointer, or raycaster runtime
  nodes
- clicking through the beam selects the intended scene object
- the beam near face begins at the agreed fingertip anchor
- visible beam origin and pointer ray origin coincide
- left/right and controller/avatar-finger modes are covered
- unit tests no longer describe an unintended full-segment offset as correct
- VR manual verification passes in `bisket-vr-demo`
