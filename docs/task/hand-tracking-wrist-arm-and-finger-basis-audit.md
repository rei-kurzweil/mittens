# Audit tracked wrist, arm, and finger coordinate frames

Date: 2026-10-09

Status: investigation planned; source inspection complete, runtime diagnosis pending.

Release target: [mittens-engine 0.10.0](epic/0.10.0/README.md), as the
prerequisite for shared hand/finger tracking.

## Current report

Controller tracking works. With hand tracking, the tested VRoid wrists appear pitched roughly
90 degrees upward. Treat that observation as the current repro target, distinct from the older
[palm-down wrist kink/jitter report](../bugs/xr-hand-tracking-wrist-kink-and-jitter.md).
No runtime capture was collected during this documentation pass, so the cause is not confirmed.

This audit gates the wrist frame contract for
[shared hand/finger tracking](hand-tracking-system-and-finger-retargeting.md).

## What the current code actually does

| Stage | Current behavior |
| --- | --- |
| Controller `GripAim` | Grip translation and aim quaternion from matching generations |
| Tracked hand orientation | Synthesized from joint positions, not raw wrist/palm quaternion |
| Tracked hand forward | `MIDDLE_PROXIMAL -> MIDDLE_DISTAL` maps to canonical `-Z` |
| Tracked hand second axis | Projected `LITTLE_PROXIMAL -> INDEX_PROXIMAL` maps to canonical `+Y` |
| Tracked hand origin | Valid wrist position, else valid palm position |
| Avatar basis | Authored `JointRetargetBasis`, or humanoid-map-derived equivalent |
| AVC visual target | Applies `target_rest_to_canonical` for Aim, GripAim, and WristPalm sources |
| Forearm IK | Extracts twist using target world rotation and immutable hand rest rotation |
| Laser mount | `RestAttachment` rest offset, with retained basis orientation handled separately |

The canonical second axis is currently palm width, not a palm normal. The OpenXR synthesis and
map-derived avatar basis intentionally use matching landmark definitions. Do not infer a bug from
the variable name `up`, or replace it with a normal on one side alone. Controller aim is a runtime
pointer frame; its relationship to an anatomical palm frame must be verified separately.

The [existing synthesis test](../../src/engine/ecs/system/openxr_system.rs) asserts that canonical
`+Y` maps to the width direction. It proves the implemented landmark convention, not agreement
with runtime controller aim or anatomical wrist posture.

The middle proximal-to-distal vector changes as the finger curls. That is a concrete weakness to
investigate for wrist tracking: finger articulation should not pitch the whole hand. Also, loss
of the required finger landmarks currently invalidates root synthesis even if wrist pose is valid.
Fallback to palm changes position origin; it does not use palm quaternion or fix orientation.

## Existing work to preserve

- [JointRetargetBasis](../spec/joint-retarget-basis-component.md) derives a full two-axis correction
  from immutable imported rest geometry. Bone-local X/Y/Z are not anatomical bend/splay/twist axes.
- [HumanoidBoneMap](humanoid-bone-map-automapping-and-mms-presets.md) retains semantic arm/hand
  bindings and supplies automatic hand bases; authored bases are expert overrides.
- [RestAttachment](../spec/rest-attachment-component.md) isolates pointer origin from joint
  orientation. The driven `T` stays directly beneath `XRHand`; the attachment wraps pointer content.
- [IKSystem](../../src/engine/ecs/system/ik_system.rs) already performs forearm twist follow using
  immutable end-hand rest rotation. This is beyond the early raw-target discussion in
  [the forearm follow-up](avc-forearm-roll-visual-hand-offset-alignment.md).

Those older tasks contain historical `CTLXR`, grip-offset, and early pipeline descriptions. Read
them for symptoms and rationale; use current source and imported rest data for the audit.

## Investigation procedure

1. Capture the exact scene, model URI, runtime/headset, active pose source per hand, authored IK
   preset, humanoid map report/generation, and any explicit basis override. Start with
   [Bisket debug](../../examples/bisket-vr-debug.mms) and
   [VTuber mirror](../../examples/vtuber-mirror-example.mms), then reproduce on the user's actual rig.
2. Enable existing `CAT_OPENXR_DEBUG=1` and `CAT_DEBUG_XR_HAND_ALIGNMENT=1`. Record controller and
   tracked-hand runs in the same physical posture. Existing logs are a starting point; planned
   instrumentation must also capture real evaluated targets/bones, not only correction products.
3. Draw/record raw wrist and palm axes, synthesized forward/width/normal, raw controller aim/grip,
   raw and visual AVC target axes, upper/lower/hand bone world axes, and pointer direction/origin.
   State which space each value belongs to and correlate one sample/queue generation.
4. Inspect immutable rest transforms for shoulder, upper arm, lower arm, hand, all finger chains,
   and intervening helpers. Compare geometric segment direction with bone-local axes; check model
   root rotation/scale, mirrored transforms, and inverse-bind matrices separately from live pose.
5. Reconstruct the correction algebra and check its inverse/sign/composition at every boundary.
   Verify the map-derived and authored hand bases agree when using identical landmarks, and that
   the correction is applied exactly once before both forearm twist and hand end rotation.
6. Check queue flush/transform propagation ordering so IK reads the current corrected target,
   not a cached prior frame. Trace the actual target bound to each `IKChain`.
7. Compare raw OpenXR joint orientation normalized according to the specification with stable
   palm-landmark synthesis. Evaluate wrist-to-MCP and MCP geometry as alternatives to a curling
   middle-finger segment. Specify origin correction if palm fallback must represent a wrist target.
8. Reproduce rest pose, palms down/up/inward, 90-degree pronation/supination, elbow bend, wrist
   flexion/extension, and fist/open transitions. Repeat both sides and controller/hand switching.
9. Test at least one rig with different authored bone axes. Separate source-frame errors from
   target-rest errors and forearm deformation limits; a VRoid-specific Euler offset is not evidence
   of a general correction.

Use rotation matrices/quaternions and geometric vectors for conclusions. Euler angles in an
arbitrary imported local basis can label the same anatomical motion as different axes.

## Hypotheses to distinguish

| Hypothesis | Evidence that would support it |
| --- | --- |
| Controller aim and landmark canonical frames disagree | Fixed discrepancy before avatar correction in equivalent physical postures |
| Tracked forward follows finger curl | Wrist target pitches while palm/MCP frame stays stable during a fist gesture |
| Imported rest basis is wrong or overridden | Synthesized source frame is correct but corrected target disagrees with rest landmark reconstruction |
| Correction inverse/order or double application is wrong | A basis round-trip fails, or an extra fixed rotation appears at a specific consumer |
| IK reads raw/stale target | Visual target is correct but evaluated hand/forearm differs by source or generation |
| Runtime tracking or rig deformation is responsible | Correct target/bone frames still produce jitter or skin kink, isolated to provider or skinning |

All are hypotheses. The current code inspection does not establish which explains the reported
90-degree upward pitch.

## Deliverables and acceptance

- [ ] A captured controller versus hand comparison identifies the first incorrect frame.
- [ ] A documented source-to-canonical and canonical-to-bone contract covers both hands.
- [ ] Open/fist transitions leave wrist/palm frame stable; missing landmarks have explicit fallback.
- [ ] Forearm twist and wrist rotation consume the same evaluated visual target frame.
- [ ] Rest reconstruction works with arbitrary authored arm/finger axes and translated/rotated roots.
- [ ] Runtime validation removes the upward wrist pitch without regressing controller poses.
- [ ] Palm-up/down and left/right mirrored motion have no fixed unintended roll offset.
- [ ] Static rest pointer placement remains correct; live fingertip attachment is specified separately.

When implementation begins, add regression fixtures for the confirmed failing frame and retain
hardware screenshots/logs for the actual model. Synthetic tests alone cannot validate a skinned
avatar's visible wrist deformation.
