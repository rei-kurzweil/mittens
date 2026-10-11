# Task: AVC crouch IK from predicted foot penetration

## Status and outcome

Planned; documentation only. First fixtures are Rei(mu) and Bisket. AVC should
automatically prepare a bounded lower-body crouch solver for a mapped humanoid
with valid XR tracking and a usable ground reference. Lowering the physical
head lowers the body; the solver bends the legs to keep the feet above the
floor without moving the HMD, camera, or locomotion root.

This is the procedural first slice. It supersedes the authored-animation-only
first step in [VR avatar crouch posture](vr-avatar-crouch-posture.md) and Phase 4
of [simple humanoid body follow](avatar-control-simple-humanoid-body-follow.md).
Authored crouch polish, kneeling, sitting, and walking remain later work. This
task does not require a general avatar pose-layer implementation.

Prerequisite: [AVC capsule height independent of HMD tracking](avc-capsule-before-hmd-tracking.md).
That task owns translation sampling and calibration: tracked body X/Z, sampled
locomotion Y. This task owns leg posture.

## Existing seams and findings

- `src/engine/ecs/system/ik/head_pose_body_xz_follow.rs` currently places body
  world Y at displaced head Y plus `model_root_local_y`. Its header still
  describes fixed Y; update that comment when implementing the relevant change.
- AVC's capsule follows the body's translation but responds on the outer
  movement root. Resolve [HMD floor-contact feedback](../bugs/mittens-corp-hmd-height-floor-contact-feedback.md)
  before using an XR smoke check to validate crouching. Crouch IK must not be
  used to disguise an origin that still cancels physical head motion.
- `HumanoidSlot` already includes Hips, Left/RightUpperLeg, LowerLeg, Foot,
  and Toes. The map report provides resolved IDs, diagnostic statuses, and a
  generation for invalidation. `head_ready` and `arm_ready` exist; a lower-body
  eligibility check is still needed.
- The shared VRoid MMS preset explicitly lists hips and arm landmarks but
  relies on automapping for legs. Leg convention candidates already exist in
  `humanoid_bone_map_system.rs`. Verify both complete reports; extend the preset
  only if explicit declarations improve its contract. Never query raw names
  from the crouch solver.
- Inspection of the two checked-in GLBs confirms identical local TRS on the
  hips and both thigh/knee/foot joints. Thigh-to-knee local distance is about
  0.40625 m and knee-to-foot about 0.46191 m in both files. These are fixture
  observations, not constants for the solver. Bind rotations are nonidentity.
- `IKSolver::TwoBoneIK` already accepts explicit root/mid/end joint IDs and a
  body-local pole. However, `solve_two_bone` currently applies forearm twist
  toward target orientation even when `copy_end_rotation` is false. Audit and
  separate that arm policy before reusing the implementation for legs.
- `ik_system.rs` is about 1,500 lines and AVC's system about 3,100. Keep crouch
  policy in a separate `system/ik/` module. Existing `zone_query` contacts and
  capsule floor sweeps are useful geometry primitives, but are not a retained
  per-foot support-plane API.

## Motion and pose ownership

Gamepad, scripted locomotion, Velocity, gravity, and contact own the simulated
movement frame. XR tracking is composed inside that frame. This solver owns
lower-body posture only and must never write raw tracking, InputXR's driven
transform, the camera, the movement root, Velocity, or grounded state.

The first leg solve rotates UpperLeg and LowerLeg, with Foot orientation
controlled to preserve a calibrated sole frame. Rotating the shared Hips bone
is not required for the first upright crouch: it would also move the torso
and interfere with head/arm alignment. Anatomical pelvis tilt or backward
shift is later polish with an explicit ownership rule. No torso scaling,
bone-length changes, or revival of spine FABRIK to chase the headset.

Use each rig's bind pose and resolved geometry, not a fixed Euler rotation
about an assumed joint axis. Check supported parent relationships, finite
nonzero segment lengths, and scale. The first slice supports uniformly scaled
rigs (with tolerance for GLTF numeric noise); unsupported hierarchies or scale
receive a diagnostic and skip the solve rather than silently deforming.

## Input: penetration before crouch correction

For each leg, evaluate its uncrouched/base FK under the current head-following
body placement. Derive a sole/contact landmark from the mapped foot plus a
calibrated local sole offset; an ankle joint is not the bottom of the shoe.
Toes are optional aids for foot orientation, not a prerequisite. Begin with
explicit per-model sole offsets and visible landmarks; automatic shoe-mesh
measurement is separate work.

The support provider is read-only. For the first deterministic fixture it
supplies an explicit horizontal floor plane. Runtime integration must resolve
a valid bounded support surface from actual static floor geometry, or consume
a valid grounding-support record if one exists by then. Neither is currently
a guaranteed API. Do not assume world Y = 0 or use HMD Y as the floor height.

For an upward unit normal `n`, plane point `p`, and predicted sole position `s`:

```text
signed_clearance = dot(n, s - p)
penetration_m = max(0, -signed_clearance)
desired_sole = s + n * penetration_m
```

Keep left/right distances and support validity separately. A maximum distance
may drive a diagnostic crouch amount, but the two leg targets use their own
penetration. No valid nearby support means no floor-driven crouch correction.
The first runtime slice covers flat support, not stair foot planting or slopes.

Crucially, never compute next frame's driver from already-corrected feet.
Reconstruct base FK independently of the previous crouch result every frame.
For the first fixture, this is the cached calibrated standing leg pose; lower
leg posture has one owner. Future authored animation can supply a base pose
before the constraint, with an explicit composition policy. Repeatedly holding
the same head height must produce a stable bend, not alternating zero/full
crouch or accumulating rotation.

## Solve and limits

Convert each desired sole position to an ankle target using the desired foot
orientation and sole offset. Solve thigh → knee → foot with the reusable
two-bone positional math and a calibrated body-local forward knee pole.
Preserve lateral stance and foot X/Z for the flat standing fixture; targets
follow the current base pose horizontally rather than world-locking the feet
during locomotion. This is not a walking/footstep system.

Bound reach to both the minimum and maximum segment reach and impose a maximum
supported knee flexion. Verify the existing two-bone math at deep crouch and
unequal segment lengths; it does not yet enforce an anatomical knee limit.
Choose a stable bend plane near straight legs. Do not copy arm twist/pole
defaults into the leg policy.

Deadband/hysteresis may suppress tracker noise. If filtering delays correction,
record the permitted transient sole error; do not silently filter the camera
or introduce persistent penetration. Unsupported depth clamps the pose and
reports residual foot error. It must never clamp the HMD or lift its origin.
Physical rising can leave the feet above the floor; do not extend legs beyond
their lengths or snap the avatar/camera down to invent contact.

## Module, automatic wiring, and frame order

Working module name: `src/engine/ecs/system/ik/humanoid_crouch.rs`, exported by
`system/ik/mod.rs`. Keep binding/calibration, penetration evaluation, and target
generation here. Reuse leg-safe two-bone math via a small shared helper or
properly configured IKChain; do not copy an entire solver into AVC.

AVC automatically attempts preparation after its own GLTF map is ready. Require
Hips and both complete three-joint leg chains for the initial symmetric crouch.
Missing/ambiguous/absent mappings skip crouch with a retained reason and leave
head and arm tracking usable. Invalid tracking postpones calibration. Calibrate
only in an explicit valid standing/support state; do not redefine standing
height every frame or when the application initially starts with a crouched user.
Expose an opt-out and sole/pole calibration overrides; exact MMS names are TBD.

Retain IDs, lengths, sole frames, mapping generation, and baseline state per
AVC/GLTF instance. Rebuild on relevant mapping/rig changes, clean up generated
targets/chains on removal, and avoid duplicate wiring. Own generated helpers
with runtime-only serialization semantics like existing AVC helpers. Mounting
suspends floor crouch; dismounting re-evaluates current support without carrying
vehicle-floor state into pedestrian mode. Seated posture is separate work.

Required order, with intents flushed and world transforms current at each
dependency boundary:

1. Resolve simulated locomotion/physics contact on the proxy whose Y is
   independent of tracking, then publish tracking composed with the final
   movement frame. Keep proxy X/Z current at relevant contact passes.
2. Initialize/update AVC and apply head-sensitive body follow.
3. Evaluate uncrouched FK, resolve support, and generate leg targets.
4. Solve leg rotations and the existing independent head/arm constraints.
5. Propagate solved bones before dependent secondary motion and skin palettes.

IKSystem currently iterates a HashSet of chains and queues writes. Do not rely
on chain iteration order or unflushed targets. The first pose policy leaves
pelvis/spine unchanged, so legs and arms can solve independently once their
inputs are committed. Introducing pelvis changes later requires a new explicit
ordering boundary.

## Testable slices

1. **Binding and measurement:** inspect both real map reports and calibrate
   rest lengths, sole offsets, and knee poles. A renamed synthetic humanoid
   verifies semantic mapping, and two live instances prove isolation.
2. **Pure driver:** controlled body-height drops against a fixed floor produce
   the expected per-foot penetration in metres. Yaw/pitch-only head motion
   uses the anatomical anchor and does not manufacture crouch. Zero support,
   scaled fixtures, and frame-to-frame baseline independence are covered.
3. **Leg math:** make two-bone solving leg-safe without regressing arm behavior.
   Solve partial/full supported crouch, near-straight legs, unequal lengths,
   and unreachable depth. Verify segment lengths, forward knees, sole clearance,
   bounded residuals, and repeatability rather than just checking an angle.
4. **AVC wiring and scheduling:** automatic setup, opt-out, map invalidation,
   tracker loss/recovery, removal, and mount/dismount lifecycle work. A real
   frame-order test verifies generated targets and solved bones reach the
   skin palette without a stale frame or pose accumulation.
5. **Rei(mu)/Bisket smoke:** use `mittens-corp` and a controlled Bisket fixture
   with foot/target markers and a small diagnostic panel. Show raw head Y,
   movement-root Y, support height, base/solved sole heights, penetration,
   knee flexion, validity, and residual error. Test standing → crouch → hold →
   stand, physical rise, looking around, gamepad movement, button jump/fall,
   and car entry/exit. Pending headset observations remain explicitly pending.

For the deterministic supported range, start with a 1 cm sole-clearance error
budget and unchanged segment lengths within numeric tolerance. Bound work to
two legs per eligible avatar; cache rig discovery and calibration, avoid
per-frame scene queries or helper allocation, and measure full-scene cost.
No pose writes may alter the locomotion root or tracking samples.

## Related work

- [XR avatar grounding first slices](xr-avatar-grounding-first-slices.md)
- [Humanoid bone-map automapping and MMS presets](humanoid-bone-map-automapping-and-mms-presets.md)
- [VR avatar embodiment epic](epic/vr-avatar-embodiment.md)
- [Transitionable avatar pose layers](avatar-pose-transition-layers.md)
- [Post-IK torso constraint](avatar-control-post-ik-torso-constraint.md)
