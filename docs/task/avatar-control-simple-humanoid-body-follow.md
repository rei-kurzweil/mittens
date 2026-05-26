# Task: Avatar control simple humanoid body follow

Replace the current spine-FABRIK body-follow experiment in AVC with a simpler,
more stable heuristic body module. The head/camera lock that was just landed is
the foundation; the body should now follow the pose driver in a limited,
predictable way without using spine IK.

Target implementation surface:

- `src/engine/ecs/system/ik/head_rotation_sensitive_body_xz_translate_follow.rs`
  (working name; can be shortened in code, e.g. `head_pose_body_xz_follow.rs`)
- `src/engine/ecs/system/avatar_control_system.rs`

The previous planar XZ-deadzone heuristic (`src/engine/ecs/system/ik/simple_humanoid.rs`,
landed in an earlier pass) is **scrapped** — it could not distinguish HMD motion
caused by walking from HMD motion caused by head rotation, so in VR it produced
constant lag behind the head and the neck visibly stretched away from the body.
The replacement removes the head-rotation contribution from the HMD pose before
deciding where the body should sit, so the body anchor stays under the real
neck regardless of how the head is oriented.

## Problem statement

The head/camera relationship is now stable because the visible head is mounted
under a dedicated driven node rather than being jointly owned by the spine
solver. That solved the hardest VR issue.

The remaining body behavior is still wrong for the current product goal:

- the torso pitches forward because the spine is still being asked to solve
  toward a target derived from the HMD/camera offset,
- the current AVC-installed arm IK / FABRIK path is also not working well
  enough to keep as the baseline and should be re-implemented separately,
- the neck can stretch / translate instead of behaving like a rotational joint,
- the current behavior is overcomplicated for the near-term need,
- crouch / kneel posture should eventually come from authored animation, not
  from pushing a procedural spine IK chain harder.

### Why a planar XZ deadzone follow is the wrong tool for VR

A naive "body lags behind the head's XZ position with a deadzone" rule fails as
soon as you remember the HMD has two independent ways to move in XZ:

- **A. The user actually moved** (walking, leaning, stepping). The body should
  follow this 1:1.
- **B. The user only rotated their head** (pitch, roll, yaw — and any
  combination). The HMD physically translates in world space because it
  swings around the real neck/skull pivot, but the user's body has *not*
  moved. The body should not follow this at all; in some cases it should
  even counter-translate.

Concretely:

- pitching the head down rotates the HMD forward around the neck — the HMD's
  world-space XZ moves forward, but the body's true XZ is still behind it.
  Naive follow drags the body forward; the correct behavior is for the body
  to stay put (which, relative to the HMD, looks like the body shifting
  slightly backward),
- rolling the head sideways shifts the HMD slightly in the direction of the
  roll for the same reason; the body should not chase that shift, and
  relative to the HMD it ends up nudged in the opposite direction of the
  roll,
- pure yaw at a stationary body doesn't translate the HMD much, but to the
  extent it does (off-center eye/HMD origin), the same rule applies.

A deadzone hides this only at small angles. At any meaningful head pitch the
body either lags visibly behind the head (deadzone too generous) or jitters
forward and back as you nod (deadzone too tight). There is no setting that
makes "lag in XZ" correct.

The desired near-term behavior is therefore not a deadzone but a *pose
compensation*:

- the head stays locked to the pose driver plus authored camera offset,
- the body's XZ anchor is the HMD's XZ minus the head-rotation-induced
  contribution to the HMD's world position, i.e. an estimate of where the
  real neck base actually is in world space,
- the body follows that estimated neck-base XZ directly (no deadzone needed —
  rotation-only HMD motion already cancels out in the estimate),
- the body never inherits pitch/roll from the pose driver,
- only yaw-follow is allowed on the body,
- the neck should not translate or stretch; it should only rotate.

## Target design

```text
driven_t (HMD / InputXR pose)            ← head_world_pose: T_h, R_h
├── fixed_head_target / visible head mount
│   └── J_Bip_C_Head
└── head-rotation-sensitive body XZ follow
    └── model_root / body anchor
    └── body_anchor_xz = (T_h - R_h * v_head_to_neck_local).xz
            └── avatar skeleton up to neck

where v_head_to_neck_local is the head-local vector from the HMD/eye origin
to the real neck base — roughly (0, -neck_to_eye_height, ~0) in head-local
coords for an upright user. R_h * v is that vector expressed in world space;
subtracting it from the HMD world position gives an estimate of where the
real neck base sits, which is exactly where the body should anchor.

Body XZ snaps to this estimate every tick (optionally low-pass filtered for
tracker jitter). Body Y stays at the calibrated avatar height; body yaw
continues to be handled by the existing QuatYawFollow op.

neck:
- rotation allowed
- no translation solve
- no stretch

future:
- crouch/kneel state derived from calibrated headset height delta
- delegated to avatar_animation_system blending authored poses
```

### Behavior under the four motion cases

| User motion           | HMD world XZ change | R_h * v_head_to_neck change | Body anchor (HMD - R_h*v) |
| --------------------- | ------------------- | --------------------------- | ------------------------- |
| Walking / leaning     | yes                 | none                        | follows 1:1               |
| Pure yaw (in place)   | small / none        | rotates around Y            | unchanged in XZ           |
| Pitch down (in place) | forward             | forward                     | unchanged (forward cancels) |
| Roll sideways         | sideways            | sideways (same direction)   | unchanged (cancels)       |

The same expression handles all four. There is no deadzone, threshold, or
follow rate to tune — only the head-local neck-offset vector, which is a
calibration constant per avatar/user.

## Non-goals for this task

- no spine FABRIK for normal body follow,
- no arm FABRIK in the first body-follow phase,
- no procedural crouch solved through spine IK,
- no attempt to make the torso exactly match the HMD pitch,
- no planar XZ deadzone heuristic — the previous attempt at this is being
  scrapped because it cannot distinguish locomotion from head rotation (see
  "Why a planar XZ deadzone follow is the wrong tool for VR" above),
- no separate X-rule and Z-rule; the head-rotation-sensitive estimate already
  produces both X and Z from one expression,
- no new transform-stream operator in the first implementation; start as
  AVC-adjacent policy code first,
- no backward-compat support for old AVC behavior.

## Phase 1 — head-rotation-sensitive body XZ translate follow

### Step 0 — pass-through smoke test (land first, throw away after)

Before the head-rotation compensation goes in, the new module should first
run as a trivial **pass-through**: each tick it just copies `driven_t`'s
world XZ into `body_anchor_xz`. The body's Y stays at the cached avatar-
height offset (`model_root_local_y`).

Why land this stub first:

- it proves the new module is wired correctly into `SystemWorld`, replacing
  the scrapped `SimpleHumanoidSystem` cleanly,
- it proves `model_root.local.translation` is being computed correctly from
  the parent (body_pipeline) world matrix — i.e. the inverse-parent math
  used to convert a target world XZ back to a local TRS write is right,
- it proves the body actually stays beneath the head when walking,
  independent of any neck-offset calibration,
- it isolates "is the plumbing correct?" from "is the head-rotation
  compensation correct?". If pass-through is broken, no formula on top of
  it can be right.

Expected pass-through behavior:

- when walking / leaning, the body translates 1:1 with the HMD in XZ,
- when only rotating the head (pitch / roll / yaw in place), the body
  *will* incorrectly translate along with the HMD's small rotation-induced
  XZ wobble — this is the exact wrong behavior the full Phase 1 formula
  fixes, and is expected at this step,
- the body sits directly below the HMD in XZ and at `model_root_local_y`
  below it in Y, with no horizontal neck stretch from the body lagging
  behind,
- the visible head still tracks the HMD via the head_target mount path
  (unchanged).

Pass-through formula:

```text
body_anchor_xz = (T_h).xz      // i.e. driven_t.world.translation.xz
body_world_y   = (T_h).y + model_root_local_y
```

This step has no calibration constants and no tunables. If it does not
produce a body that stays glued under the HMD while walking, the bug is in
the model_root local-write math, not in the follow rule — fix that before
adding compensation.

### Step 1 — head-rotation compensation (the actual Phase 1 behavior)

Once Step 0 looks correct, extend the same module with the
head-rotation-sensitive estimate:

- add `src/engine/ecs/system/ik/head_rotation_sensitive_body_xz_translate_follow.rs`
  (shorter filename is fine in code; the doc uses the descriptive form),
- delete or empty out `src/engine/ecs/system/ik/simple_humanoid.rs` and remove
  its registration from `SystemWorld` — it is not coexisting with the new
  module, it is being replaced,
- preserve the existing head/camera mount path (head_target under driven_t,
  head bone re-parented under head_target) exactly as it is today,
- keep the body yaw pipeline (`QuatYawFollow`) — this module owns translation
  only, not rotation,
- the new module computes a *body anchor world XZ* from the HMD pose every
  tick:

  ```text
  v_local  = head-local vector from HMD origin to real neck base
             (calibration constant; see below)
  T_h, R_h = HMD world translation and rotation (from driven_t)

  body_anchor_xz = (T_h - R_h * v_local).xz
  ```

- write `model_root.local.translation` so `model_root.world.xz` lands at
  `body_anchor_xz` and `model_root.world.y` stays at the existing avatar-height
  rest value,
- no deadzone, no follow rate, no threshold — the compensation removes
  rotation-induced HMD motion before the follow, so following the result 1:1
  is correct,
- optionally apply a light low-pass filter on `body_anchor_xz` to smooth
  tracker jitter; jitter is the only reason to filter, not gameplay feel,
- body orientation stays yaw-only,
- body must not inherit pose-driver pitch or roll,
- no spine IK in this phase,
- no arm IK in this phase,
- do not introduce a new transform-stream operator unless reuse pressure shows
  up after the AVC-specific version is proven.

Step 1 is a one-line change on top of Step 0: replace `T_h.xz` with
`(T_h − R_h * v_local).xz`. The model_root local-write code (the
inverse-parent math) is identical between the two steps.

### Calibrating `v_local`

`v_local` is the head-local vector from the HMD origin to the user's real
neck base. In a head-local frame where +Y is up and -Z is forward (OpenXR
convention), an upright user has the neck base roughly **below** and very
slightly **behind** the HMD, so `v_local` is approximately
`(0, -neck_to_eye_height, +small)`.

Reasonable starting point: `v_local = (0.0, -0.12, 0.02)` in metres, then
tune `-y` against the existing `eye_height_from_head_bone` /
`head_ik_eye_height` settings so that with the head upright the body
anchor sits exactly under the HMD.

Expose this on `AvatarControlComponent` as a single `[f32; 3]` field (e.g.
`head_to_neck_offset_local`) with a builder. Do not split it into separate
height / forward / lateral fields — it's one vector.

### Expected AVC integration

- keep the fixed head/camera mount path exactly as-is,
- remove the scrapped `SimpleHumanoidSystem` field, tick site, mod entry, and
  the supporting fields on `AvatarControlComponent`
  (`body_planar_deadzone`, `body_planar_follow_rate`, `body_anchor_world_xz`,
  `body_anchor_initialized` — these no longer carry meaning),
- keep `model_root_id`, `model_root_local_y`, `neck_bone_id`,
  `neck_rest_translation` — the new module needs the same plumbing,
- add `head_to_neck_offset_local: [f32; 3]` (default `[0.0, -0.12, 0.02]`)
  and a builder,
- route `model_root` XZ updates through the new module; Y stays at the
  cached rest offset.

### Acceptance criteria

**Step 0 (pass-through smoke test):**

- walking / leaning translates the body 1:1 with the HMD in XZ,
- the body sits directly below the HMD with no horizontal neck stretch
  while standing still and not rotating the head,
- head/camera lock remains stable,
- AVC no longer depends on the spine FABRIK or arm IK setups for baseline
  VR body behavior.

**Step 1 (head-rotation compensation, the real Phase 1 goal):**

- everything from Step 0, plus:
- a pure head pitch (looking up/down) does not translate the body in XZ,
- a pure head roll does not translate the body in XZ,
- a pure head yaw does not translate the body in XZ,
- the neck does not visibly stretch horizontally away from the body during
  any head rotation.

## Phase 2 — neck constraints and rigid upper chain behavior

Once body follow is heuristic-driven, fix the neck joint behavior explicitly.

Requirements:

- neck may rotate, but must not translate,
- neck may not stretch,
- upper torso → neck relation should remain rigid in translation,
- if any procedural solve remains in this area, it must preserve authored bone
  lengths exactly.

This phase may use one of two approaches:

- remove neck translation writes entirely and keep only rotational updates, or
- keep a constrained solve but clamp the neck to pure rotational behavior.

Acceptance criteria:

- neck length is visually stable while looking around,
- no visible telescoping / stretching,
- no camera-relative drift introduced by the neck fix.

## Phase 3 — arm IK reimplementation

Reintroduce arm IK only after the body-follow and neck behavior are stable.

Scope:

- arm FABRIK is a separate concern from the head-rotation-sensitive body
  XZ follow module,
- do not couple arm solving to the body-follow module unless later
  implementation experience proves that shared ownership is simpler,
- AVC may still be the integration point, but the arm solver logic should live
  in its own implementation surface,
- rebuild arm solve behavior against the simplified body/head baseline rather
  than trying to preserve the current failing setup.

Initial expectation:

- arms should be reintroduced after Phases 1 and 2 are working,
- arm targets / constraints should be revisited from scratch,
- arm solve success should not depend on torso pitch compensation from spine IK.

Acceptance criteria:

- arm IK is restored only after the simplified body baseline is stable,
- the new arm implementation is independent from the removed AVC arm IK path,
- head/camera and neck stability are not regressed by reintroducing arms.

## Phase 4 — avatar animation for crouch / kneel

Replace procedural body-drop behavior with authored animation blending.

Plan:

- calibrate standing headset height at init (or when XR becomes active),
- measure headset vertical delta from that standing baseline,
- once the delta passes a configurable threshold, derive a crouch amount,
- delegate that crouch amount to a future `avatar_animation_system`,
- blend authored crouch / kneel / sit poses based on that amount.

The head-rotation-sensitive body XZ follow module remains responsible for:

- stable body X/Z follow under the pose-compensated rule (HMD − R_h * v_local),
- maintaining the head/body separation of concerns.

Yaw follow continues to live in the existing `QuatYawFollow` transform-stream
op on the body pipeline — it is not part of the new module.

The avatar animation system becomes responsible for:

- body compression / crouch pose,
- kneel transitions,
- future posture-specific polish.

Acceptance criteria:

- lowering the headset below the standing threshold does not procedurally crush
  the torso,
- crouch is animation-driven and blendable,
- returning to standing restores the idle pose cleanly.

## Documentation follow-up

After the implementation phases above land, audit and refresh stale AVC docs.

Likely affected docs:

- `docs/task/avatar-control-head-driven-redesign.md`
- any AVC comments / topology diagrams in `src/engine/ecs/component/avatar_control.rs`
- any examples or comments that still describe spine FABRIK as the current body
  follow path.
- any docs or comments that still imply the current AVC arm IK path is part of
  the retained baseline.

Update them to reflect:

- fixed head mount under the pose driver,
- head-rotation-sensitive body XZ translate follow (HMD − R_h * v_local) as
  the body-translation rule,
- the scrapped planar XZ deadzone heuristic is **not** the body-follow rule
  and should not be referenced as the current approach,
- no spine IK for normal body follow,
- arm IK removed from the initial AVC rewrite and reintroduced later as a
  separate concern,
- avatar-animation ownership of crouch/kneel behavior.