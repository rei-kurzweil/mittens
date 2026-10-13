# Task: AVC VR standing height and sole-to-floor calibration

Date: 2026-10-13.

Status: investigation complete for current code; implementation and headset
measurements pending. This task records a read-only source/asset investigation.

## Report and outcome

In VR, the avatar appears roughly 10–20 cm above the floor even though its
overall height looks close to the user's real height and static contact works.
When the user kneels, the avatar's knee/floor relationship also looks displaced.
These are user observations, not measured runtime clearances.

Establish a measurable standing relationship between the physical tracking
floor, HMD/eyes, avatar head pivot, and shoe soles. Keep physical head movement
free. Verify standing alignment before implementing crouch or kneeling posture.

## What the current implementation calibrates

There are two independent vertical placements:

1. **Visible body:** `try_init_splices` in
   [avatar_control_system.rs](../../src/engine/ecs/system/avatar_control_system.rs)
   measures the resolved camera anchor in immutable model-relative rest space.
   It caches its negative Y as `model_root_local_y`. `avatar_height(...)`
   overrides that distance; it does not scale the avatar or mean total
   crown-to-sole height. Init also writes unit scale to the model-root transform.
2. **Collision capsule:** `try_init_or_route_capsule` measures aggregate CPU
   render bounds and infers bottom/top from `min.y`/`max.y`. In XR, supported
   uniform world scale is applied once. A translation-sampling operator replaces
   capsule Y with the pre-tracking locomotion frame's Y, then adds the measured
   capsule-center offset. Its bottom is therefore `frame_y + scaled_bounds_min_y`.
   Contact corrects the movement root, independently of physical HMD Y.

[HeadPoseBodyXzFollowSystem](../../src/engine/ecs/system/ik/head_pose_body_xz_follow.rs)
actually updates body **XYZ**, despite its name and stale fixed-Y header:

```text
model_root.world.y = displaced_head.world.y + model_root_local_y
```

The displaced head follows the tracked driver with the rotated negative
eye-to-head offset. The body consequently rises/lowers with the physical head,
while its collision capsule has a stable vertical placement. Grounded capsule
contact does not guarantee grounded visible feet.

The XR runtime creates an identity `STAGE` reference space in
[openxr_system.rs](../../src/engine/ecs/system/openxr_system.rs). This relies
on the runtime's floor calibration; there is no separate AVC measurement of
the user's physical floor or standing eye height. In the normal unscaled XR
path, 0.1 world unit corresponds to 10 cm.

## Current mittens-corp setup and asset measurements

[mittens-corp.mms](../../examples/mittens-corp.mms) uses `rei(mu).glb`, the
shared VRoid humanoid map, no `avatar_height` override, and an authored CXR/rider
wrapper at `[0, 0.08, 0.12]`. AVC consumes that wrapper as an eye/head offset.

[The map preset](../../assets/components/humanoid_bone_maps/vroid.mms) supplies
head and both eye joints without an explicit camera anchor. The map resolver
prefers a unique named camera/view/eyes descendant, then derives an eye midpoint,
then falls back to the head. Record the actual report/provenance in the live
scene; do not assume a head fallback when both eyes resolve.

Read-only GLB inspection, composing node rest TRS and transforming POSITION
accessor bounds, gave the following for the checked-in Rei(mu) asset:

| Landmark | Asset rest Y, metres |
| --- | ---: |
| Head pivot | 1.518189 |
| Eye midpoint | 1.575187 |
| Left/right foot joint (ankle) | 0.131308 |
| Left/right lower-leg joint (knee) | 0.592459 |
| Aggregate mesh bounds minimum | 0.029031 |
| Aggregate mesh bounds maximum | 1.856926 |

The aggregate height is about 1.828 m; the eye-midpoint-to-bounds-bottom
distance is about 1.546 m. Matching total avatar height to total human height
does not match standing eye-to-sole height, especially with hair/accessories.
The ankle is about 10.2 cm above the aggregate bottom: foot-joint Y is not a
shoe-sole measurement.

These are asset rest/accessor measurements, not a runtime deformed-skin or
per-shoe contact measurement. Runtime CPU bounds, resolved anchor, effective
scale, and visible sole landmarks still need to be compared.

## Why a standing gap is possible

For an upright, unscaled avatar with no authored height override, define:

```text
h = tracked HMD Y above the STAGE origin
d = vertical HMD-to-displaced-head offset
a = camera-anchor rest Y in the model
s = calibrated sole Y in the model
b = inferred capsule bounds minimum Y
f = locomotion frame world Y

sole_world_y    = f + h - d - a + s
capsule_bottom = f + b
standing_gap   = h - d - a + s - b   (when capsule_bottom rests on support)
```

Contact fixes `f + b`; it does not force `standing_gap` to zero. If sole and
capsule bottom coincide, the current Rei(mu) eye-midpoint path with an 8 cm
offset would require approximately `h = 1.655187 m` for zero standing gap.
For illustration, `h = 1.80 m` would yield about 14.5 cm clearance under these
assumptions. This demonstrates a plausible mechanism, not a diagnosis of the
user's unmeasured headset pose.

Audit an additional convention mismatch: initialization subtracts the
**camera-anchor** height, while steady body follow anchors that subtraction at
the **displaced head pivot**. With an eye midpoint, the anchor is already about
5.7 cm above the head pivot; the authored 8 cm wrapper adds another head offset.
Resolve which landmark each offset describes and apply each conversion once.
This mismatch alone should not be claimed to explain the reported positive gap;
its sign and net effect depend on actual HMD height and resolved configuration.

Audit scale independently. Capsule scale checks do not prove visible-body
calibration is correct: body follow adds the cached local height directly to a
world-space head Y, and init overwrites model-root scale. Test the user's actual
scale hierarchy rather than assuming the capsule's scale test covers embodiment.

## Standing, crouching, and kneeling are separate acceptance cases

There is no implemented standing-HMD capture/recalibration workflow, automatic
human/avatar eye-to-sole matching, calibrated per-foot sole frame, or floor-based
leg solver in the current AVC path. The lower body retains its ordinary base pose
unless authored animation changes it.

[Foot-penetration crouch IK](avc-foot-penetration-crouch-ik.md) is planned.
It bends legs as the head-driven body lowers toward valid support. It should
start from a verified standing calibration. Kneeling, sitting, and anatomical
knee contact are explicitly later posture work; lowering the HMD alone cannot
infer that the real knees are touching the floor.

The earlier [capsule/HMD separation](avc-capsule-before-hmd-tracking.md) is
implemented. Do not restore a head-following capsule to force the visible feet
down: that can cancel physical crouching through movement-root correction.

## Implementation slices

1. **Measure the live mismatch.** Add a small optional diagnostic panel and
   markers for raw STAGE HMD Y, composed HMD Y, displaced head Y, resolved
   anchor/provenance, cached calibration, model-root Y, effective scales,
   capsule bottom, per-foot sole Y, nearby support top, grounded state, and
   each movement-root correction. Separate standing clearance from crouch/knee
   errors. Compare Rei(mu) and Bisket under the same scene setup.
2. **Unify landmark conventions.** Define the model's sole-to-head and
   head-to-eye relationships explicitly. Correct the camera-anchor/head-pivot
   offset composition and supported world-scale handling. Preserve the final
   tracked camera pose and neck/head alignment while changing body placement.
   Update stale field/system comments with the implemented convention.
3. **Explicit standing calibration.** Capture only on user request with valid
   tracking and known nearby support; do not capture a crouched startup pose.
   Retain per-avatar standing eye height and sole offsets. Define a documented
   policy for human/avatar proportion mismatch: user-selected uniform scale,
   an explicitly calibrated body offset with measurable head/neck consequences,
   or later posture adjustment. A fixed root shift cannot simultaneously fix
   arbitrary proportions, soles, and head attachment. Public API names are TBD.
4. **Lifecycle and persistence.** Define reset/recalibrate behavior for avatar
   changes, scale edits, tracking loss, runtime reference-space changes, and
   recentering. Preserve a valid baseline across ordinary crouch/stand and
   mount/dismount; do not calibrate pedestrian soles against the cockpit.
5. **Posture integration.** Feed calibrated sole/support frames into the existing
   crouch task. Track kneeling as a later explicit pose/contact policy, without
   declaring that standing calibration alone fixes it.

## Acceptance and verification

- Standing on the stage and studio floor gives measured sole clearance within
  1 cm for supported calibrated avatars; head/camera alignment and model scale
  agree with the selected calibration policy. Stage top is Y=0.12; studio-floor
  top is Y=-0.85. Neither is assumed to be world Y=0.
- Repeat standing/crouch/stand without accumulating movement-root drift.
  Physical HMD Y alone does not change capsule Y or manufacture contact.
- Crouched startup remains uncalibrated until explicit valid standing capture.
- Head pitch/yaw, jump/fall, gamepad movement, runtime floor changes, and car
  mount/dismount have distinct diagnostics and do not silently change baseline.
- Use renamed synthetic rigs plus real Rei(mu)/Bisket fixtures for landmark,
  scale, and offset-composition checks. Measure sole and head errors together.
- Real headset validation remains required for the reported 10–20 cm gap and
  physical knee/floor observation.

Existing verification run on 2026-10-13: `cargo test --lib xr_capsule_`
passed all six tests. These cover tracking-independent capsule Y, scale,
wall/jump/gravity contact, mount/dismount, source loss, and instance cleanup.
They do not test calibrated rendered soles or kneeling posture.

## Related work

- [VR avatar embodiment epic](epic/vr-avatar-embodiment.md)
- [VR avatar crouch posture](vr-avatar-crouch-posture.md)
- [Skinned humanoid comparison/calibration](skinned-humanoid-avc-calibration.md)
- [HMD floor-contact feedback investigation](../bugs/mittens-corp-hmd-height-floor-contact-feedback.md)
- [Display-car floor contact](display-car-floor-contact-and-gravity.md)
