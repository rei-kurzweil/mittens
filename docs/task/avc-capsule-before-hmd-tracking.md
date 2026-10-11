# Task: Place the AVC collision capsule before HMD tracking

## Status and outcome

Planned; documentation only. Move the XR avatar's generated collision capsule
out of the head-driven model branch and attach it to the simulated locomotion
frame, alongside the HMD tracking branch. Keep grounding/static-contact
correction on the existing physics/locomotion response target.

Physical HMD translation must remain free inside the constrained frame.
Gamepad motion, scripted movement, Velocity, gravity, and floor/wall contact
still affect that frame. This is the prerequisite for
[foot-penetration crouch IK](avc-foot-penetration-crouch-ik.md), not part of the
crouch solver itself.

## Problem and current code

`avatar_control_system.rs::try_init_or_route_capsule` currently creates a
translation-only TransformForkTRS beneath `model_root_id`. The model root
follows the displaced tracked head in XYZ. Consequently, contact correction
responds to physical head movement by displacing the outer locomotion root,
which also carries the camera. A crouch can therefore raise the tracking
origin and cancel the user's movement.

The response target is appropriate. The proxy's pose source is the problem.
[The investigation](../bugs/mittens-corp-hmd-height-floor-contact-feedback.md)
records the baseline hardware report and an isolated downward-cancellation
probe; the full bidirectional headset symptom is not yet reproduced in a test.

## Target hierarchy

Conceptual topology when physics and gamepad have separate transforms:

```text
physics / grounding transform                 <- Velocity + contact response
└─ gamepad / simulated locomotion transform    <- capsule pose source
   ├─ runtime capsule branch
   │  └─ upright capsule + calibrated offset
   └─ InputXR
      └─ HMD tracking transform
         └─ AVC + head-driven avatar / camera
```

The capsule branch is structurally a sibling of InputXR, and therefore outside
its driven HMD transform. InputXR is a component boundary, not an extra physical
motion layer. Keep a translation-only stream within the runtime capsule branch
if necessary to preserve the upright, world-up capsule contract; do not blindly
inherit carrier pitch/roll or scale into the collision shape.

In `mittens-corp`, physics and gamepad already share
`rei_mu_locomotion_root`, so both branches belong beneath that existing root.
Do not add an unnecessary motion layer or move AVC out of its tracking branch.

## Separate pose source from response target

Resolve two roles explicitly:

- **Capsule carrier:** the composed simulated frame immediately before the
  owning InputXR applies tracked pose. This must inherit all simulated motion,
  including an inner gamepad transform when physics acts on an outer transform.
- **Contact response target:** the transform that receives penetration
  correction. Retain `AVC.movement_target(...)` and existing automatic routing.
  The target may be an ancestor of the carrier rather than the carrier itself.

For the normal XR hierarchy, discover the carrier from the owning InputXR's
pre-tracking ancestry. Validate it as a live transform frame and validate that
moving the response target carries the proxy and tracking branch. An explicit
movement target must not silently become the carrier when doing so would omit
an inner motion layer. Unsupported/disconnected routing stays unready with a
diagnostic rather than retaining a stale pose source.

The selected contract excludes all tracked HMD translation from capsule pose,
including physical X/Z. Physically walking or leaning can therefore move the
visible head/body away from the capsule; this is expected under this boundary.
Future room-scale collision, proxy recentering, or tracking fades require a
separate explicit policy. Do not quietly reintroduce head-following on X/Z.

Limit this migration to XR first. Desktop has no equivalent HMD boundary;
preserve its existing pose-source/response routing and verify that behavior.

## Calibrated offset and readiness

Keep the existing CPU-bounds-based shape inference and authored capsule radius.
Convert the shape dimensions and calibrated placement to a consistent world
metric; moving the branch must not change its measured height or apply model
scale twice. Record bounds provenance and reject unsupported scale with a
diagnostic. The calibrated capsule bottom must match the intended standing
body/ground relation, not the current frame's tracked head height.

Retain a stable placement offset relative to the simulated carrier. Establish
it from a known standing calibration/model relation or an explicit bottom
offset, with valid tracking if the chosen calibration requires it. Do not bake
an arbitrary crouched startup pose into the capsule, assume every floor is
world Y = 0, or recalculate the offset from the live avatar each frame.
Recalibration is explicit; ordinary HMD changes and recentering do not silently
change capsule geometry or placement.

`CapsuleReady` must wait for usable dimensions, calibration, valid carrier and
response routing, and propagated world pose. Preserve its settled-frame event
behavior and scene gravity gating. Do not report readiness merely because the
runtime Zone exists. Ensure the authored `collision_disabled()` opt-out remains
effective and document any provisional calibration API before exposing it.

## Lifecycle and scheduling

The capsule moves outside AVC's subtree, so recursive avatar deletion will no
longer automatically remove its helpers. Track the complete generated branch
as owned runtime state. Avatar/GLTF removal, opt-out, carrier loss, and rig
replacement must retire or rebuild it without deleting unrelated siblings.
Keep helpers excluded from serialization and prevent duplicate branches on
retries. Preserve two-avatar isolation and mapping/asset invalidation behavior.

Reconcile carrier/routing changes after external reparenting or mount topology
changes. Normal car mounting carries the same simulated frame and both branches
together. Keep the existing mounted-contact exclusion; dismount must restore
pedestrian contact without recalibrating against the vehicle or head pose.
Discard obsolete sweep/contact history when a proxy is replaced or its frame
changes discontinuously, including teleport, so old positions do not create
false floor crossings.

The capsule must be current before each existing pose-only or fixed-step contact
pass. Simulated root changes propagate to both branches; physical tracking
updates affect only the HMD branch. If correction happens after OpenXR eye
publication, refresh/recompose the final rendered eye pose as needed rather
than leaving the camera one frame behind. Do not solve this by skipping
vertical contact, changing raw XR poses, or adding another floor responder.

## Testable slices and acceptance

1. **Binding and topology:** resolve carrier separately from response target.
   Test both the single combined `mittens-corp` root and separate nested
   physics/gamepad frames. The capsule branch sits outside InputXR, follows
   every simulated frame once, and has no HMD-driven pose dependency.
2. **Calibration and lifetime:** retain dimensions/standing placement across
   relocation; verify scale, startup readiness, explicit recalibration, opt-out,
   two avatars, external reparenting, carrier/AVC/GLTF removal, and serialization.
   Component counts return to baseline; no helper siblings leak.
3. **Contact and frame-order integration:** vary physical head XYZ up/down and
   laterally with no simulated movement. Capsule pose and locomotion frame
   remain unchanged by tracking; head world displacement equals its tracking
   displacement. Repeated crouch/stand creates no accumulated origin drift.
   Existing floor support can remain active, but tracking alone must not create
   a new impulse or change the support result. Then combine changing HMD pose
   with gamepad motion, button jump, gravity/fall, and floor/wall contact. Verify
   genuine contact still corrects the response target, updates its Velocity,
   and reaches the final rendered camera without stale-frame publication.
4. **Real scenes:** verify Rei(mu) in `mittens-corp` and Bisket in
   `capsule-stick-figure`. Exercise physical crouch/stand and lateral movement,
   simulated locomotion, jump/landing, stage-edge behavior, teleport/recenter,
   and mount/drive/dismount/remount. Preserve desktop contact behavior. Record
   capsule center/bottom, calibration offset, carrier/root Y, raw head Y, and
   contact displacement. Until crouch IK lands, visible feet may penetrate the
   floor during physical crouching; this must not cancel tracking movement.

Keep hot-path work bounded per avatar: cache carrier/routing discovery, avoid
per-frame hierarchy scans and allocations, and measure contact/proxy-update
cost against the existing implementation. No multi-parent support, general
constraint graph, stair-stepping policy, or crouch joint solving is required.

## Related work

- [AVC upright character capsule](avc-upright-character-capsule.md)
- [XR avatar grounding first slices](xr-avatar-grounding-first-slices.md)
- [Attachment topology cleanup](attachment-system-world-owned-topology-cleanup.md)
- [VR avatar embodiment epic](epic/vr-avatar-embodiment.md)
