# Task: AVC crouch IK from predicted foot penetration

## Status and outcome

Native flat-support first slice implemented, 2026-10-21. Initial headset feedback
on 2026-10-22: crouching seems to work fairly well, provided XR floor height is
calibrated properly. Broader scenario acceptance and tuning remain open.
`HumanoidCrouchConstraint` / `HumanoidCrouchSystem` live in
`src/engine/ecs/system/ik/humanoid_crouch.rs`. AVC binds mapped legs automatically;
explicit foot-local sole calibration enables solving. Rei(mu), mittens-corp and
bisket-vr-demo now supply landmarks for their shared shoes.

XR floor-height calibration belongs to the XR runtime's room setup and the
[XR settings/floor-correction layer](../spec/xr-settings.md). Crouch IK consumes
the resulting tracked pose and collision support geometry; it does not determine
the physical floor height or adjust the tracking origin, camera, or floor
correction to make a pose fit. Correct tracking-floor alignment is a prerequisite
for the pose to look right. Avatar proportions and foot-local sole landmarks
remain separate rig-calibration concerns even when the XR floor is correct.

The system shares the [retained collidable BVH](../spec/collidable-bvh.md) with
static contact. One padded world AABB covers the capsule and predicted soles.
Candidates are filtered by center below capsule center, exact yawed-box X/Z
footprint, and top height within the support margin of the capsule bottom.
Each foot chooses the eligible top closest to that bottom; stable collidable
ordering breaks ties. No authored floor marker, world-Y-zero floor assumption,
new collider, second BVH or separate Y-only index is introduced.

Cached rest FK supplies the base feet every update, independently of last frame's
crouch. Thigh/knee rotations close the bounded positional solve; foot rotation
preserves its base sole frame. Shared two-bone triangle math is extracted from
the arm solver, retaining its separate forearm-twist behavior. Reach and knee
flexion limits report residual penetration instead of moving the tracked view.
There are no generated IKChain/target components to allocate or serialize.

Automated checks exercise synthetic semantic mappings, both actual GLB reports,
rest rotations, scale/yaw, repeated holds, bounded depth, individual foot support,
floor edits/removal, airborne support loss, opt-out/tracking recovery, mount and
dismount, multiple instances, MMS configuration, and the complete avatar stage.
Frame propagation precedes arm IK and skin palette updates. These are CPU checks;
the initial visual report is positive, but measured shoe alignment, full-scene
cost and the complete headset scenario checklist remain to verify.

Validation: 12 crouch checks pass, plus the head/neck, avatar/capsule, collidable
BVH, static-contact and bone-map groups. Two older IK reference-resolution tests
fail identically when compiled from the committed `ik_system.rs`: their chains
are not registered with the local IKSystem. Arm geometry/twist checks pass.

## First-slice MMS controls and checkpoint

```mms
AVC
    .crouch_sole_offsets([0.0, 0.0530325, 0.0874534], [0.0, 0.0530325, 0.0874534])
    .crouch_knee_pole([0.0, 0.0, 1.0])
    .crouch_knee_limit(2.617994)
    .crouch_support_margin(0.05)
```

`crouch_enabled(bool)` defaults true; false releases leg ownership and restores
the cached rest pose if solving was active. Sole offsets default unset and are
in each foot's local bind frame; unset calibration yields `NeedsSoleCalibration`,
never an ankle-as-sole guess. The example values are specific to the checked-in
Rei(mu)/Bisket shoes: original ankle height about 0.131308 m and shoe-bottom height
about 0.029031 m, converted through each foot's nonidentity bind basis. They are
not a universal VRoid measurement. No live standing-height/HMD calibration is
needed to bind these immutable landmarks, including at crouched startup.

The pole is body-local, +Z by default. Knee limit is radians, default 150 degrees;
support margin defaults 0.05 model metres and scales with the body. Runtime
support is required near the independently grounded capsule bottom, so a nearby
table top or distant floor during a jump cannot pull the feet toward it.
No valid per-foot support restores that leg's base pose. Tracking loss, mounting,
disabled collision, unsupported bindings/scale, or opt-out release the pose.
Map changes refresh calibration and release previously owned joints.

Rust diagnostics are available through `SystemWorld.humanoid_crouch.status(id)`
and `.feet(id)`: support zone/height, base and solved soles, penetration, knee
flexion and residual. A dedicated MMS diagnostic panel remains follow-up work.
Use normal `mittens-corp`, `rei(mu)` or `bisket-vr-demo` for remaining headset
checks with a calibrated XR floor: standing → crouch → hold → stand, then physical
rise, look/roll, walking, button jump/fall and car entry/exit. The initial report
does not establish acceptance of every scenario. Slopes, stair planting, world-locked steps,
pelvis/torso adjustments and authored crouch blending remain separate work.

First leg-solver fixtures are Rei(mu) and Bisket. AVC should
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

- Supported rigs now use `HeadNeckBodyConstraintSystem` for body placement before
  crouch evaluation. `head_pose_body_xz_follow.rs` is its legacy fallback.
- AVC's capsule follows the body's translation but responds on the outer
  movement root. Resolve [HMD floor-contact feedback](../bugs/mittens-corp-hmd-height-floor-contact-feedback.md)
  before using an XR smoke check to validate crouching. Crouch IK must not be
  used to disguise an origin that still cancels physical head motion.
- `HumanoidSlot` already includes Hips, Left/RightUpperLeg, LowerLeg, Foot,
  and Toes. The map report provides resolved IDs, diagnostic statuses, and a
  generation for invalidation. `head_ready` and `arm_ready` exist; a lower-body
  eligibility check is still needed.
- The shared VRoid preset now explicitly lists both leg chains. The mapper also
  recognizes VRoid thigh/knee/foot convention names; real-GLB tests verify complete
  reports. `lower_body_ready()` checks hips plus both complete chains. The crouch
  solver never queries raw bone names.
- Inspection of the two checked-in GLBs confirms identical local TRS on the
  hips and both thigh/knee/foot joints. Thigh-to-knee local distance is about
  0.40625 m and knee-to-foot about 0.46191 m in both files. These are fixture
  observations, not constants for the solver. Bind rotations are nonidentity.
- `IKSolver::TwoBoneIK` retains its existing arm policy. Both systems now use
  `system/ik/two_bone.rs` for positional triangle math; crouch adds leg limits
  and sole orientation without applying arm/forearm twist.
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

The support provider is read-only, using bounded static zone geometry through
the retained contact index. Flat, upright (including yawed) boxes are supported.
The capsule bottom bounds the permitted support height; each foot's X/Z must
lie within that particular surface footprint. Tests use real collidable zones,
including a floor above world Y = 0. HMD Y never supplies the floor height.

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
head and arm tracking usable. Invalid tracking postpones AVC initialization.
Binding uses immutable rig rest geometry and authored sole landmarks, so a
crouched startup cannot become a new standing baseline. A future measured
standing-height or sole-calibration flow must require an explicit valid standing/
support state rather than redefining that reference each frame.
Expose the opt-out and sole/pole/limit/margin controls documented above.

Retain IDs, lengths, sole frames, mapping generation, and baseline state per
AVC/GLTF instance. Rebuild on relevant mapping/rig changes and release previously
owned joints. The native first slice uses no generated target/chain components;
bindings are runtime-only and removal clears retained system state. Mounting
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
