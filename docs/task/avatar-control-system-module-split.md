# Avatar control system module split

Status: steps 1–3 done (2026-11-16). Step 4 lands with the first
[body solver](body-solver-horizontal-mode.md) slice.

## Observation

`src/engine/ecs/system/avatar_control_system.rs` is about 4,450 lines. Roughly
2,380 are production code and 2,070 are two inline test modules. The upper
body solver needs to take over arm IK setup, which currently sits in the
middle of the 554-line `try_init_pose_drivers`.

Line ranges are as of 2026-11-16.

| Responsibility | Lines | Approx. size |
| --- | --- | ---: |
| System entry: struct, `tick`, `tick_one`, register/remove, capsule gate hooks | 35–237 | 200 |
| Mouth amplitude (`update_amplitude_mouth_open`) | 238–332 | 95 |
| Eye gaze: tracking, head-motion freeze policy, limits, rest restore | 333–648 | 315 |
| Rig topology helpers: camera anchor refresh, model root, owning input lookups, ancestry, uniform scale | 649–810 | 160 |
| Capsule: routing, contact validity, init, readiness events, diagnostics, fallback height | 811–1283 | 470 |
| Arm IK binding and eligibility | 1284–1369 | 85 |
| `try_init_pose_drivers`: hand discovery, head splice, camera bone and eye offset, height calibration, neck rest, body operator, arm chains, camera re-parenting | 1370–1923 | 554 |
| Hands: grab attachment, targets, aim correction, map hand basis, pose corrections, alignment debug | 1924–2301 | 380 |
| Rest-pose helpers (`read_bone_rest_pose`, `rest_model_relative_to`) | 2302–2379 | 80 |
| Tests: `hand_pose_correction_tests` (covers eyes, mouth, head, arms and hands despite its name) and `capsule_tests` | 2380–4447 | 2,070 |

`avatar_eye_blink.rs` is already separate, included through `#[path]`.
`avatar_control_system/first_person_test_support.rs` already exists.

The rest-pose helpers are also used by `ik/head_neck_body_constraint.rs`,
`ik/lower_body/rig.rs` and `rest_attachment.rs`. The IK modules therefore
depend on the avatar control system for a generic utility.

## Proposed layout

```
avatar_control_system/
  mod.rs          AvatarControlSystem, tick, tick_one, register/remove
  init.rs         try_init_pose_drivers as an ordered sequence of steps
  head_mount.rs   camera discovery, eye offset, model-root height
                  calibration, neck rest, head mount and constraint binding,
                  camera re-parenting
  body_operator.rs  body operator creation and model-root re-parenting
  arm_ik.rs       ArmIkBinding, eligibility, arm chain construction
                  (moves to ik/upper_body/arms.rs in step 4)
  capsule.rs
  hands.rs        controller discovery, hand preparation, lasers and grab
                  attachments, targets, aim correction, pose corrections
  eyes.rs
  eye_blink.rs    moved from avatar_eye_blink.rs; no #[path]
  eye_blink/tests.rs
  mouth.rs
  topology.rs     owning-input lookups, ancestry, model root, camera anchor,
                  map_target, emit_attach, update_local_rotation
  first_person_test_support.rs
  tests/          mod.rs (shared helpers), eyes, mouth, head, hands, arms, capsule
ik/rest_pose.rs   read_bone_rest_pose, rest_model_relative_to
```
Module boundaries follow component families, not individual functions. Keep
helpers as `pub(super)` free functions: no traits or runtime dispatch.

## Order

1. **Moves without behavior changes**: done as one commit. Same 35 tests.
2. **`ik/rest_pose.rs`**: move the shared helpers and update the three
   external callers. This removes the IK → avatar control dependency.
3. **Break up `try_init_pose_drivers`** into `init.rs` steps that pass an
   explicit context struct (resolved bones, camera children, eye offset). This
   is initialization-only code, so no per-frame cost can change.
4. **Move arm IK binding to `ik/upper_body/arms.rs`** as the first upper
   body solver commit. Initially it still creates `IKChainComponent`s that
   `ik_system` solves; ownership moves to the upper body system later.

## Performance constraints

- `tick` keeps one pass over registered avatars, with no new per-frame
  allocations, traits, or boxed dispatch.
- Steps 1–3 never touch the per-frame paths; step 4 only touches initialization.

## Validation

- After each step: `cargo test --lib avatar_control`, `head_neck`,
  `lower_body`, `ik_system`, `rest_attachment`, `multiplayer` (twilight and
  two-peer avatar tests) and `openxr_avatar`. Same pass and fail counts as
  before; the full suite currently has 100 failures that already fail on `main`.
- `git diff --stat` for each move-only commit is mostly renames and additions
  with matching deletions.
