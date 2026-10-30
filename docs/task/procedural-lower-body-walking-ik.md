# Procedural lower-body walking IK

Date: 2026-10-28

Status: first flat-ground stepping slices implemented and CPU-tested. Initial
headset feedback on 2026-10-28 reports that walking works, with floor alignment
hard to assess before adding the XR height panel. Broader headset acceptance
and tuning remain pending. Authored whole-armature animation remains separate.

## Implemented slices

1. Extract native crouch into `system/ik/lower_body/`. `LowerBodyIkSystem` owns
   rig bindings and runtime state in `SystemWorld`. All original 12 crouch checks
   pass before and after extraction.
2. Generalize the leg solver to arbitrary world-space sole position and
   orientation. An actual-rig test locks a foot through translation/yaw on both
   Rei(mu) and Bisket without changing bone lengths.
3. Add retained contacts, one timed swing at a time, alternating steps, a brief
   double-support transfer, bounded prediction from actual capsule X/Z motion,
   and bounded step length/lateral prediction. Displacement and predicted
   trailing-foot lag admit steps; elapsed time finishes an airborne step even
   after movement stops.

The planner anticipates actual capsule motion to lift the first foot earlier.
The lifted foot advances earlier along its swing arc as speed increases.
Landing prediction includes recovery of feet trailing in the travel direction,
with forward lead bounded to 80% of stride and total travel bounded to stride.
Speed and directional foot lag shorten swing duration (to a minimum of 80 ms)
and the following support-transfer interval. An active swing can accelerate
without rewinding its phase; its landing remains fixed and support-checked.
Landing and support transfer consume only their remaining time, carrying the
unused frame time into the next phase. Cadence therefore avoids rounding every
phase up to a whole render frame. Idle time is never accumulated toward a future
step.

Stopping holds the resulting supported stance. Further corrective steps occur
only when stance error exceeds the deadband. Exact return to authored standing
foot positions is not required. Disabling walking releases its contacts and
returns target ownership to the rest/crouch policy.

## Rust ownership and composition

`SystemWorld.lower_body_ik` owns one `LowerBodyInstance` per AVC. Each instance
owns its optional rig binding, walking contacts/swing state, pose ownership flags
and diagnostics. AVC retains authorable settings and resolved avatar references;
it no longer stores a `HumanoidCrouchConstraint` or crouch bind error.

Modules under `src/engine/ecs/system/ik/lower_body/`:

- `system.rs`: registration, binding lifecycle, gates, goal selection, diagnostics
  and final pose emission. Both leg outputs validate before either is emitted.
- `rig.rs`: mapped rig discovery, immutable rest calibration, transform helpers
  and restoration of previously owned joints.
- `support.rs`: bounded read-only flat-support queries against the existing
  retained collidable BVH, including self-collider rejection.
- `leg.rs`: desired sole pose to bounded thigh/knee/foot rotations. It reuses
  `ik/two_bone.rs`; arm-specific twist policy remains separate.
- `crouch.rs`: supported rest goals reconstructed independently of prior solves.
- `walking.rs`: retained contacts, swing trajectories, step admission and landing.
  It returns goals and never emits joint or locomotion transforms.

The coordinator selects crouch/rest goals when no gait contact is active,
world-space contact goals during stance, and trajectory goals during swing.
Crouching with a planted contact changes the hip-to-foot geometry consumed by
the same leg solver. Swing clearance is not overwritten by a second crouch pass.

The existing `crouch_*` sole, knee and support settings calibrate the shared
lower-body solve. `crouch_enabled(false)` disables rest-goal penetration
correction; enabled walking still bends legs to reach contact/swing goals.

## Scheduling and lifecycle

After AVC updates, `prepare_bindings` captures the resolved map before flushing
AVC's head-splice intents. Those reparent operations can invalidate the report.
Body/head placement then commits, lower-body IK reads current hip/body frames
and emits legs, and arm IK follows the lower-body flush. Solved transforms reach
the normal secondary-motion and skin-palette stage.

Compatible map-generation changes preserve walking state. Changed leg mappings
release old owned joints and reset contacts. Tracking loss, mounting, disabled
collision, invalid calibration and explicit opt-out release the gait. Support
removal/height changes, jumping away from support, large translation, uniform
scale changes and sole-calibration edits invalidate contacts. Invalid/nonpositive
frame time or a frame over 250 ms releases gait to rest/crouch; the next ordinary
supported frame can establish fresh contacts.

The gait samples actual capsule-center displacement after movement/contact,
including room-scale translation and scripted/controller movement. Neither
behavior writes the tracked driver, camera, locomotion root, velocity, grounded
state or shared Hips bone.

## Controls and headset smoke scene

Walking defaults on for eligible calibrated XR AVCs, including existing scenes.
Use `.walking_enabled(false)` to opt out. Sole calibration is still required;
missing mappings, invalid tracking or missing support retain the existing gates.
Walking tuning belongs in an optional `WalkingIK` direct child of a calibrated
AVC. Existing `walking_*` AVC calls remain supported when no child is present.

```mms
AVC
    .crouch_sole_offsets([0.0, 0.0530325, 0.0874534], [0.0, 0.0530325, 0.0874534]) {
    WalkingIK {
        enabled(true)
        stride(0.32)
        step_duration(0.22)
        step_height(0.07)
        step_scale(1.3)
    }
}
```

The sole offsets above are specific to the checked-in Rei(mu)/Bisket shoes.
Stride is base maximum horizontal foot travel in model metres, valid in `[0.05, 1.0]`.
Duration is the relaxed swing time in seconds, valid in `[0.08, 1.0]`; speed and
trailing-foot recovery can shorten it automatically. Height is model metres,
valid in `[0, 0.3]`. `step_scale` is a multiplier in `[0.1, 2.0]` applied to
forward landing prediction, maximum step travel and swing clearance. Its default
is 1.3, including when no component is authored: 30% more reach and lift than the
previous gait (0.416 m maximum travel and 0.091 m clearance at unit avatar scale).
Use `step_scale(1.0)` for the previous distance tuning. This changes foot goals;
the two-bone solve determines thigh and knee angles from reachable geometry. It
does not multiply joint angles or change the calibrated knee bend direction.

The child's settings replace the AVC's walking distances and duration. The
existing `AVC.walking_enabled(false)` remains a master opt-out, and the child can
also disable walking with `enabled(false)`. Crouch calibration stays on AVC.
Only one direct child is allowed; nested model/hand components do not configure
the owning avatar. Settings are read each frame without caching pose state in
the component. Changing tuning affects subsequent steps; active swing endpoints
remain fixed. Defaults are shown above; model distances scale with the avatar.
The idle displacement trigger is 32% of base stride; motion adds half a swing's
predicted travel to directional lag to start steps sooner. The heading trigger
is 25 degrees.

```sh
cargo run --release -- load examples/lower-body-walking-vr.mms
cargo run --release --example lower-body-walking-vr
cargo run --release --example lower-body-walking-vr -- --model rei
```

The MMS scene is directly loadable with Bisket; the optional Rust launcher selects
either model. Both use a retained MMS session with live XR settings callbacks.
The scene supplies a collidable flat floor, a full-height mirror, an XR floor
correction panel, hand lasers/pointers and left-stick locomotion at 0.6 m/s.
The launcher's `--validate` evaluates the scene, imports the
selected GLB and checks mapped legs with XR disabled, without entering the window
loop. The model defaults to Bisket.

With an already calibrated XR floor, check standing still, short forward/backward
and sideways movement, releasing the stick mid-step, reversal, turning, and
crouching while stationary/moving. Compare upright walking with a slight physical
crouch to distinguish target planning from reach limitations.

Rust diagnostics are `systems.lower_body_ik.status(avc)`, `.feet(avc)` and
`.walking(avc)`. Each foot reports rest/planted/swing phase, base/desired/solved
sole positions, desired orientation, support, flexion, residual penetration and
full positional target error. Walking diagnostics expose per-side step counts
and swing side/progress. Gait state and diagnostics are not serialized.

## Current limits and next slices

[Shared AVC setup and untracked walking](shared-avc-setup-and-untracked-walking.md) extends the
initial XR eligibility to desktop and untracked AVCs without changing how movement admits steps.

Unsupported moving avatars need a separate posture policy. Track trailing legs,
relaxed hovering, and takeoff/landing transitions in
[airborne lower-body IK](airborne-lower-body-ik.md). That policy is planned and
will share the existing lower-body scheduler and leg writer.

This slice does not lower or tilt the pelvis. A fully extended upright leg has
insufficient length for an offset fixed contact. Targets remain planted, while
the bounded solution reports positional error and `Limited` status above 1 cm;
it never stretches bones or adjusts tracking to conceal that error. Exact 3D
contact is tested within reachable geometry. Pelvis allowance needs coordinated
head/body/torso ownership in a later slice.

Support covers horizontal upright/yawed static boxes near the capsule bottom.
Slopes, stairs, moving-platform attachment, heel/toe roll, whole-shoe footprint
validation and swept swing-obstacle avoidance remain later work. A swing uses a
fixed landing destination; reversal affects subsequent steps. High speeds and
abrupt direction changes may exceed reach. Running is not an accepted gait.

The displacement threshold is stride-based, with motion anticipation and
adaptive cadence. Predicting support-leg reach through the whole swing and
richer recovery remain later planner improvements. Headset testing covers
ownership, contact retention, swing/landing and crouch composition.

## Automated checks

Run `cargo test --lib lower_body`. The group covers original crouch scenarios,
both real GLBs, sole orientation through body movement, multi-avatar isolation,
alternation at 30/60/120 Hz, earlier motion onset, increasing step length/cadence
and bounded foot lag at 0.25–1.2 m/s, acceleration during swing, and the Sakura
gamepad pace of 1.4 m/s on both real rigs at 30/45/60/90/120 Hz and uneven frame
times. Those pace checks cover upright and crouched posture, forward/backward,
diagonal and sideways travel, with comparable cadence and lag across rates.
The group also covers scaled swing clearance/reach and forward thigh movement on
both real rigs, WalkingIK MMS validation/save-load and avatar scoping,
backward/sideways/diagonal motion, stop-mid-swing and idle stability,
scale/yaw/reversal, contact release/recovery, bad frame times,
bounded unreachable targets, MMS validation/save-load, and the complete avatar
update stage with unchanged tracking/capsule transforms.

Headset appearance, motion comfort and tuning remain to verify.

Validation on 2026-10-28: all 23 lower-body tests pass; the Bisket and Rei(mu)
smoke scenes both pass `--validate`. Targeted Rust formatting and whitespace
checks pass. The broader library run has 55 failures, all reproduced on an
untouched HEAD checkout, with no additional failures from this change. Two
microphone/XR scene tests were excluded from that comparison after stalling in
the initial run. The baseline had one additional panel-factory failure.
