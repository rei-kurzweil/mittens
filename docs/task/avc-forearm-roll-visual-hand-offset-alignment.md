# Task: AVC forearm roll should follow the visual hand frame, not just the raw hand target

Date: 2026-06-28

Status: active follow-up after the initial wrist-kink reduction.

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
