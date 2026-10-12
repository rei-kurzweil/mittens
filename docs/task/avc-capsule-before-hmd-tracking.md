# Task: Make AVC capsule height independent of HMD tracking

## Status and outcome

Implemented in the working tree; real-scene headset acceptance is pending.
The earlier TOP refactor passed user smoke testing through `c38c0502`.
The XR capsule follows the head-driven
avatar's X/Z, but replace its Y with the simulated locomotion frame's Y plus
a stable calibrated offset. Keep grounding/static-contact correction on the
existing physics/locomotion response target.

Physical HMD Y must remain free inside the constrained frame on unchanged
support. This applies before mounting a vehicle and after dismounting,
as well as while mounted. The reported pedestrian symptom is that the world
moves with the head along Y, preventing physical crouch/stand movement relative
to it; the fix must remove that cancellation in both directions. The isolated
probe has confirmed downward cancellation; verify the full reported symptom
through the real frame phases and a headset smoke test.

Physical X/Z still moves the capsule and participates in wall/floor
contact; this task does not promise unrestricted room-scale motion through walls.
Gamepad motion, scripted movement, Velocity, gravity, and floor/wall contact
still affect that frame. This is the prerequisite for
[foot-penetration crouch IK](avc-foot-penetration-crouch-ik.md), not part of the
crouch solver itself.

The generic [axis-selective TOP translation sample](transform-sample-translation-axes.md)
and AVC source/calibration wiring are implemented. Automated contact checks pass;
test this in the headset before starting crouch IK. The prerequisite [TOP refactor](transform-operator-api-cleanup.md),
including existing stateful/camera-specific kind migration, is complete.

## Problem and current code

Before this fix, `avatar_control_system.rs::try_init_or_route_capsule` created a
TOP fork with rotation/scale dropped beneath `model_root_id`. The model root
follows the displaced tracked head in XYZ. Consequently, contact correction
responds to physical head movement by displacing the outer locomotion root,
which also carries the camera. A crouch can therefore raise the tracking
origin and cancel the user's movement.

The response target is appropriate. The proxy's pose source is the problem.
[The investigation](../bugs/mittens-corp-hmd-height-floor-contact-feedback.md)
records the baseline hardware report and an isolated downward-cancellation
probe; the full bidirectional headset symptom is not yet reproduced in a test.

## Target operator composition

Prerequisite: [axis-selective translation sampling](transform-sample-translation-axes.md).
Two equivalent arrangements can construct the desired translation:

```text
operator input = head-driven body world transform
sample Y     = simulated locomotion frame world Y
output       = [body.x, frame.y, body.z]

alternative: input = locomotion frame; sample body X/Z
```

Prefer the first arrangement for the existing implementation. Keep the capsule
branch beneath the model root's TOP fork, retain its rotation/scale
drops, then add a standalone translation-sampling stage to its recomposed
output. Sample only Y from the pre-tracking frame and apply the calibrated
capsule offset afterward. This replaces the earlier
proposal to move the capsule alongside InputXR and exclude tracked XYZ.
The file path is retained so existing task links remain valid.

Concretely, insert the new sampling component **between the TOP fork and
the capsule T**, as a downstream stage rather than inside its translation map:

```text
TOP.fork_trs().drop("rotation").drop("scale")
└─ TOP.sample_translation().source(locomotion_frame).axes("y")
   └─ capsule T(calibrated offset) -> Zone -> Collidable
```

The HMD transform and model root retain their existing parents. The completed
refactor stores fork pass/drop settings as values. This height-fix slice then gives the capsule T the sampling operator as its
parent and changes its calibrated offset. It edits fork output before the capsule local offset is applied.
Putting the stage beneath capsule T would instead edit the basis of its
descendants after that offset, not the capsule T itself; direct-T modifier
semantics are not part of this first API.

Structural sibling placement is still possible using the alternative X/Z
sampling arrangement, but is not required for this first slice. It would add
external-helper ownership obligations without changing the desired vector.
Do not add an unnecessary motion layer or move AVC out of its tracking branch.

In `mittens-corp`, physics and gamepad share `rei_mu_locomotion_root`; that root
supplies sampled Y. If the two are nested separately, sample the combined
simulated frame immediately before InputXR applies tracking. Referencing that
transform is a data dependency, not a new parent attachment.

## Separate pose sources from response target

Resolve three roles explicitly:

- **Horizontal source:** the current head-driven model root, retaining AVC's
  displaced-head/body-anchor convention instead of switching to raw HMD center.
- **Vertical source:** the composed simulated frame immediately before the
  owning InputXR applies tracked pose. Its world Y includes all simulated
  motion, including inner gamepad motion beneath an outer physics transform.
- **Contact response target:** the transform that receives penetration
  correction. Retain `AVC.movement_target(...)` and existing automatic routing.
  The target may be an ancestor of the vertical source rather than that source.

For the normal XR hierarchy, discover the vertical source from the owning
InputXR's pre-tracking ancestry. Validate it as a live transform and validate that
moving the response target carries the proxy and tracking branch. An explicit
movement target must not silently become the sampled source when doing so would omit
an inner motion layer. Unsupported/disconnected routing stays unready with a
diagnostic rather than retaining a stale pose source.

The selected contract excludes physical HMD Y, not X/Z, from capsule pose.
Physical horizontal movement can encounter different ground heights and cause
genuine support correction. The guarantee is that changing physical head Y
alone does not move the capsule vertically or manufacture a root correction.
Stair stepping, slopes, and room-scale contact refinements are separate policy.

Limit this migration to XR first. Desktop has no equivalent HMD boundary;
preserve its existing pose-source/response routing and verify that behavior.

## Calibrated offset and readiness

Keep the existing CPU-bounds-based shape inference and authored capsule radius.
Convert the shape dimensions and calibrated placement to a consistent world
metric; changing the Y source must not change its measured height or apply model
scale twice. Record bounds provenance and reject unsupported scale with a
diagnostic. The calibrated capsule bottom must match the intended standing
body/ground relation, not the current frame's tracked head height.

Retain a stable vertical placement offset relative to the sampled frame. Establish
it from a known standing calibration/model relation or an explicit bottom
offset, with valid tracking if the chosen calibration requires it. Do not bake
an arbitrary crouched startup pose into the capsule, assume every floor is
world Y = 0, or recalculate the offset from the live avatar each frame.
Recalibration is explicit; ordinary HMD changes and recentering do not silently
change capsule geometry or placement.

`CapsuleReady` must wait for usable dimensions, calibration, valid sample and
response routing, and propagated world pose. Preserve its settled-frame event
behavior and scene gravity gating. Do not report readiness merely because the
runtime Zone exists. Ensure the authored `collision_disabled()` opt-out remains
effective and document any provisional calibration API before exposing it.

## Lifecycle and scheduling

The preferred arrangement retains existing structural ownership. Track its
generated branch and sampled source references. Avatar/GLTF removal, opt-out,
source loss, and rig replacement must retire, disable, or rebuild it without
deleting unrelated components. Do not fall back to tracked Y while contact is
enabled if the sampled source disappears or has not resolved.
Keep helpers excluded from serialization and prevent duplicate branches on
retries. Preserve two-avatar isolation and mapping/asset invalidation behavior.

Reconcile sampling/routing changes after external reparenting or mount topology
changes. Normal car mounting carries the entire rig. Keep the existing
mounted-contact exclusion; dismount must restore pedestrian contact without
recalibrating against the vehicle or head pose.
Discard obsolete sweep/contact history when a proxy is replaced or its frame
changes discontinuously, including teleport, so old positions do not create
false floor crossings.

The capsule must be current before each existing pose-only or fixed-step contact
pass. Simulated root changes propagate to the input and sampled source; physical
tracking changes input X/Z while sampled Y stays independent of head Y.
Refresh the body X/Z source before consumers read it, and propagate referenced
source changes even when the input has not changed. If correction happens after
OpenXR eye publication, refresh/recompose the final rendered eye pose as needed rather
than leaving the camera one frame behind. Do not solve this by skipping
vertical contact, changing raw XR poses, or adding another floor responder.

## Testable slices and acceptance

1. **Binding and operators:** resolve horizontal/vertical sources separately from
   the response target.
   Test both the single combined `mittens-corp` root and separate nested
   physics/gamepad frames. Verify the y-sampling component is wired correctly,
   simulated parent motion is included once, and HMD Y has no pose dependency.
2. **Calibration and lifetime:** retain dimensions/standing placement across
   the Y-source change; verify scale, readiness, explicit recalibration, opt-out,
   two avatars, external reparenting, source/AVC/GLTF removal, and serialization.
   Component counts return to baseline; no helper siblings leak.
3. **Contact and frame-order integration:** vary physical head Y up/down with
   no simulated movement on unchanged support. Capsule Y and locomotion frame
   remain unchanged by that tracking delta; head world Y inherits the full
   tracking displacement. Vary physical X/Z separately and verify capsule X/Z
   follows the body anchor, including existing wall response. Repeated
   crouch/stand creates no accumulated origin drift.
   Existing floor support can remain active, but head Y alone must not create
   a new impulse or change the support result. Then combine changing HMD pose
   with gamepad motion, button jump, gravity/fall, and floor/wall contact. Verify
   genuine contact still corrects the response target, updates its Velocity,
   and reaches the final rendered camera without stale-frame publication.
4. **Real scenes:** verify Rei(mu) in `mittens-corp` and Bisket in
   `capsule-stick-figure`. Exercise physical crouch/stand and lateral movement,
   simulated locomotion, jump/landing, stage-edge behavior, teleport/recenter,
   and mount/drive/dismount/remount. Preserve desktop contact behavior. Record
   capsule center/bottom, calibration offset, sampled frame/root Y, raw head Y, and
   contact displacement. Until crouch IK lands, visible feet may penetrate the
   floor during physical crouching; this must not cancel tracking movement.

## Implemented slice and remaining acceptance

The generated XR branch is now `model root -> fork(drop rotation/scale) ->
sample_translation(Y from pre-InputXR frame) -> capsule T -> Zone/Collidable`.
The contact response target remains separate and can be an outer physics root
above an inner gamepad frame. Reference/routing discovery is cached by World
selector revision and authored target; normal pose updates use live handles.

The current calibration assumes the engine's OpenXR **STAGE** reference space:
its floor origin maps to the pre-tracking frame's Y. Capsule bounds remain
model-relative CPU/rest geometry, scaled once into world units. Their center
provides the stable offset; tracked head height is never used for XR calibration.
The fallback is authored avatar height or the established rest head/body offset
after a GLTF has spawned. Positive uniform world scale is supported; unsupported
scale/shear or a later scale change keeps contact/readiness disabled. A general
floor-origin/bottom-offset or explicit recalibration API is still follow-up work.

XR colliders start disabled. Before every existing contact pass, retained avatar
samples settle and contact is enabled only with valid source/routing/scale.
Source loss never enables the generic tracked-Y fallback. Changing source or
response target clears proxy sweep history; existing teleport and mounted-contact
history handling remains active. Generated helpers stay beneath their avatar,
the entire generated XR fork/sample/capsule branch is excluded from filtered scene
saves, and ordinary subtree removal clears sampling state. Desktop retains its original
pose composition. OpenXR's render path reads the current rig origin when composing
eyes, so correction reaches final eye composition; hardware verification remains
required.

Headless tests exercise SystemWorld's real contact/physics/AVC frame phases with
a completed-splice fixture: crouched startup, repeated up/down tracking with no
origin drift, physical X/Z, separate physics/gamepad layers, real penetration,
wall response, jump/gravity/landing and grounded velocity, mount/dismount/remount,
uniform scale, invalid scale, source loss/recovery, collision opt-out, two-avatar
isolation, and removal returning component counts to baseline. These fixtures
do not simulate a headset, imported avatar initialization, or GPU eye publication.

Pending acceptance: Rei(mu) in `mittens-corp` and Bisket in `capsule-stick-figure`,
rendered eye motion, real stage/stair/edge behavior, teleport/recenter, rig/GLTF
replacement and explicit recalibration policies, and runtime cost profiling.
Do not mark the hardware bug resolved or begin crouch IK before the smoke test.

### Validation commands and headset checkpoint

Automated validation includes the transform, AVC, static-contact, Velocity,
attachment, mittens-corp, TOP authoring, World graph, and round-trip test groups,
plus `cargo check --examples`. Transform checks exclude the previously recorded
`live_eval_nested_let_attached_transform_animates_via_keyframe_block`,
`live_eval_imported_factory_component_supports_top_level_update_transform`, and
`live_eval_transform_quaternion_builder_supports_array_aliases` failures.
Round-trip checks exclude the recorded `roundtrip_animation_paused`,
`roundtrip_keyframe`, and `roundtrip_music_note_c5` failures. These are baseline
exclusions, not failures resolved by this slice.

Build and run the normal scene:

```sh
cargo build --release --bin mittens-engine
target/release/mittens-engine load examples/mittens-corp.mms
```

First check physical crouch/stand before mounting and after dismounting, then
physical lateral movement, jump/landing, and another mount/dismount cycle.
Expected: the head moves through its full Y range while capsule height follows
simulated locomotion; actual floor/wall contact still corrects that locomotion.

Keep hot-path work bounded per avatar: cache sampling/routing discovery, avoid
per-frame hierarchy scans and allocations, and measure contact/proxy-update
cost against the existing implementation. No multi-parent support, general
constraint graph, stair-stepping policy, or crouch joint solving is required.

## Related work

- [Completed transform operator API cleanup](transform-operator-api-cleanup.md):
  unified Rust kind representation, builders, and ordinary child-output semantics.
- [Axis-selective translation sampling](transform-sample-translation-axes.md)
- [AVC upright character capsule](avc-upright-character-capsule.md)
- [XR avatar grounding first slices](xr-avatar-grounding-first-slices.md)
- [Attachment topology cleanup](attachment-system-world-owned-topology-cleanup.md)
- [VR avatar embodiment epic](epic/vr-avatar-embodiment.md)
