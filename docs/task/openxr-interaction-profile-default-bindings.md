# Task: OpenXR interaction profiles and sensible default bindings

Date: 2026-06-28

Status: active

This task narrows the remaining XR controller-input problem to the current OpenXR reality:

- Cat Engine is OpenXR-only again
- the runtime on this machine reports
  `/interaction_profiles/oculus/touch_controller`
- only part of the current action map is actually live on the tested hardware/runtime
- `ButtonB` is confirmed live
- several other accepted bindings still report inactive state
- several alias / click-style paths are outright unsupported

This note exists to make the next pass concrete:

- interaction profiles should be manageable
- default binding suggestions should be sane for common runtimes
- unsupported aliases should stop polluting the active map
- value-vs-click differences should be handled deliberately
- debug/UI oddities around face buttons should be investigated separately from the low-level
  binding report

Related context:

- [docs/task/openxr-controller-actions-and-default-stick-locomotion.md](./openxr-controller-actions-and-default-stick-locomotion.md)
- [docs/task/shared-xr-backend-abstraction-and-openvr-followup.md](./shared-xr-backend-abstraction-and-openvr-followup.md)
- [src/engine/ecs/system/openxr_system.rs](../../src/engine/ecs/system/openxr_system.rs)
- [src/engine/ecs/system/input_xr_gamepad_system.rs](../../src/engine/ecs/system/input_xr_gamepad_system.rs)
- [examples/xr-input-gamepad.mms](../../examples/xr-input-gamepad.mms)

---

## 1. Current evidence

The latest binding-report instrumentation showed:

- active profile left:
  - `/interaction_profiles/oculus/touch_controller`
- active profile right:
  - `/interaction_profiles/oculus/touch_controller`
- accepted Touch-style paths:
  - `thumbstick/x`
  - `thumbstick/y`
  - `trigger/value`
  - `squeeze/value`
  - `x/click`
  - `y/click`
  - `a/click`
  - `b/click`
  - `aim/pose`
  - `grip/pose`
- unsupported paths:
  - `joystick/x`
  - `joystick/y`
  - `trigger/click`
  - `grip/value`
  - `squeeze/click`
  - `grip/click`
- live-state snapshot at report time:
  - `B` was the only face-button action already reporting `is_active=true`
  - `A`, `X`, and `Y` were present but inactive in that snapshot
  - thumbsticks / trigger value / grip value were present but inactive in that snapshot
  - aim/grip poses were accepted at bind time but invalid in the same early runtime snapshot

That gives two clear conclusions:

1. the current Touch profile defaults still include bad aliases and dead click-style paths
2. binding acceptance alone is not enough; we still need sensible runtime defaults and better
   validation of when controls actually become active

---

## 2. Main goal

The immediate goal is:

- make the OpenXR interaction-profile layer manageable and explicit
- make default bindings sane for the runtime-reported profile on common controller runtimes
- stop relying on unsupported click aliases where value-driven controls are the practical source
- preserve a stable engine-facing gamepad shape even if low-level binding policies change

This task is about the OpenXR profile/binding layer, not backend selection and not locomotion
policy by itself.

---

## 3. Scope

This task should cover:

- profile-specific binding defaults in `OpenXRSystem`
- reducing bad alias suggestions for known profiles
- defining fallback/default policy for value-driven controls
- making the binding report actionable and low-noise
- checking why `A/X/Y` sometimes do not appear consistently in the debug UI even though their
  binding paths were accepted

This task should not cover:

- restoring OpenVR
- redesigning the engine-facing `XrGamepadState` shape from scratch
- authored locomotion defaults beyond what is needed to expose correct stick/button state
- unrelated XR rendering architecture work

---

## 4. Problems to solve

### A. Unsupported-path noise

The current Touch profile still suggests aliases that are known-bad on the tested runtime:

- `joystick/*`
- `trigger/click`
- `grip/value`
- `squeeze/grip click`

Those should not remain part of the ordinary default path once their failure is established.

### B. Value-vs-click policy

The runtime accepted:

- `trigger/value`
- `squeeze/value`

but rejected:

- `trigger/click`
- squeeze/grip click variants

That means the engine should likely treat:

- trigger pressed
- grip pressed
- select-like activation

as value-threshold-derived on this profile unless a true click path is known to work.

### C. Accepted-but-inactive controls

Some actions were accepted but still inactive in the snapshot:

- `A`
- `X`
- `Y`
- thumbsticks
- trigger/grip values
- aim/grip poses

This might be normal for an early snapshot, or it might indicate:

- runtime-side activation rules
- wrong polling assumptions for some action types
- a need for post-focus validation rather than immediate one-shot conclusions

### D. Debug/UI inconsistency

The user reported that `X` and `Y` sometimes do not render on the debug UI.

That is odd because:

- the bindings for `x/click` and `y/click` were accepted
- the low-level snapshot object did contain `left_x` / `left_y`

Possible buckets:

- UI rendering conditional on `is_active`
- stale or partial `InputVRGamepad` publication
- data-renderer / MMS-side visibility issue
- left-hand button state not surviving one layer of translation cleanly

This should be treated as a separate symptom to trace, not as proof that the binding itself failed.

---

## 5. Desired profile-management direction

The OpenXR layer should move toward this policy:

### A. Runtime-reported profile is authoritative

Do not infer the active binding map from hardware branding.

Instead:

- query the active runtime profile
- select the profile-specific default binding policy for that path
- apply known-good defaults for that profile

### B. Default suggestions should be conservative

Prefer:

- one known-good path

over:

- large alias bundles with dead legacy variants

unless multiple variants are genuinely needed for real runtimes.

### C. Boolean gameplay semantics may come from float actions

For controls like trigger and grip:

- keep the raw analog value when available
- derive pressed/down/released via a clear threshold policy
- only use click actions when they are confirmed supported and useful

### D. Engine-facing state should remain stable

Even if profile logic changes underneath, higher layers should still consume the same broad shape:

- face buttons
- thumbsticks
- trigger/grip values
- trigger/grip pressed state
- select-equivalent state where needed

---

## 6. Concrete next implementation steps

1. Remove known-dead Touch aliases from the default suggestion set:
   - `joystick/x`
   - `joystick/y`
   - `trigger/click`
   - `grip/value`
   - `squeeze/grip click` variants

2. Remap Touch `select_left` / `select_right` away from `trigger/click`:
   - first candidate: `trigger/value`-driven threshold semantics

3. Add explicit threshold policy for value-driven controls:
   - trigger pressed threshold
   - grip pressed threshold
   - optional select threshold if `select` remains a logical action rather than a direct path

4. Improve validation after session focus:
   - log whether accepted controls ever become active after a short focused-session window
   - distinguish "accepted but not yet touched" from "accepted but never active"

5. Trace the `X` / `Y` debug-UI inconsistency:
   - verify low-level snapshot
   - verify `XrGamepadState` publication
   - verify `InputVRGamepadSystem` propagation
   - verify debug UI rendering conditions

---

## 7. Exit criteria

This task is complete when:

- the default Touch-profile binding map no longer contains the currently known-dead aliases
- trigger/grip/select semantics are sensible on the tested runtime even without click paths
- `A/B/X/Y` visibility is consistent between low-level debug snapshots and the debug UI
- thumbstick/trigger/grip defaults are understandable enough to serve as the engine's practical
  OpenXR baseline for common controller runtimes

It is acceptable if additional profiles still need later tuning, as long as:

- the runtime-reported profile path is handled intentionally
- defaults are conservative rather than noisy
- the tested runtime no longer depends on lucky partial behavior like "only `B` works"
