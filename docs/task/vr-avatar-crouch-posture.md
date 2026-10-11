# Task: VR avatar crouch posture and standing-height calibration

## Status

Planned authored-posture follow-up. The first crouch implementation is now
[AVC crouch IK from predicted foot penetration](avc-foot-penetration-crouch-ik.md):
automatic mapped-leg solving while physical HMD Y remains unconstrained.
The authored-pose plan below is retained for later posture polish after
transitionable avatar pose layers; it no longer gates the initial crouch solver.

## Outcome

When the player lowers their real head relative to a calibrated standing
height, the Bisket avatar smoothly adopts an authored crouch posture while
keeping the tracked head/camera aligned.  Returning to standing restores the
standing pose without torso crushing, feet floating, or abrupt snapping.

## Why crouch comes first

Crouching validates the essential embodiment contract with one unambiguous
input: vertical headset-height change.  It exercises calibration, pose-layer
transitions, tracking ownership, and body anchoring without conflating them
with horizontal movement speed, stride phase, or root motion.

## Design constraints

- Calibrate standing HMD height only after a valid XR pose is available; make
  recalibration explicit rather than silently changing the baseline.
- Derive a normalized crouch amount from height below that baseline, using a
  deadband/hysteresis and smoothing sufficient to avoid tracker-noise flicker.
- Drive an authored Bisket crouch pose through the pose-layer API.  Do not
  procedurally scale/compress the torso or solve crouching with spine IK.
- Preserve the HMD-driven head/eye relationship and the current anatomical
  body-anchor rule.  Crouch changes body posture, not the eye-to-head
  calibration or raw HMD coordinate convention.
- State the policy for unsupported depths: clamp to a maximum crouch before
  introducing kneel/sit as separate authored states.

## Deliverables

1. Capture/author a Bisket standing reference and crouch pose, scoped to the
   body joints that should be animation-owned.
2. Add calibrated standing-height state and a testable crouch-amount resolver.
3. Bind the crouch pose to the avatar and transition its weight continuously.
4. Add an XR/debug repro that can simulate standing, partial crouch, full
   crouch, and return-to-standing from controlled HMD heights.
5. Define acceptance for real-headset validation, including pitch/roll while
   crouched and ordinary locomotion-free head movement.

## Acceptance scenarios

1. A small height fluctuation near standing does not visibly toggle the pose.
2. Lowering the HMD smoothly transitions into crouch; raising it reverses from
   the current blend value rather than snapping.
3. Looking down while standing or crouched does not falsely create crouch.
4. The HMD/camera and visible tracked head remain aligned throughout.
5. On tracker loss or avatar removal, no pose-layer state leaks to another
   instance.

## Related work

- [Transitionable avatar pose layers](avatar-pose-transition-layers.md)
- [Avatar control simple humanoid body follow](avatar-control-simple-humanoid-body-follow.md)
- [VR avatar embodiment epic](epic/vr-avatar-embodiment.md)

