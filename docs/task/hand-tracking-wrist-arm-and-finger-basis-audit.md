# Audit tracked wrist, arm, and finger coordinate frames

Date: 2026-10-09

Status: runtime palm frame and data-source-aware selection implemented; Focus 3 retest pending.

Release target: [mittens-engine 0.10.0](epic/0.10.0/README.md), as the
prerequisite for shared hand/finger tracking.

## Current report

Controller tracking works. With hand tracking, the tested VRoid wrists appear pitched roughly
90 degrees upward. Treat that observation as the current repro target, distinct from the older
[palm-down wrist kink/jitter report](../bugs/xr-hand-tracking-wrist-kink-and-jitter.md).
No runtime capture was collected during this documentation pass, so the cause is not confirmed.

2026-10-19 update: the testing headset is a Vive Focus 3, and the reported upward wrist rotation
appears when hand tracking takes over from controllers. A synthetic regression reproduces a
separate, concrete source error: the old `MIDDLE_PROXIMAL -> MIDDLE_DISTAL` forward axis follows
finger curl, rotating the wrist target by the finger's bend angle even when the palm stays fixed.
The root now uses `MIDDLE_METACARPAL -> MIDDLE_PROXIMAL`, within the palm. Controller selection,
canonical palm-width axis, and avatar rest-basis correction retain their existing contracts.
Whether this explains the entire Focus 3 handoff offset still requires a hardware comparison.

Follow-up report: rolling the hands reveals an apparent clockwise yaw offset of about 45 degrees.
The primary orientation source is now the runtime palm quaternion, followed by the wrist
quaternion; position-based synthesis remains a fallback. Native OpenXR `-Z` is preserved, while
native `+X` (left) or `-X` (right) supplies canonical palm width `+Y`. This is a frame conversion,
not a fitted correction for the reported angle. Hardware confirmation of the remaining offset
is still needed. The checked-in Bisket rest finger and wrist-to-knuckle directions differ by
about 6.8 degrees, which does not establish a 45-degree rest-basis error.

The scene/model are now identified: `examples/mittens-corp.mms`, using
`assets/models/mitten.glb`. With arms extended, palms down, and fingers forward, both hands
appear tilted inward by about 45 degrees, mirrored left/right. This clarifies the current
symptom as a roll discrepancy. mitten and Bisket have matching imported hand rest geometry;
mitten's hand/finger rest skin matrices reconstruct identity, so inspection does not reveal a
45-degree bind/rest discrepancy. The runtime pose source and palm-frame measurements in that
posture remain necessary to locate the first incorrect frame.

The mitten regression imports its real node/rest transforms, applies the same retained hand
landmarks as the scene's humanoid map, and runs the arm IK with the scene's elbow policy.
Palm-down and mirrored roll poses retain the expected evaluated finger direction and palm
normal under rotated tracking/model roots. This checks retargeting and IK with known input;
the live scene's selected source and runtime joint samples require a separate capture.

The 2026-10-19 capture `/tmp/mittens-hand-alignment-before-source-fix.log` identifies a selection
defect. On SteamVR/OpenXR 2.16.7, each hand briefly selects `WristPalm`, then active emulated
actions take over as `ControllerGripAim` for the steady samples. The profile is reported as
Oculus Touch, while a bound source is named "Hand Tracking Controller". The held-pose samples
show the evaluated hand closely following the selected action target, so changing the palm
frame alone was bypassed by selection. The user relaxed the reference pose near the end;
the final samples are not a neutral-pose measurement.

SteamVR advertises `XR_EXT_hand_tracking_data_source`. The engine now enables it, explicitly
requests both `UNOBSTRUCTED` and `CONTROLLER` at tracker creation, and locates the source state
in the same output chain as the joints. A valid unobstructed root wins over emulated actions;
controller-derived or unknown joints preserve controller priority. Each hand's source and root
are cleared together on loss/errors. No interaction-profile or localized-name heuristic is used.
See [the data-source contract](https://registry.khronos.org/OpenXR/specs/1.1/html/xrspec.html#XR_EXT_hand_tracking_data_source).
The new source log and a hardware retest must confirm which data source SteamVR reports and
whether selecting the palm resolves the visible inward roll.

This audit gates the wrist frame contract for
[shared hand/finger tracking](hand-tracking-system-and-finger-retargeting.md).

## What the current code actually does

| Stage | Current behavior |
| --- | --- |
| Controller `GripAim` | Grip translation and aim quaternion from matching generations |
| Pose ownership | Valid unobstructed root first; controller-derived/unknown joints retain action priority |
| Tracked hand orientation | Runtime palm quaternion, then wrist quaternion, then landmark fallback |
| Tracked hand forward | Native `-Z`; fallback is `MIDDLE_METACARPAL -> MIDDLE_PROXIMAL` |
| Tracked hand second axis | Native `+X` for left, `-X` for right; fallback is projected little-to-index width |
| Tracked hand origin | Valid wrist position, else valid palm position |
| Avatar basis | Authored `JointRetargetBasis`, or humanoid-map-derived equivalent |
| AVC visual target | Applies `target_rest_to_canonical` for Aim, GripAim, and WristPalm sources |
| Forearm IK | Extracts twist using target world rotation and immutable hand rest rotation |
| Laser mount | `RestAttachment` rest offset, with retained basis orientation handled separately |

The canonical second axis remains palm width, not a palm normal. Runtime OpenXR `+Y` points out
of the back of the hand, so it must be remapped before applying the retained avatar basis.
The landmark fallback and map-derived avatar basis use matching width definitions. Controller
aim is a runtime pointer frame; its relationship to an anatomical palm frame must be verified
separately. See the
[OpenXR joint conventions](https://registry.khronos.org/OpenXR/specs/1.1/html/xrspec.html#convention-of-hand-joints).

The [frame tests](../../src/engine/ecs/system/openxr_system.rs) cover native orientation selection,
per-side conversion, independent position/orientation validity, invalid-quaternion fallback,
and yaw/roll poses with deliberately skewed landmarks. The landmark fallback still asserts that
canonical `+Y` maps to the width direction. These synthetic tests do not establish agreement
with the actual runtime controller aim or skinned avatar posture.

The root no longer follows middle proximal-to-distal finger curl or requires distal landmarks.
Regression coverage includes both hand-width directions, translated/rotated palms, open-to-fist
finger angles, and position-only palm fallback. Missing landmarks no longer invalidate a root
with a usable palm or wrist orientation. Palm position fallback still changes the origin and
does not synthesize a wrist location.

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
   `[OpenXR][hand-frame]` now samples native palm/wrist and selected canonical quaternions, the
   orientation source joint, reported data source, and relative landmark-frame axis/angle
   every 120 XR frames.
   Step-angle diagnostics compare consecutive selected root frames, rather than comparing a
   previous canonical frame with a current native quaternion.
   AVC additionally logs target and observed palm-back normals in world space and their
   orientation error. The observed bone is sampled before this tick's IK; hold the reference
   pose still when comparing it with the target.
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

Finger-curl coupling is confirmed by a synthetic regression and corrected. The other hypotheses
remain open; source inspection does not establish which explains the full reported upward pitch.

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
