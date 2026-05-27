# Arm + Spine IK — Status Checklist

Date: 2026-03-23

Tracking what has been implemented and what remains to get arms and spine
working with IK across all vtuber/VR examples.

---

## ✅ Done

- **`IKChainComponent` + `IKSolver`** — three solver variants (`AimConstraint`,
  `TwoBoneIK`, `Fabrik`) in `src/engine/ecs/component/ik_chain.rs`
- **`IKSystem`** — scans all `IKChainComponent`s each tick, dispatches to
  solvers; registered in `SystemWorld` after `AvatarControlSystem`
- **`AimConstraint` solver** — head/neck rotation from InputXR; wired in
  `try_init_splices`
- **`TwoBoneIK` solver** — closed-form triangle solve with pole vector, optional
  end rotation copy; builds `[root, first-TC-child, end_effector]` directly to
  avoid non-TC intermediate node issues
- **`BoneMappingSystem`** — stateless; `resolve_arm_chain` with `min_bone_length`
  threshold; topology walk via `tc_ancestor_at_distance`; in
  `src/engine/ecs/system/bone_mapping_system.rs`
- **`AvatarControlComponent`** — added `left/right_upper/lower_arm_bone` fields
  with builder methods, `encode`, `decode`
- **`try_init_splices`** — calls `BoneMappingSystem::resolve_arm_chain`; selects
  arm IK mode vs simple splice mode per hand
- **`vr-input.rs` / `vr-input.mms` arm IK auto-activates** — topology walk
  resolves `J_Bip_L/R_UpperArm` and `J_Bip_L/R_LowerArm` without explicit names

## ✅ Verified working in VR (vr-input-mms, 2026-03-23)

- **Arm IK looks good** — arms reach controllers, no obvious solver artifacts
- **Body / neck** — no obvious regressions

---

## ⚠️ Known issues (pre-existing, not regressions)

- **Hand rotation angle is wrong** — wrist pose offset vs controller. Pre-existing.
- **Camera does not track head pitch** — CameraXR alignment breaks when head
  pitches. Pre-existing; see `docs/spec/avatar-camera.md`.

---

## ❌ Not done / needs work

### Arm IK

- ❌ **Side-specific pole directions** — both arms use `[0, -1, 0]` (elbow down).
  Natural VR pose wants `[-1, -0.5, 0]` / `[1, -0.5, 0]` (elbow out per side).
  Blocked by pole space issue below.

- ❌ **Pole direction body-local space** — world-space pole breaks when body
  rotates. `IKChainComponent` needs a `pole_space: BodyLocal | World` field, or
  AVC should rotate the pole vector by `model_root` world rotation each tick.

- ❌ **Hand rotation smoothing in arm IK mode** — `hand_rotation_smoothing` is
  ignored when arm IK mode activates (only applies to simple splice fallback).
  Add `QuatTemporalFilter` on end-effector rotation if needed.

- ❌ **Update `vr-input.rs` comment block** — lines 337–338 say controllers are
  "re-parented to lower_arm"; no longer true in arm IK mode.

### Spine IK

- ❌ **`TranslationFollow` pipeline op** — body XZ currently tracks HMD exactly.
  A `TranslationFollow` on XZ would let the body lag the head when leaning,
  creating natural spine lean.

- ❌ **Spine IK design** — FABRIK spine chain from hips to neck, driven by the
  head position offset from body. Requires `TranslationFollow` first.

- ❌ **`BoneMappingSystem` spine detection** — find hips and spine chain via
  topology (section 5 of `docs/spec/bone-mapping-system.md`). Not implemented.

### Examples

| Example | Status | Notes |
|---|---|---|
| `vr-input.rs` | ✅ arm IK auto-on | Topology walk resolves VRM arm bones |
| `vr-input.mms` + `vr-input-mms.rs` | ✅ arm IK auto-on | Verified in VR 2026-03-23 |
| `vtuber-desktop.rs` | ➖ N/A | No controllers; head-only AVC |
| `vtuber-joints-example.rs` | ➖ N/A | No `AvatarControlComponent` — desktop joint visualiser |

### Meow meow registry

- ❌ **`with_left/right_upper/lower_arm_bone` not registered** in
  `src/meow_meow/component_registry.rs`. Not needed for the current VRM model
  (topology resolves automatically), but needed for explicit bone name overrides.
