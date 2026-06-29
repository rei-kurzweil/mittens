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

The practical target is not "every OpenXR control layout at once". The near-term target is the
common motion-controller shape used by modern HMDs:

- dual sticks where available
- dual trigger analog values
- dual grip / squeeze values or clicks
- common face buttons (`A/B/X/Y`) where present

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

More concretely, the generic engine-facing controller shape we want to support first is:

- left stick
- right stick
- left trigger value + pressed state
- right trigger value + pressed state
- left grip value/click + pressed state
- right grip value/click + pressed state
- left face buttons (`X`, `Y`) when present
- right face buttons (`A`, `B`) when present

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

### E. Binding policy is still too alias-oriented

The current implementation in `OpenXRSystem` is still mostly:

- profile path
- a list of suggested raw paths
- a direct raw-path choice for `select`

That was good enough for bring-up, but it is not the cleanest way to express the real goal:

- "thumbstick if this profile has one"
- "trigger analog if exposed"
- "grip analog if exposed, otherwise click fallback"
- "face buttons when exposed"
- "select as a logical semantic, not necessarily a literal hardware click path"

The next pass should think in terms of semantic capability first, then choose the best raw path
for that profile.

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

## 6. Generic controller target

For common HMD motion controllers, the default engine-facing capabilities should be:

- locomotion stick left: 2D analog when present
- locomotion stick right: 2D analog when present
- trigger left/right: analog value plus derived pressed state
- grip left/right: analog value if available, otherwise click-backed pressed state
- face buttons left/right: `X/Y` and `A/B` when present
- aim and grip pose

This does **not** mean every profile must provide every capability.

The rule should be:

- missing capabilities stay `None`
- present capabilities map into one stable engine slot
- higher layers do not care whether the low-level source was `squeeze/value`, `grip/click`, or
  a threshold derived from analog input

### A. Main profile families

For the practical near-term target, the important families are:

1. Touch-like:
   - Oculus / Meta Touch
   - HTC Vive Focus 3

2. Valve Index

3. Microsoft motion controller

4. Legacy Vive wand

The first three are the real "generic modern HMD controller" baseline.
Legacy Vive still matters, but it should be treated as a click-first fallback profile rather than
the shape that drives default policy for everyone else.

### B. Minimal default policy for modern dual-stick controllers

For modern motion-controller profiles, the default policy should be:

- prefer `thumbstick/x` + `thumbstick/y`
- prefer `trigger/value`
- prefer `squeeze/value` for grip when supported
- use face-button click paths where the profile exposes them
- derive pressed semantics from analog values when click actions are absent or known-dead

### C. Select policy

`select` is an engine semantic, not necessarily a dedicated hardware button.

For modern motion controllers:

- trigger should remain the default select source
- use a real trigger click path only when it is actually reliable on that profile/runtime
- otherwise derive select from trigger value threshold

That keeps pointer activation stable even across runtimes that disagree on whether trigger is
primarily a click action or a float action.

### D. Profile-specific expectations

Touch-like profiles:

- dual thumbsticks
- trigger analog
- squeeze analog
- `X/Y` on left
- `A/B` on right

Index:

- dual thumbsticks
- trigger analog
- squeeze analog
- face buttons only where actually exposed/verified

Microsoft motion:

- dual thumbsticks
- trigger analog
- grip may be click-first

Legacy Vive:

- click-first trigger/grip semantics
- no modern dual-stick assumption

### E. Implication for current code

In the current `OpenXRSystem` tables, this likely means:

- Touch-like profiles should stop carrying the known-dead `joystick/*`, `trigger/click`,
  `grip/value`, and click-style squeeze/grip aliases once verified dead
- Index should remain analog-first
- Microsoft motion should remain mixed analog/click
- Vive wand should remain click-first
- `select_left` / `select_right` should be treated as a semantic default source, not a permanent
  insistence on one raw OpenXR path

---

## 7. Concrete next implementation steps

1. Split the profile policy mentally into:
   - modern dual-stick controllers
   - legacy click-first controllers

2. Remove known-dead Touch aliases from the default suggestion set:
   - `joystick/x`
   - `joystick/y`
   - `trigger/click`
   - `grip/value`
   - `squeeze/grip click` variants

3. Remap Touch `select_left` / `select_right` away from `trigger/click`:
   - first candidate: `trigger/value`-driven threshold semantics

4. Add explicit threshold policy for value-driven controls:
   - trigger pressed threshold
   - grip pressed threshold
   - optional select threshold if `select` remains a logical action rather than a direct path

5. Make the modern generic-controller target explicit in code comments / tables:
   - dual sticks when present
   - trigger analog
   - grip analog-or-click
   - `A/B/X/Y` when present

6. Improve validation after session focus:
   - log whether accepted controls ever become active after a short focused-session window
   - distinguish "accepted but not yet touched" from "accepted but never active"

7. Trace the `X` / `Y` debug-UI inconsistency:
   - verify low-level snapshot
   - verify `XrGamepadState` publication
   - verify `InputXRGamepadSystem` propagation
   - verify debug UI rendering conditions

---

## 8. Exit criteria

This task is complete when:

- the default Touch-profile binding map no longer contains the currently known-dead aliases
- trigger/grip/select semantics are sensible on the tested runtime even without click paths
- the engine has a clear "generic modern HMD controller" policy covering:
  - dual sticks
  - dual trigger values
  - grip / squeeze
  - common face buttons
- `A/B/X/Y` visibility is consistent between low-level debug snapshots and the debug UI
- thumbstick/trigger/grip defaults are understandable enough to serve as the engine's practical
  OpenXR baseline for common controller runtimes

It is acceptable if additional profiles still need later tuning, as long as:

- the runtime-reported profile path is handled intentionally
- defaults are conservative rather than noisy
- the tested runtime no longer depends on lucky partial behavior like "only `B` works"
- legacy click-first controllers are still supported without distorting the modern default policy
