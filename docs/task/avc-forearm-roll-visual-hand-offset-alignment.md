# Task: AVC forearm roll alignment and elbow deformation

Date: 2026-06-28

Updated: 2026-10-12

Status: active investigation; forearm roll follow works, elbow pinching remains.
Documentation/source inspection only for this update; no code or model changes,
runtime reproduction, or tests run while code is being edited separately.

## Current report: hand roll pinches the elbow

The user reports that rotating the tracked hand now rolls the lower arm while
two-bone IK supplies the arm's segment directions. The wrist end looks correct,
but the elbow end pinches in some lower-arm orientations. Desired behavior is
less hand-driven twist near the elbow while preserving the good wrist alignment.
The exact affected scene/model and pose capture are still to be recorded.

This extends the existing forearm task. The June frame-mismatch discussion below
is historical context, not a confirmed diagnosis of the new elbow symptom.

### Findings from current source and bundled assets

- In [IKSystem](../../src/engine/ecs/system/ik_system.rs), `solve_two_bone`
  first solves the shoulder/elbow/wrist geometry, then constructs the solved
  hand orientation from `new_lower_world_rot * end_local_rest`. It extracts
  twist from the difference to `target_world_rot`, about the geometric
  elbow-to-target axis, and applies that entire extracted twist to
  `new_lower_world_rot`. This is quaternion swing/twist handling, not copying
  an imported local Euler roll axis.
- That is one rigid lower-arm transform. It contains no spatial falloff or
  separate elbow-end and wrist-end twist transforms. Pure rotation about the
  segment axis can preserve joint positions while rotating the skin around them;
  correct IK positions alone do not establish correct elbow deformation.
- The end hand rotation, when `copy_end_rotation` is enabled, consumes the same
  `target_world_rot` and compensates relative to the twisted lower arm.
  `read_bone_rest_rot` uses `BoneRestPoseComponent` when present, with a live
  transform fallback when absent. Verify imported rest data in the affected rig;
  the existing rest-rotation regression test does not validate visible skin.
- [AVC arm setup](../../src/engine/ecs/system/avatar_control_system.rs) creates
  a three-joint upper-arm/lower-arm/hand chain from the humanoid map. This path
  does not distribute roll across additional deformation joints. The IK
  `weight` blends the whole solve; it is not a forearm twist falloff control.
- Read-only inspection of `assets/models/bisket.glb` and `assets/models/rei(mu).glb`
  found direct `J_Bip_*_UpperArm -> J_Bip_*_LowerArm -> J_Bip_*_Hand` chains
  on both sides, no nodes named twist/roll, and `JOINTS_0`/`WEIGHTS_0` skin data.
  This establishes those assets' topology, not which model produced the report
  or the quality of its elbow weights. Collider/cloth children are not evidence
  of dedicated forearm twist deformation joints.
- The [deformation shader](../../assets/shaders/mesh-deformation.comp) computes
  a weighted sum of joint matrices before transforming each vertex, matching
  the [CPU reference](../../src/engine/graphics/deformation_reference.rs).
  This is linear blend skinning. Blending differently rotated joint matrices
  can shrink the cross-section, so elbow weight transitions are a plausible
  source of pinching even with correct bone orientations.

### Working hypothesis and weight-painting limits

The leading hypothesis is a deformation limitation: full twist reaches the
lower-arm joint at the elbow and blends against the upper arm's different
rotation. A rotation/rest-frame error remains possible; source inspection and
the user's report do not establish the cause.

Weight painting can reduce how much twisting lower-arm motion reaches elbow
vertices by changing their influences. It cannot make one rigid bone rotate
less at one end. Moving weights onto the upper arm also changes elbow bending,
so simply reducing lower-arm weights risks poor flexion or stretching. Blending
only between joints that carry the same roll cannot produce an untwisted elbow.

For explicit control of twist along the forearm, investigate dedicated twist
deformation joints with appropriate weights, retaining the main IK joint for
elbow bending and placing more twist toward the wrist. Joint hierarchy and
hand compensation must avoid double application. This is a candidate rig/engine
extension, not yet a selected fix. Reducing all lower-arm twist merely leaves
more relative rotation at the wrist and may reintroduce the original kink.

### Investigation before choosing a fix

1. Record the affected scene, model, pose source, IK policy, rest data, and
   screenshots. Hold wrist position and elbow bend fixed while sweeping hand
   twist through neutral, +/-45 and +/-90 degrees; repeat both arms with
   straight, moderately bent, and deeply bent elbows. Include wrist flexion
   and extension separately to distinguish swing from forearm twist.
2. Capture corrected target, upper/lower/hand world rotations, geometric twist
   axis, extracted twist, elbow position, and skin palette for matching frames.
   Check sign, continuity, rest basis, and whether a supposedly pure twist
   unexpectedly changes the bend plane. Coordinate with the
   [wrist/arm/finger basis audit](hand-tracking-wrist-arm-and-finger-basis-audit.md)
   if a target-frame discrepancy appears.
3. Inspect the affected mesh's actual elbow-to-wrist weights, inverse binds,
   and any helper joints. Separate skin from sleeve/cloth deformation. Compare
   the same bone matrices in an authoring tool using linear blend skinning;
   a different volume-preserving deformation mode is a separate comparison.
4. After concurrent code work finishes, compare baseline, twist disabled,
   reduced global twist, and a controlled weight-only variant. These are
   diagnostics: record elbow shape, wrist shape, bending, and tracking fidelity
   together before adopting a change.
5. Choose the smallest supported fix: correct a proven rotation error, adjust
   demonstrably poor asset weights, or specify twist-joint distribution when
   independent proximal/distal twist is required. Consider a different skinning
   method only if matrix-blend volume loss remains the demonstrated limitation.

### Proposed inspection example

Reuse the existing diagnostics when implementation starts:

- [ik-rest-pose-vr.mms](../../examples/ik-rest-pose-vr.mms) and its
  [launcher](../../examples/ik-rest-pose-vr.rs) already provide a rendered mirror,
  Bisket/Rei(mu) model selection, arm debug geometry, and four-pose capture.
- [bisket-vr-debug.rs](../../examples/bisket-vr-debug.rs) already scripts neutral
  and mirrored palm-up/down arm probes, with target/bone samples and comparisons
  of authored hand offsets and end-rotation copying. Its numeric output alone
  cannot establish whether the elbow skin pinches.

Add a rendered twist inspection mode, preferably reusing the existing inspection
scene, with a deterministic desktop replay as well as tracked XR comparison.
Keep wrist position and the bend plane fixed for each sweep; provide separate
elbow-bend presets and a hand-twist slider/sweep. Show forearm/hand axes, elbow
and wrist markers, extracted twist, target error, and a close view of the elbow
skin. Compare both avatars and both arms under simple lighting, with sleeve
and secondary motion isolated when they obscure the skin.

Begin with baseline captures. Once diagnostic support is implemented, compare
full/zero/reduced forearm twist at identical target poses; separately load
weight-only and twist-joint asset variants if made. A procedural tube with known
weights can isolate matrix-blend shrinkage and demonstrate twist distribution,
but acceptance still requires the affected avatar. Twist joints need both skin
weights and runtime rotation updates; importing extra bones alone is insufficient.
No example implementation was made during this documentation pass.

### Acceptance for the elbow follow-up

- [ ] A reproducible capture distinguishes incorrect bone rotation from skin
  deformation with correct rotations.
- [ ] The chosen fix specifies whether it changes AVC/IK, rig/weights, or
  skinning, with evidence for that choice.
- [ ] Elbow pinching is reduced over the captured twist and bend range while
  retaining wrist alignment, target fidelity, and left/right symmetry.
- [ ] Any twist-joint solution preserves segment lengths and elbow positioning
  and applies the tracked hand orientation once.

## Historical investigation: visual hand frame alignment (2026-06-28)

Related bug:

- [docs/bugs/xr-hand-tracking-wrist-kink-and-jitter.md](../bugs/xr-hand-tracking-wrist-kink-and-jitter.md)

Related code:

- [src/engine/ecs/system/avatar_control_system.rs](../../src/engine/ecs/system/avatar_control_system.rs)
- [src/engine/ecs/system/ik_system.rs](../../src/engine/ecs/system/ik_system.rs)
- [src/engine/ecs/component/avatar_control.rs](../../src/engine/ecs/component/avatar_control.rs)
- [examples/vtuber-mirror-example.mms](../../examples/vtuber-mirror-example.mms)
- [examples/bisket-vr-debug.mms](../../examples/bisket-vr-debug.mms)
- [examples/bisket-vr-debug.rs](../../examples/bisket-vr-debug.rs)

## Problem

The recent lower-arm roll follow change materially improved the XR wrist kink.
The arm now looks broadly correct, but there is still a fixed rotational offset
between the lower arm and the hand, especially visible in local Z roll.

The remaining mismatch looks less like a generic IK failure and more like a
frame-of-reference mismatch:

- the hand is being visually corrected by `hand_grip_rotation_left/right`
- the lower arm is following hand roll much better than before
- but the lower arm still appears to be matching a different rotation frame than
  the final visible hand frame

The result is that the hand can look correct for the controller/hand-tracking
pose while the lower arm remains consistently offset from that same visual pose.

## Current understanding

The likely split is:

- `AvatarControlSystem` resolves a hand target and may apply a grip-space visual
  offset for authored controller semantics
- `IKSystem` solves upper/lower arm position, then now applies forearm twist so
  the lower arm follows hand roll more closely
- the end hand bone still copies target rotation relative to the solved lower
  arm

If the forearm twist is derived from a pre-offset target frame while the hand is
finally displayed in a post-offset frame, a constant lower-arm/hand roll gap is
expected.

That matches the current symptom better than:

- wrong chain resolution
- missing forearm participation
- or simple left/right sign inversion

## Goal

Make the lower arm and hand use the same intended visual hand frame for roll.

More concretely:

1. keep the authored hand grip offset that makes the visible hand/controller
   relationship look correct
2. make the lower-arm roll follow that same corrected hand frame
3. avoid introducing a second fixed roll offset that only affects the hand end
   effector

## Likely fix point

The next fix probably belongs at the hand-target/orientation-source boundary,
not in generic two-bone elbow positioning.

The main question is where the grip offset should become authoritative:

- option A: bake the grip offset into the IK target rotation that both the
  forearm twist solve and the hand end-effector copy use
- option B: keep a separate raw tracking frame and synthesized visual hand
  frame, but make both lower-arm twist and hand copy read the visual frame

Either way, the lower arm should not be solving against one rotational frame
while the hand mesh is rendered in another.

## Investigation / implementation plan

1. Trace the exact rotation pipeline for each hand target:
   - raw XR/controller rotation
   - any `TransformMapRotationComponent` or authored rotation remap
   - `hand_grip_rotation_left/right`
   - final `IKChainComponent.target_id` world rotation
2. Confirm whether the current forearm twist extraction in
   [ik_system.rs](../../src/engine/ecs/system/ik_system.rs) is using the same
   post-offset target rotation that the hand bone ultimately copies.
3. If not, define one explicit "visual hand target rotation" and use it for:
   - lower-arm twist matching
   - hand end-effector rotation copy
4. Re-run the scripted [bisket-vr-debug.rs](../../examples/bisket-vr-debug.rs)
   wrist poses and compare:
   - lower arm local roll vs hand local roll
   - left/right mirrored palm-up and palm-down poses
   - runs with and without neutralized hand grip offsets

## Acceptance criteria

This follow-up is complete when:

- the remaining fixed lower-arm/hand roll offset is explained in terms of one
  concrete frame mismatch or authored offset application point
- the lower arm and hand respond to the same visual hand frame
- mirrored left/right roll poses stay visually symmetric
- removing or changing `hand_grip_rotation_left/right` changes both hand and
  lower-arm roll behavior coherently rather than only moving the hand

## Non-goals

This task does not require:

- replacing the two-bone solver
- solving full finger tracking
- generalizing immediately to all humanoid rigs
- resolving raw OpenXR wrist-vs-palm root semantics unless that is proven to be
  the remaining source of the mismatch
