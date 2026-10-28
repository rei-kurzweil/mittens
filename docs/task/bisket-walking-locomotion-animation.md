# Task: Bisket walking locomotion animation

## Status

The procedural lower-body path is now implemented separately in
[procedural lower-body walking IK](procedural-lower-body-walking-ik.md).
The authored animation work below remains planned.

Planned after crouch posture.  Bisket already has captured running-pose
samples and a discrete demonstration loop; this task turns validated assets
and pose-layer behavior into a locomotion presentation policy.

Desktop planning entry: [Locomotion and whole-armature animation](../desktop/locomotion-and-armature-animation.md).

## Outcome

Bisket presents a convincing idle/walk/run animation appropriate to avatar
locomotion, with smooth starts, stops, and speed changes, while VR head and
hand tracking remain authoritative.

## Prerequisites

- [Transitionable avatar pose layers](avatar-pose-transition-layers.md) is
  complete and supports interruption/replacement without snapping.
- [VR avatar crouch posture and standing-height calibration](vr-avatar-crouch-posture.md)
  is validated on Bisket.  Walking must compose with the resolved crouch
  policy rather than invent a second posture owner.
- The HMD/body anchor behavior in the embodiment epic is hardware-validated.

## Scope

1. Audit the existing captured Bisket assets:

   ```text
   000-relaxed.pose.mms
   001-running_0.pose.mms
   002-running_1.pose.mms
   003-running_2.pose.mms
   ```

   Decide whether they form a usable loop and whether additional walk, idle,
   and transition poses must be captured.
2. Define the locomotion signal: use the avatar/root movement actually chosen
   by locomotion policy, not raw HMD wobble caused by head rotation.
3. Define idle/walk/run thresholds, hysteresis, stride phase, and the policy
   for in-place versus root-motion animation.
4. Blend body-pose layers for idle, gait, and crouch while excluding bones
   owned by head/eye/controller tracking.
5. Provide a Bisket VR and desktop validation scene with observable locomotion
   state and pose/phase diagnostics.

## Non-goals

- inferring foot contacts or building full-body IK;
- using raw HMD angular velocity as walking evidence;
- letting animation overwrite live tracked head, eyes, or hands; or
- solving crouch for the first time as part of gait work.

## Acceptance scenarios

1. Start and stop walking without a pose snap or a frozen partial stride.
2. Vary speed through walk/run thresholds without rapid state chatter.
3. Look around while standing in place: animation remains idle.
4. Walk while looking around: gait responds to locomotion, while head and
   tracked hands retain their expected motion.
5. Crouch then move: the documented crouch/gait composition remains stable.

## Related work

- [VR avatar crouch posture and standing-height calibration](vr-avatar-crouch-posture.md)
- [Movement-driven Bisket pose example](../../examples/gltf-pose-animation.mms)
- [VR avatar embodiment epic](epic/vr-avatar-embodiment.md)
