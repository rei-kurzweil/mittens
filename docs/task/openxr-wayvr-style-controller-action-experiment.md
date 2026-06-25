# Task: OpenXR WayVR-Style Controller Action Experiment

Date: 2026-06-25

This task records the next narrow debugging step for XR controller input:

- imitate the relevant parts of WayVR's OpenXR controller action setup
- keep the experiment focused on controller action activation
- avoid introducing MMS-config-driven binding authoring in this phase

Related context:

- [docs/task/openxr-controller-actions-and-default-stick-locomotion.md](./openxr-controller-actions-and-default-stick-locomotion.md)
- [docs/task/xr-gamepad-and-hand-input-refactor.md](./xr-gamepad-and-hand-input-refactor.md)
- [src/engine/ecs/system/openxr_system.rs](../../src/engine/ecs/system/openxr_system.rs)
- [/tmp/wayvr/wayvr/src/backend/openxr/input.rs](/tmp/wayvr/wayvr/src/backend/openxr/input.rs)
- [/tmp/wayvr/wayvr/src/backend/openxr/openxr_actions.json5](/tmp/wayvr/wayvr/src/backend/openxr/openxr_actions.json5)

---

## 1. Current evidence

The current Cat Engine runtime evidence is:

- OpenXR session reaches `FOCUSED`
- controller action set is created and attached successfully
- suggested bindings are emitted for common controller profiles, including Focus 3 aliases
- controller interaction profiles remain `none`
- controller pose actions remain invalid
- trigger/grip/button/stick actions remain `is_active = false`
- `CTLXR` motion is still coming from hand-root fallback, not controller action poses

The important implication is:

- the current blocker is still controller action activation at the OpenXR layer
- the stick binding rewrite alone did not change that result

---

## 2. What WayVR actually does

The local WayVR inspection showed these relevant differences in approach:

### A. No subaction paths for controller actions

WayVR creates its OpenXR actions with empty subaction-path lists:

- button-like actions: `create_action::<bool>(..., &[])`
- analog-like actions: `create_action::<f32>(..., &[])`
- pose actions: `create_action::<xr::Posef>(..., &[])`
- vector-like actions: `create_action::<Vector2f>(..., &[])`

### B. Polling through `Path::NULL`

WayVR reads action state through `xr::Path::NULL` rather than polling distinct left/right subaction paths.

### C. Pose spaces created with `Path::NULL`

WayVR creates pose spaces with:

- `pose.create_space(&session, xr::Path::NULL, xr::Posef::IDENTITY)`

not one space per left/right subaction path.

### D. Config-driven profile bindings

WayVR uses `openxr_actions.json5` to define profile bindings.

That is **not** the part to copy yet.

For this phase, the useful insight is the action model:

- no subaction paths
- `Path::NULL` state polling
- `Path::NULL` pose-space creation

not the config file system itself.

---

## 3. Important non-finding from WayVR

WayVR did **not** reveal a special Vive Focus 3 controller profile solution.

Observed:

- no explicit `/interaction_profiles/htc/vive_focus3_controller` entry in the inspected OpenXR action config
- no obvious workaround for `current_interaction_profile == none`
- no generic A/B/X/Y gamepad surface matching Cat Engine's current `InputXRGamepad` shape

So this task is not based on:

- "WayVR has a magic Focus 3 path we are missing"

It is based on:

- "WayVR's OpenXR action model is structurally simpler than ours, and we should test whether that matters for this runtime"

---

## 4. Scope of this experiment

This experiment should change only the lower-level OpenXR controller action path.

It should cover:

- creating controller actions without subaction paths
- polling controller actions with `Path::NULL`
- creating controller pose spaces with `Path::NULL`
- preserving Cat Engine's existing higher-level `InputXRGamepad` and MMS event surface
- preserving authored locomotion behavior on the engine side

It should not cover:

- MMS-based binding authoring
- general XR input redesign
- renaming `CTLXR`
- hand-root fallback redesign
- full controller profile policy cleanup

---

## 5. Proposed implementation direction

### Step 1: Convert controller actions to WayVR-style action creation

Change `OpenXRSystem::try_init_controller_input(...)` so that:

- controller button actions use `&[]`
- controller analog actions use `&[]`
- controller pose actions use `&[]`

instead of left/right subaction paths.

### Step 2: Convert pose spaces to `Path::NULL`

Change controller aim/grip pose-space creation to:

- one aim space using `Path::NULL`
- one grip space using `Path::NULL`

or an equivalent WayVR-style structure

instead of per-hand subaction spaces.

### Step 3: Poll with `Path::NULL`

Change low-level controller state polling to use:

- `action.state(&session, openxr::Path::NULL)`

for:

- thumbstick / joystick axes
- trigger
- grip
- buttons
- select

### Step 4: Reconstruct engine hand-specific state if needed

If Cat Engine still wants a left/right gamepad shape, resolve that after polling.

This experiment accepts that:

- the engine's higher-level state shape may temporarily be less semantically pure than the current design

if that lets us prove whether the runtime activates controller actions under the WayVR-style model.

---

## 6. Success criteria

This experiment is a success if, on the target Focus 3 runtime:

- one or more controller actions become `is_active = true`
- thumbstick values become non-zero when moved
- controller pose actions become valid
- `InputXRGamepadSystem` begins receiving live stick state

Even partial success is useful:

- e.g. stick activity appears before buttons do
- or pose becomes valid before buttons do

because that would prove the current blocker is partly about action shape / polling style.

---

## 7. Failure interpretation

If this experiment still produces:

- active profile = `none`
- all controller actions inactive
- no valid controller pose actions

then the likely conclusion becomes stronger:

- the runtime is not exposing controller actions to this app/session in the normal OpenXR path we are using

At that point, further binding-path churn should be deprioritized in favor of:

- runtime/session behavior investigation
- comparing app manifest/session setup with known-working apps
- determining whether working apps are using a different backend path than the one we assume

---

## 8. Deliverable for this task

The code outcome for this task should be:

- a WayVR-style controller action experiment in `OpenXRSystem`
- runtime logs that clearly distinguish the post-change results
- no premature config/MMS binding system added yet

The primary question this task should answer is:

- does adopting WayVR's no-subaction-path + `Path::NULL` action model make Focus 3 controller actions actually activate in Cat Engine?
