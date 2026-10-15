# Task: XR head/neck bend and derived body offset

## Status and outcome

Proposed, 2026-10-15; feasibility review and implementation task only. No
runtime changes have been made for this task.

Design direction update: [declarative transform math](../draft/declarative-transform-math.md)
breaks this behavior into rotation selection, quaternion/vector math, translation
patching, and shared derived values. Explore those small reusable pieces before
choosing a dedicated IK implementation. The constraint goal and the operator
implementation are compatible; existing TOP composition already expresses part
of the geometry.

When looking down at their torso or legs in XR, the user should see a connected
neck bending with the head, with the body placed behind the head as required by
that bend. The tracked view and visible head remain authoritative. Body position
is derived from the articulated neck geometry, rather than an arbitrary
pitch-to-backward-translation multiplier.

This is feasible as a small bounded kinematic constraint. It does not require
motion classification, pose history, or a full-body IK solver. The exact bend
share and limits need headset evaluation on Bisket and Rei(mu).

## Existing documentation and actual implementation

- [IK system](../spec/ik-system.md) describes AimConstraint, TwoBoneIK, and
  FABRIK. Parts of its AVC wiring and frame-order descriptions are historical.
- [Simple humanoid body follow](avatar-control-simple-humanoid-body-follow.md)
  establishes the head-driven body baseline and rotation-only neck policy.
- [HMD-center versus head-pivot bug](../bugs/avatar-body-follow-targets-hmd-center-instead-of-head-pivot-xz.md)
  records an earlier anchor mismatch. Its raw-HMD-target description is not
  the current code: body follow already targets the displaced head pivot.
- [Transform operators](../spec/transform-operators.md) describes current TOP
  composition and retained temporal state.

Checked implementation surfaces:

- [AVC initialization and head maintenance](../../src/engine/ecs/system/avatar_control_system.rs):
  creates `driven_t -> head_mount -> displaced_head`, restores head rest
  rotation, and keeps the displaced head's local translation zero. AVC does
  not install a head AimConstraint. The head is outside the body's FK branch.
- [HeadPoseBodyXzFollowSystem](../../src/engine/ecs/system/ik/head_pose_body_xz_follow.rs):
  sets model-root X/Z to displaced-head world X/Z and Y to head Y plus a
  cached height offset. It preserves body yaw and pins neck local translation
  to rest. That pin does not calculate neck rotation or enforce endpoint
  closure between the separated branches.
- [IK solvers](../../src/engine/ecs/system/ik_system.rs) and
  [configuration](../../src/engine/ecs/component/ik_chain.rs): existing
  AimConstraint matches one joint's pose; TwoBoneIK solves a positional
  three-joint chain. Neither expresses this distributed orientation and
  floating body-root constraint. FABRIK follows a connected positional chain
  and contains an older small-neck-bend clamp; it is not a ready-made
  anatomical neck constraint for the displaced-head topology.
- [Frame scheduling](../../src/engine/ecs/system/system_world.rs): AVC writes
  flush before body follow. Body follow and IK currently tick before their
  shared flush; final secondary motion and skin refresh follow. A neck/body
  constraint that changes shoulder placement needs an explicit propagation
  boundary before arm IK reads those shoulders.

This explains a plausible missing behavior, not a headset-confirmed diagnosis.
An incorrect camera/eye-to-head calibration can produce a similar symptom.
Inspect both before tuning a neck bend to hide an offset error.

## Geometry and ownership

Keep head position and orientation derived from the same current XR pose and
calibrated eye-to-head convention as today. Do not add an offset to the rendered
XR view, tracking origin, or locomotion root to create this effect.

Distinguish three landmarks: HMD/eye origin, head-bone pivot at the skull base,
and neck base on the upper torso. Rotating the eye-to-head offset finds the
head pivot; it does not also solve the neck-base location once the neck bends.

For the first supported fixture, leave the upper torso upright and choose a
bounded fraction of head pitch as neck flexion. Compute pitch in a calibrated
anatomical frame relative to the current yaw-only body frame, accounting for
the rig's rest rotations. Avoid raw imported Euler axes. Retain the head's
full world orientation; the remaining head-versus-neck rotation is whatever
is needed to preserve that orientation. Begin with pitch sharing; separate
roll/yaw sharing and chest bending can follow.

Evaluate a virtual copy of the original root-to-head FK path using immutable
rest transforms, current body yaw, and the chosen neck rotation. Include
intermediate helper transforms. The displaced head remains the independent
target, so this virtual path must be retained before splicing changes topology.

In world units:

```text
P_head = desired head-pivot world position from tracking and calibration
d      = virtual root-to-head displacement with selected neck bend/body yaw
P_root = P_head - d

P_virtual_head = P_root + d = P_head
```

For a simple neck with fixed root-to-neck displacement `c`, neck-world
orientation `Q_neck`, and rest-local neck-to-head vector `v`:

```text
d = c + rotate(Q_neck, v)
```

Use the complete FK path for real rigs and include supported scale. Keep neck
local translation, segment lengths, and scale at their baseline values.
Apply rotation relative to the calibrated rest frame, never relative to last
frame's corrected rotation. The head remains on its existing tracking mount;
the constraint makes the virtual neck endpoint meet that separate head pivot.

As a geometric example, a vertical 12 cm segment bent forward by 25 degrees
advances its endpoint about 5.1 cm and lowers it about 1.1 cm. With head pivot
held fixed, the body moves back about 5.1 cm and up about 1.1 cm. These are
illustrative dimensions and angles, not selected human or avatar defaults.

The small Y correction matters: an X/Z-only offset with the old fixed height
cannot in general preserve both neck length and endpoint closure. Select the
full derived model-root translation for the first constraint fixture. Feed
the resulting body pose into subsequent leg/crouch evaluation; keep simulated
grounding Y independent of head tracking. Standing feet may need leg correction
or may lift slightly, so test this alongside
[crouch IK](avc-foot-penetration-crouch-ik.md) before enabling it by default.
Foot planting or zero sole error is not guaranteed by a neck solve alone.

Do not stack a second backward-offset rule on top of existing body follow.
For an eligible avatar this constraint replaces that model-root translation
write, deriving the complete root position from the shared head target and
virtual chain. Ineligible avatars retain the current behavior.

## State and transform-operator feasibility

| Concern | Required state / existing fit |
| --- | --- |
| Neck bend and body placement | Current tracked head pose, current body yaw, rig calibration and limits; deterministic without pose history |
| Rig binding | Per-instance IDs, original FK path, immutable rest transforms, anatomical basis, supported scale, and map generation |
| Existing body yaw | Already stateful in `TOP.yaw_follow`; consume its current result |
| Optional smoothing | Retained per-avatar filter values, reset on rebind/tracking transitions; not needed for the first fixture |
| Walking versus looking down | No motion classifier required; one HMD pose does not uniquely identify actual torso/neck posture |

Existing TOPs can express a fixed local landmark offset followed by yaw-only
output. They already support temporal state, so statefulness itself does not
rule out using operators. However, current fork channels are independent;
there is no operation that distributes head orientation into a neck joint and
derives a second body output from the resulting FK geometry. An ordinary child
operator also cannot update its parent or sibling neck branch.

Explore the [declarative math decomposition](../draft/declarative-transform-math.md)
before committing to a dedicated solver. The body calculation can be a
single-output transform operator; shared upstream values can supply both that
output and the neck rotation. Current TOPs do not expose that full expression
surface. A pure calculation helper with an AVC-adjacent owner remains a viable
implementation if the declarative extension proves larger than the use case.
Keep calibration, eligibility, and pose ownership explicit in either design.
Current translation sampling is restricted to structural ancestor sources and
cannot simply sample a solved sibling branch. No general transform constraint
graph is a prerequisite for the first slice.

## Implementation slices

1. **Inspect calibration and reproduce.** On
   [Bisket VR](../../examples/bisket-vr-demo.mms) and
   [Rei(mu)](../../examples/rei(mu).mms), visualize/log HMD origin, calibrated
   head pivot, neck base, virtual neck endpoint, body root, relative head pitch,
   chosen neck pitch, and endpoint residual. Verify eye/head offset and forward
   axes before comparing constraint enabled/disabled.
2. **Bind and calculate.** Use the owning GLTF's retained HumanoidBoneMap report
   for Head and Neck plus the original path to model root. Derive rest geometry
   from BoneRestPose data. Validate finite nonzero neck length and supported
   hierarchy/scale; initially support uniform scale. Missing/ambiguous maps,
   unsupported rigs, or missing calibration skip with a retained reason.
   Preserve the original pre-splice path rather than walking toward the head
   in the already-reparented runtime tree.
3. **Deterministic fixture.** Implement pitch sharing with configurable weight
   and separate flexion/extension bounds; exact defaults/API remain TBD.
   Saturate neck flexion while the head retains the rest of its tracked
   rotation. Test the virtual endpoint and derived body translation together.
4. **Integrate ownership and scheduling.** Enable only for the avatar's active,
   valid XR pose driver. Desktop and invalid/unbound cases keep their baseline.
   Make one module own neck rotation and body translation; compose the neck
   rest-position pin with it. For the first fixture the neck has one pose owner;
   future animation supplies an explicit base pose instead of racing writes.
   Flush and propagate neck/body results before arm IK and future leg target
   generation, then propagate solves before secondary motion and skinning.
   Do not depend on IK chain HashSet order.
5. **Lifecycle and headset checkpoint.** Provide enable/disable and bend tuning,
   restore the prior base pose on disable, invalidate/rebuild on rig/map/basis
   replacement, and clean up per-instance state on removal. Invalid tracking
   suspends solving; reacquisition evaluates the current pose without replaying
   accumulated correction. Suspend for mounted/seated modes until their body
   ownership is defined. Enable by default only after both headset fixtures
   pass neck, arm, and grounding checks.

The model-root update must use the current transform propagation contract
through operator boundaries. Existing body-follow comments describe an older
cached-world workaround; verify current behavior rather than copying that
workaround into a second writer.

## Acceptance and meaningful verification

- Repeating the same input produces the same neck rotation/body pose with no
  accumulation, drift, oscillation, or dependency on previous corrected FK.
- Neutral pose reproduces calibrated rest geometry, including nonzero authored
  horizontal offsets. Virtual neck endpoint and tracked head pivot agree within
  a stated numeric tolerance; neck lengths/translations/scales stay fixed.
- Pitch down through a supported sweep (for example 0, 30, 60, 80 degrees):
  the neck shares only its bounded amount, head/view remain tracked, and body
  offset follows the computed arc. Looking at torso and legs feels connected
  in the headset. Compare with actual captured motion, not a requirement that
  every physical nod hold the HMD pivot fixed.
- Upward pitch, yaw, roll, and combined rotations remain finite and continuous.
  Near vertical gaze, retain a stable body/anatomical frame rather than
  extracting yaw from a vanishing horizontal forward vector.
- Translating the whole tracked pose at fixed orientation translates the solved
  body by the same amount. Rotated parents, both avatar forward conventions,
  nonidentity rest rotations, uniform scale, and renamed bones remain correct.
- Confirm current shoulders are used by arm IK and future crouch targets use
  the corrected body pose in the same frame. Inspect sole motion from the Y
  correction, and verify head pitch cannot drive simulated capsule Y or move
  the rendered XR origin.
- Missing neck, unsupported scale, tracking loss/reacquisition, disable/re-enable,
  avatar replacement, and desktop mode behave according to the lifecycle above.

Pure geometry tests and a small integration fixture should establish closure,
frame ordering, and ownership before the headset checkpoint. A plausible
skeleton result alone does not prove skin deformation or first-person comfort.

## Related work and boundaries

- [VR embodiment epic](epic/vr-avatar-embodiment.md): integration tracker.
- [Avatar camera anchoring](../spec/avatar-camera.md): camera calibration and
  tracking-view ownership.
- [Avatar pose layers](avatar-pose-transition-layers.md): later animation
  composition; not required for the initial single-owner neck fixture.
- [Joint basis retargeting](../spec/joint-basis-retargeting-system.md): anatomical
  frame conventions; inspect available runtime basis support during binding.

This task extends the simpler body-follow baseline with bounded neck flexion;
it does not revive spine FABRIK, infer whole-body posture from one headset,
change eye tracking, or implement walking/foot planting.
