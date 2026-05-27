# Avatar body follow targets HMD center instead of head-pivot XZ

## Status

Open bug / investigation note.

## Symptom

In the current AVC VR body-follow path, the body is positioned under the HMD center in X/Z rather than under the avatar's head pivot / neck base in X/Z.

This looks "almost right" in neutral pose, but breaks down as soon as the head rotates:

- the body drifts out from under the head
- the neck appears to stretch
- tuning `body_to_head_offset(...)` can make the mismatch easier to see, but does not solve the underlying target-choice problem

Observed behavior matches the idea that the body anchor is being driven from the headset center rather than from the actual head-bone pivot location the skeleton expects.

## Expected behavior

The body should anchor under the head-bone root / skull-base pivot in X/Z, not under the HMD center.

More concretely:

- the visible head can still track the HMD pose
- the body root should land under the projected head pivot / neck-base position in world X/Z
- head rotation should not cause the neck to visually stretch away from the torso

## Current repro

Primary repro:

- [examples/bisket-vr-demo.mms](../../examples/bisket-vr-demo.mms)

Useful comparison case:

- [examples/vtuber-desktop.mms](../../examples/vtuber-desktop.mms)

The VR path shows the mismatch most clearly when pitching or otherwise rotating the head while watching the relative alignment of the torso, neck, and head.

## What currently looks wrong

The current body-follow heuristic is targeting the wrong point.

Current shape of the system:

- `AvatarControlSystem` creates a dedicated head target / splice path for the head under the driver pose
- `IKSystem` still handles the head `AimConstraint`
- `HeadPoseBodyXzFollowSystem` now aligns the body from the driver / HMD world translation plus a local body offset

That means the body root and the head pivot are no longer guaranteed to be solving toward the same physical point.

Using the HMD center as the body anchor target sounds reasonable at first, but the skeleton does not care about the HMD center. The skeleton cares about the head-bone pivot and the neck chain beneath it.

## Why this causes stretching

If the body is placed under headset center, but the head solver is placing / orienting the visible head around the head-bone pivot, then the torso-neck-head chain is solving against two different anchors:

- body anchor = HMD center X/Z
- head anchor = head target / head-bone pivot convention

When the head rotates, those two anchors diverge.

That disagreement shows up visually as:

- the body no longer sitting under the head
- the neck translating or stretching to absorb the difference

This is especially obvious in VR because the HMD position is not the same thing as the skull-base pivot the armature is authored around.

## Comparison with vtuber-desktop

The comparison with [examples/vtuber-desktop.mms](../../examples/vtuber-desktop.mms) is useful because it shows that `AimConstraint` itself is not obviously enough to explain the bug.

Desktop behavior can still look correct when the head rotates, which suggests the main issue is not simply "AimConstraint math is broken".

The stronger explanation is that AVC VR body-follow is choosing the wrong target point for body X/Z alignment.

## Likely cause

The likely bug is that body-follow currently targets HMD center X/Z when it should target the world X/Z of the avatar's head pivot or neck-base estimate.

Possible target definitions, in increasing order of correctness:

- bad target: raw `driven_t` / HMD center world X/Z
- better debug target: current solved head-bone pivot world X/Z
- intended target: head-pivot or neck-base world X/Z derived from the same convention the head solver uses

The bug is not just a missing constant offset. A constant `body_to_head_offset` can help diagnose the mismatch, but head rotation changes the error direction in world space, which is why the neck still stretches under rotation.

## Investigation targets

- [src/engine/ecs/system/avatar_control_system.rs](../../src/engine/ecs/system/avatar_control_system.rs)
- [src/engine/ecs/system/ik/head_pose_body_xz_follow.rs](../../src/engine/ecs/system/ik/head_pose_body_xz_follow.rs)
- [src/engine/ecs/system/ik_system.rs](../../src/engine/ecs/system/ik_system.rs)
- [src/engine/ecs/component/ik_chain.rs](../../src/engine/ecs/component/ik_chain.rs)

Questions to answer:

- what exact point is the head `AimConstraint` treating as the effective head anchor: HMD center, head-bone pivot, or pivot plus local offset?
- should body-follow use the solved head-bone pivot world X/Z directly as its target in v0?
- should the final target instead be a neck-base estimate derived from the same head-local offset used by the head solver?
- is `copy_position` on the AVC-installed `AimConstraint` making the head chase a target that is inconsistent with the current body anchor?

## Likely fix direction

The body-follow target should be changed from HMD center X/Z to the same head-pivot convention used by the head solver.

That likely means one of these:

- derive body X/Z from the solved head-bone pivot world position
- derive both head target and body target from one shared neck-base / head-pivot calculation, so there is a single source of truth

The important rule is that body placement and head placement must use the same anatomical anchor, not two nearby but different points.

## Notes

`body_to_head_offset(...)` on `AvatarControl` is useful as a temporary tuning and debugging aid, but it does not address the underlying target mismatch by itself.