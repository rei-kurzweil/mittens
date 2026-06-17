# AVC right-arm TwoBoneIK bends inward instead of outward

## Status

Open bug / investigation note.

## Symptom

When a humanoid `GLTFComponent` is wrapped by `AvatarControlComponent` and driven through the
usual `InputComponent` or `InputXRComponent` parent setup, the arm IK is asymmetric:

- the left arm bends in the expected direction, with the elbow moving outward away from the torso
- the right arm bends toward the avatar's centerline, as if it is using the same lateral bend
  direction as the left arm

Visually, the right elbow collapses inward toward the chest/ribs instead of flaring outward on
the avatar's right side.

## Expected behavior

For a normal humanoid rig, both arms should bend anatomically outward relative to their own side:

- left elbow should bias outward on the avatar's left side
- right elbow should bias outward on the avatar's right side

The right arm should not reuse a bend direction that effectively points leftward in world/body
space.

## Current repro

Primary repro:

- [examples/bisket-vr-demo.mms](../../examples/bisket-vr-demo.mms)

Typical setup shape:

- `InputComponent` or `InputXRComponent`
- child `AvatarControlComponent`
- child `TransformComponent` model root
- child `GLTFComponent` humanoid armature

Observed while AVC creates and drives the built-in hand/arm IK chains for the humanoid model.

## What currently looks wrong

This does not look like a generic "2-bone IK is unstable" problem. It looks specifically like
the right-arm elbow hint is mirrored incorrectly, not mirrored at all, or interpreted in the
wrong space.

The important comparison is:

- left arm: elbow moves away from the body and looks plausible
- right arm: elbow moves in the same lateral direction as the left arm, which is inward for the
  avatar's right side

That strongly suggests the right-arm pole configuration used by AVC is wrong for the intended
humanoid setup.

## Likely cause

Most likely causes:

- AVC is installing the wrong `pole_direction` for the right arm
- the right-arm pole hint has the wrong sign on the lateral axis
- the pole hint is authored in world space when it really needs to be mirrored or transformed from
  avatar/body-local space
- the solver or AVC wiring is treating left/right arm defaults as if they were interchangeable

Relevant current defaults already suggest this area:

- [src/engine/ecs/component/avatar_control.rs](../../src/engine/ecs/component/avatar_control.rs)
- [src/engine/ecs/system/avatar_control_system.rs](../../src/engine/ecs/system/avatar_control_system.rs)
- [src/engine/ecs/system/ik_system.rs](../../src/engine/ecs/system/ik_system.rs)
- [docs/spec/ik-system.md](../spec/ik-system.md)

## Investigation targets

- verify the exact `right_arm_pole_direction` value AVC installs on the runtime `IKChain`
- confirm whether the right-arm pole is being interpreted in world space or should instead be
  derived from avatar/body-local space each tick
- compare left and right arm setup at AVC init to make sure the right side is not accidentally
  reusing the left-side bend convention
- check whether the solver's elbow-plane construction needs side-aware mirroring beyond the raw
  configured pole vector

## Likely fix direction

The right-arm TwoBoneIK configuration installed by AVC should be flipped so the elbow biases
outward on the avatar's right side.

At minimum, this likely means:

- correcting the right-arm pole-direction config AVC uses for humanoid models, or
- converting per-arm pole hints from avatar/body-local space into world space before solving

The key rule is simple: the right elbow should solve away from the torso, not inward through it.
