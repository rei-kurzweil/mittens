# Epic: VR avatar embodiment and HMD pose interpretation

## Outcome

An avatar responds coherently to a player's physical head motion.  The engine
uses one explicit interpretation of the HMD pose to place the visible head and
body, and can retain short pose history where a consumer must relate data from
different instants.  Looking around must not create body drift, neck stretch,
or opposite-direction eye motion; actual locomotion must still move the body.

This is an integration tracker.  It does not turn `AvatarControl` into a
generic constraint graph, and it does not merge eye-tracking transport policy
with body-follow policy.

## Current checkpoint

- Native flat-support crouch IK is implemented (2026-10-21), with explicit sole
  landmarks configured in Rei(mu), mittens-corp and bisket-vr-demo. It shares the
  retained collidable BVH, preserves tracking/locomotion ownership and suspends
  while mounted. Automated coverage is in place; headset acceptance remains
  pending. See [crouch task](../avc-foot-penetration-crouch-ik.md).

- AVC already derives a displaced head pivot from the HMD pose and calibrated
  eye-to-head offset. Native `HeadNeckBodyConstraintSystem` now replaces simple
  body follow for eligible rigs, placing the body from the bent neck geometry.
  It follows positional input immediately; bounded planar follow remains planned.
  Desktop look keeps the intentional body anchor fixed and derives the head arc.
  Automated checks pass for the supported fixture/rest paths; headset acceptance
  and tuning remain pending.
- The body-follow task specifies the desired kinematic rule:

  ```text
  neck_base_world = hmd_world_translation - hmd_world_rotation * head_to_neck_local
  ```

  This removes HMD translation caused only by rotating about the neck, while
  retaining translation caused by walking or leaning.
- Automatic eye tracking currently consumes the latest retained gaze vector
  without a paired HMD pose time.  Head angular velocity strongly correlates
  with the avatar eyes rotating oppositely, so the leading diagnosis is a
  temporal mismatch between gaze and HMD samples—not a fixed axis/basis error.

## Shared foundation: HMD pose state and history

Publish a small, engine-owned HMD pose-state/history facility at the point
where OpenXR produces authoritative head poses.  It should retain, for each
sample:

- monotonic engine receive/sample time;
- world position and orientation;
- sequence number; and
- optionally source/runtime timestamp when OpenXR or a transport provides one.

The facility must support bounded lookup/interpolation of orientation and
position at a requested time.  It is data infrastructure, not an inference
that every HMD movement is locomotion.

Consumers keep their own policies:

| Consumer | Uses pose state/history for | Policy |
| --- | --- | --- |
| AVC body follow | current HMD pose and calibrated HMD-to-neck/head-pivot offset | derive the anatomical body anchor; body retains yaw-only follow |
| Eye tracking | HMD orientation at the gaze sample's effective time | temporally align gaze before converting to eye-local rotation |
| Future locomotion/animation | pose deltas after subtracting the rotation-induced offset motion | choose animation, smoothing, or root-motion behavior explicitly |

Do not use raw angular velocity as an eye correction multiplier.  It is useful
diagnostic evidence and can help estimate delay, but the correction must be
based on poses from known or calibrated times.

## Workstreams

1. **Anatomical head/body anchoring** — preserve and validate the shared
   head-pivot convention across AVC, camera alignment, neck behavior, and body
   follow.  The body must not be targeted at HMD center.
2. **Gaze/HMD temporal alignment** — establish the eye transport's coordinate
   convention and latency, then pair gaze with the corresponding HMD pose.
   A static `CancelHeadRotation` setting is not the selected remedy unless
   controlled evidence proves a static coordinate-space mismatch.
3. **Pose-history API and diagnostics** — provide bounded history plus the
   recording needed to inspect HMD pose, angular velocity, gaze arrival, and
   final eye/body results together.
4. **Future locomotion semantics** — only after the above is validated,
   consider behavior that distinguishes walking/leaning from head-only motion.
   This is an animation/root-motion policy, not a prerequisite for correct
   head-pivot body anchoring.

## Posture and locomotion delivery order

The generic transition runtime already supports transform and emissive
interpolation, but it does not yet provide a persistent transitionable avatar
pose layer. The initial procedural crouch does not depend on that layer.
Delivery proceeds in this order:

First complete the focused phase of
[transform operator API cleanup](../transform-operator-api-cleanup.md): grouped
constructors, builder configuration, and explicit child-output composition,
including the translation sample. Then complete the
[capsule operator/calibration change](../avc-capsule-before-hmd-tracking.md), using
[axis-selective translation sampling](../transform-sample-translation-axes.md):
retain tracked body X/Z and sample pre-tracking locomotion Y. Physical head Y
does not drive grounding correction. This is the grounding prerequisite for
the posture work below.

1. [AVC crouch IK from predicted foot penetration](../avc-foot-penetration-crouch-ik.md)
   — separate tracking from grounding, then automatically solve mapped legs
   for Rei(mu), Bisket, and supported humanoids without constraining HMD Y.
2. [Transitionable avatar pose layers](../avatar-pose-transition-layers.md)
   — bind a pose to one GLTF instance and transition its blend weight while
   respecting tracked-bone ownership.
3. [VR avatar crouch posture and standing-height calibration](../vr-avatar-crouch-posture.md)
   — add authored posture polish with explicit ownership alongside the initial
   leg constraint; this precedes walking.
4. [Bisket walking locomotion animation](../bisket-walking-locomotion-animation.md)
   — turn validated Bisket pose assets into idle/walk/run behavior based on
   locomotion, not head-only HMD motion.

## Linked work

- [XR head/neck bend and derived body offset](../xr-head-neck-bend-and-body-offset.md)
  — proposed bounded neck flexion for looking down at the body; derive body
  placement from the neck arc while preserving the tracked head/view. Includes
  operator feasibility, state requirements, and grounding/arm ordering. The
  2026-10-16 follow-up adds a retained X/Z lean allowance, with positional neck
  closure and earlier body follow wherever anatomical reach requires it. Intended
  as default AVC behavior through native `HeadNeckBodyConstraint` /
  `HeadNeckBodyConstraintSystem`, replacing the current body-follow constraint
  for eligible avatars. VMN is a separate future authoring/evaluation facility,
  not a prerequisite. Both drivers use
  one per-rig follow threshold containing the rotational motion envelope; desktop
  look stays inside it, while XR positional excess can trigger follow.
- [Transform operator API and output-contract cleanup](../transform-operator-api-cleanup.md)
  — grouped constructors/builders and uniform downstream-child composition;
  the focused phase precedes the capsule height fix; remaining specialized
  migrations can follow separately.
- [Avatar control simple humanoid body follow](../avatar-control-simple-humanoid-body-follow.md)
  — canonical kinematic body-anchor design and current implementation path.
- [Eye tracking appears to counteract head rotation](../../bugs/eye-tracking-head-rotation-counteracts-gaze.md)
  — hardware repro, transport diagnosis, and the temporal-alignment fix.
- [Eye and face tracking epic](eye-and-face-tracking.md)
  — tracker transport, calibration, and avatar mapping work outside the
  timing-specific bug.
- [Avatar-control head-driven redesign](../avatar-control-head-driven-redesign.md)
  — head mount, eye-offset, and head/body ownership design history.
- [CameraXR/avatar alignment audit](../../analysis/cameraxr-avatar-alignment.md)
  — current pose data flow and known alignment trade-offs.
- [XR avatar pose grounding analysis](../../analysis/xr-avatar-pose-grounding.md)
  — broader grounding and eventual full-body-IK alternatives.

## Acceptance scenarios

1. With the player stationary, pitch, roll, and yaw the HMD: the visible head
   follows, while the body anchor stays at the derived neck/head-pivot location
   and the neck does not visibly stretch.
2. Walk or lean while holding orientation: the body anchor follows the true
   HMD translation according to the calibrated pose-compensation rule.
3. Hold gaze neutral relative to the head while rotating the HMD: the avatar
   eyes do not counter-rotate during motion or after it settles.
4. Make intentional left/right/up/down gaze during the same rotations: gaze
   direction remains correct and is not inverted by temporal alignment.
5. Diagnostics can show the raw gaze sample, its effective time, matched HMD
   pose, current HMD pose, and final eye-local rotation for a headset capture.

## Boundaries

- HMD pose history is shared infrastructure; source-specific eye transport
  settings stay on their tracker components.
- A calibrated anatomical offset is a pose relationship, not an estimate based
  solely on velocity.
- Full-body IK, crouch/kneel inference, and locomotion animation are follow-on
  work.  They must not block the head-pivot or eye-timing fixes.
