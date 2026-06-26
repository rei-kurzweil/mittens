# Task: OpenXR controller actions and default stick locomotion

Date: 2026-06-25

This task narrows the current XR input work to the main blocked goal:

- make OpenXR controller actions actually become active on Vive Focus 3 controllers
- get thumbsticks/buttons/triggers flowing through `InputVRGamepad`
- expose that state reliably to MMS events
- make AVC + `InputVR` + `VRHand` use one or both sticks for locomotion by default unless authored otherwise

This task is intentionally about the **controller action path**, not the hand-tracking pose-quality
problem.

Related context:

- [docs/task/xr-gamepad-and-hand-input-refactor.md](./xr-gamepad-and-hand-input-refactor.md)
- [docs/task/openxr-per-hand-input-state.md](./openxr-per-hand-input-state.md)
- [docs/draft/shared-xr-backend-abstraction-openxr-openvr.md](../draft/shared-xr-backend-abstraction-openxr-openvr.md)
- [src/engine/ecs/system/openxr_system.rs](../../src/engine/ecs/system/openxr_system.rs)
- [src/engine/ecs/system/input_xr_gamepad_system.rs](../../src/engine/ecs/system/input_xr_gamepad_system.rs)
- [src/engine/ecs/component/input_xr_gamepad.rs](../../src/engine/ecs/component/input_xr_gamepad.rs)
- [src/engine/ecs/component/avatar_control.rs](../../src/engine/ecs/component/avatar_control.rs)
- [examples/xr-input-gamepad.mms](../../examples/xr-input-gamepad.mms)

---

## 1. Main goal

The immediate product goal is:

- a Vive Focus 3 controller thumbstick should produce usable MMS events in-engine
- the same path should also work on other OpenXR controller runtimes where possible
- AVC in XR should then be able to consume stick input for locomotion by default

Right now this is blocked at a lower layer:

- OpenXR action objects are created
- the XR session reaches `FOCUSED`
- `InputXRGamepadSystem` is alive
- but controller interaction profiles remain `none`
- and all controller action states remain inactive

So before changing authored defaults, the controller action/profile path itself must be made real.

---

## 2. What is already known

The current debugging work has already established:

- hand-root fallback can drive `VRHand` independently of controller actions
- hand tracking and pinch inference can be debugged separately
- those hand-tracking results do **not** explain the dead thumbsticks/buttons

The controller-specific evidence so far is:

- no active interaction profile for `/user/hand/left`
- no active interaction profile for `/user/hand/right`
- aim/grip controller action poses invalid
- button/stick/trigger actions sampled but always inactive
- `InputXRGamepadSystem` does not appear to be the root problem

That means the current blocker is still inside OpenXR controller input setup or runtime activation.

---

## 3. Scope for this task

This task should cover:

- controller interaction-profile binding suggestions
- controller action creation and sync validation
- Focus 3-specific path validation
- thumbstick/button/trigger activation proof
- MMS event delivery from live controller inputs
- default XR locomotion consumption of sticks in AVC once the low-level path is alive

It should not cover:

- full hand-finger/avatar retargeting
- wrist-vs-palm hand-root quality investigation
- general hand-tracking smoothing
- replacing internal engine type names in this task

---

## 4. Desired runtime behavior

### A. Controller input layer

When Focus 3 controllers are active in a focused XR session:

- OpenXR should report a non-`none` active interaction profile per hand
- thumbstick axes should update continuously
- trigger/grip/button actions should become active and toggle
- `OpenXRSystem` should publish those values into the engine-owned XR gamepad state
- `InputXRGamepadSystem` should emit matching MMS events

### B. Authored locomotion default

For the common authored XR rig shape:

```text
InputVR
  Transform
    AVC
      ...
    VRHand(left)
    VRHand(right)
    InputVRGamepad
```

desired default behavior is:

- HMD continues to provide head pose / root pose information through `InputVR`
- `VRHand` continues to provide left/right pose drivers for authored XR hand/controller transforms
- `InputVRGamepad` provides button/stick/controller state
- AVC uses one or both controller sticks for locomotion by default unless explicitly configured not to

This should be treated as a higher-level follow-up after controller actions are proven alive.

---

## 5. Questions this task must answer

### Controller profile / binding questions

1. Which interaction profile does the runtime actually activate for Focus 3 controllers?
2. Is it `/interaction_profiles/htc/vive_focus3_controller`, a fallback HTC profile, or something else?
3. Are the suggested binding paths correct for the active runtime profile?
4. Are we binding `thumbstick`, `trigger`, `select`, `grip`, `x/y/a/b` to the right OpenXR paths?
5. Is the action set attached and synced correctly for the session that becomes focused?

### Event / engine questions

6. Once actions are alive, do thumbstick changes reach MMS handlers correctly?
7. Is the current `InputVRGamepad` state shape enough for default locomotion, or does AVC need a cleaner consumption path?
8. Should AVC read stick locomotion directly from `OpenXRSystem`, from `InputVRGamepad`, or from a more generic engine-level input abstraction?

---

## 6. Immediate debugging steps

The next concrete debugging work should stay narrow and evidence-driven.

### Step 1: Binding dump at init

Add debug output that clearly prints:

- each controller interaction profile we suggest bindings for
- the exact left/right paths used for:
  - `thumbstick`
  - `select`
  - `trigger_value`
  - `trigger_click`
  - `grip_value`
  - `grip_click`
  - `x/y/a/b`

This should be one-time or low-noise output, not per-frame spam.

### Step 2: Active profile confirmation

Keep runtime logging that shows:

- active profile per hand when it changes
- whether it ever becomes non-`none`

### Step 3: Live action proof

Once a non-`none` controller profile appears, verify:

- thumbstick changes produce non-zero axis values
- trigger/grip/button actions become active
- `InputXRGamepadSystem` emits the expected events

### Step 4: Only then wire default locomotion

After thumbsticks are proven alive:

- decide how AVC should consume them by default
- make that default explicit and overridable

---

## 7. Likely failure buckets

This task should distinguish between at least these possibilities:

- wrong Focus 3 binding paths
- wrong active interaction profile assumption
- runtime only activating a fallback profile we do not bind correctly
- action-set attach/sync issue
- controller runtime state never activating for this session even though hand tracking is alive
- higher-level event bridge issue after low-level actions are already correct

Right now the evidence still points at one of the first four buckets.

---

## 8. Desired follow-up implementation direction

If controller actions start working, the likely short-term engine direction is:

### A. Keep authored split

- `VRHand`: pose
- `InputVRGamepad`: sticks/buttons/triggers

### B. Add default XR locomotion policy

AVC should have a clear default policy for XR locomotion, likely:

- left stick: planar translation
- optional right stick: snap turn / smooth turn / secondary locomotion role

but this should be author-configurable.

### C. Preserve future coexistence with hand tracking

Hand tracking support for avatar fingers/hands should coexist with controller actions:

- hand tracking may drive avatar fingers/hands when available
- controller sticks/buttons/triggers should still work when controllers are active
- hand tracking being enabled should not implicitly kill controller locomotion/buttons

---

## 9. Completion criteria

This task is complete when all of the following are true:

- on Focus 3, an active controller interaction profile is observed when controllers are in use
- thumbstick actions become live and non-zero in engine logs
- `InputVRGamepad` emits usable thumbstick/button events into MMS
- a minimal MMS example can react to Focus 3 thumbstick input
- AVC has a clear default path for XR stick locomotion, or there is a tightly scoped follow-up task to add it immediately next

This task is **not** complete merely because hand tracking works or because `VRHand` moves.

If the current OpenXR controller-action bring-up remains blocked, the broader backend direction is
captured in:

- [docs/draft/shared-xr-backend-abstraction-openxr-openvr.md](../draft/shared-xr-backend-abstraction-openxr-openvr.md)
