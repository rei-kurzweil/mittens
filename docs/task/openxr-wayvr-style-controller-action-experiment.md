# Task: OpenXR WayVR-style controller action experiment

Date: 2026-06-25

This task captures the next narrow OpenXR debugging step for XR controller input:

- copy the relevant structural parts of WayVR's controller action setup
- keep the experiment focused on low-level action activation
- avoid pulling in WayVR's config-driven binding authoring in this phase

Status: implemented and tested on 2026-06-25. Result: negative.

Related context:

- [docs/task/openxr-controller-actions-and-default-stick-locomotion.md](./openxr-controller-actions-and-default-stick-locomotion.md)
- [docs/task/xr-gamepad-and-hand-input-refactor.md](./xr-gamepad-and-hand-input-refactor.md)
- [src/engine/ecs/system/openxr_system.rs](../../src/engine/ecs/system/openxr_system.rs)
- [/tmp/wayvr/wayvr/src/backend/openxr/input.rs](/tmp/wayvr/wayvr/src/backend/openxr/input.rs)
- [/tmp/wayvr/wayvr/src/backend/openxr/openxr_actions.json5](/tmp/wayvr/wayvr/src/backend/openxr/openxr_actions.json5)

---

## 1. Why this task exists

Current Cat Engine evidence still points at the same low-level blocker:

- the OpenXR session reaches `FOCUSED`
- the controller action set is created and attached
- suggested bindings are emitted for several controller profiles
- active controller interaction profiles remain `none`
- controller pose actions remain invalid
- trigger, grip, button, and stick actions remain `is_active = false`
- `CTLXR` motion still comes from hand-root fallback rather than controller action poses

The important implication is:

- the controller action path is still dead at the OpenXR layer
- the recent binding-path cleanup did not change that

So the next step should not be a broad XR input redesign. It should be a very small
structural experiment against the OpenXR action model itself.

---

## 2. Hypothesis from WayVR

WayVR's OpenXR controller setup is structurally simpler than Cat Engine's current one.

The useful differences are:

- controller actions are created without left/right subaction paths
- action state is polled through `xr::Path::NULL`
- controller pose spaces are also created with `xr::Path::NULL`

The hypothesis for this task is:

- the Focus 3 runtime may be failing to activate Cat Engine's per-hand subaction-based controller action model
- a WayVR-style no-subaction-path + `Path::NULL` model may allow the runtime to expose controller actions normally

This is only a runtime activation experiment. It is not yet a statement that the engine should
permanently adopt the same architecture.

---

## 3. What to copy from WayVR

Local WayVR inspection suggests copying only these parts:

### A. Action creation without subaction paths

WayVR creates controller actions with empty subaction-path lists:

- button actions: `create_action::<bool>(..., &[])`
- scalar analog actions: `create_action::<f32>(..., &[])`
- pose actions: `create_action::<xr::Posef>(..., &[])`
- vector actions: `create_action::<Vector2f>(..., &[])`

### B. State polling through `Path::NULL`

WayVR reads action state with:

- `action.state(&session, xr::Path::NULL)`

rather than polling separate left/right subaction paths.

### C. Pose spaces created through `Path::NULL`

WayVR creates pose spaces with:

- `pose.create_space(&session, xr::Path::NULL, xr::Posef::IDENTITY)`

rather than creating one pose space per hand.

---

## 4. What not to copy yet

WayVR also has a config-driven binding layer via `openxr_actions.json5`.

That is not the experiment here.

This task should not introduce:

- MMS-driven binding authoring
- a new config file format
- a general XR input architecture rewrite
- a broad `InputXRGamepad` redesign

The only thing this task wants to learn is whether the simpler action model changes runtime
activation behavior.

---

## 5. Important non-findings from WayVR

WayVR did not reveal a known-good Focus 3-specific profile trick.

Observed during inspection:

- no explicit `/interaction_profiles/htc/vive_focus3_controller` entry in the inspected action config
- no obvious workaround for `current_interaction_profile == none`
- no generic gamepad surface that directly matches Cat Engine's current `InputXRGamepad` model

So this task is not based on:

- "WayVR has a special Focus 3 binding we are missing"

It is based on:

- "WayVR uses a simpler OpenXR action model, and that model is worth testing directly on this runtime"

---

## 6. Scope of the experiment

This experiment should only touch the low-level OpenXR controller action path.

It should cover:

- creating controller actions without subaction paths
- polling controller actions with `Path::NULL`
- creating controller aim/grip pose spaces with `Path::NULL`
- keeping existing higher-level `InputXRGamepad` and MMS event surfaces alive as much as possible
- keeping authored locomotion behavior outside the scope of this change

It should not cover:

- locomotion-policy redesign
- `CTLXR` renaming
- hand-root fallback redesign
- hand-tracking quality work
- complete interaction-profile policy cleanup

---

## 7. Proposed implementation shape

### Step 1: Convert controller action creation

Change `OpenXRSystem::try_init_controller_input(...)` so controller actions use `&[]`
instead of left/right subaction paths.

This applies to:

- button actions
- scalar analog actions
- thumbstick/vector actions
- pose actions

### Step 2: Convert controller pose-space creation

Create controller aim/grip spaces with `openxr::Path::NULL` instead of per-hand subaction paths.

If the current internal data shape assumes one space per hand, it is acceptable for this
experiment to introduce a temporary adapter layer.

### Step 3: Convert low-level polling

Poll controller state with:

- `action.state(&session, openxr::Path::NULL)`

for:

- thumbsticks
- trigger
- grip
- select
- face buttons

### Step 4: Reconstruct engine-facing state if needed

If Cat Engine still needs left/right-shaped state at the engine boundary, rebuild that after
the low-level OpenXR read.

This experiment accepts some temporary awkwardness in the internal shape if that is what lets
us determine whether action activation starts working.

---

## 8. Success criteria

This experiment is successful if, on the target Focus 3 runtime, any of the following begin
working:

- one or more controller actions report `is_active = true`
- thumbstick values become non-zero while moving
- controller pose actions become valid
- `InputXRGamepadSystem` begins receiving live controller state

Partial success still matters:

- stick activity without buttons
- buttons without valid poses
- valid poses before a non-`none` profile is reported

Any of those would show that the action model itself is affecting runtime behavior.

---

## 9. What was implemented

The experiment was implemented in `OpenXRSystem` with the following changes:

- `ControllerInput` was reshaped to use one shared `aim_space` and one shared `grip_space`
- controller actions that previously used left/right subaction paths were changed to `create_action(..., &[])`
- controller pose spaces were changed to `create_space(..., openxr::Path::NULL, ...)`
- low-level controller state polling was changed to `action.state(&session, openxr::Path::NULL)`
- shared controller action poses were mirrored back into the existing left/right engine-facing pose cache so the rest of the engine could continue running unchanged

This was intentionally an experiment, not a final architecture decision.

---

## 10. Runtime result

On the target runtime, the post-change logs still showed:

- active interaction profile = `none` for both `/user/hand/left` and `/user/hand/right`
- `select`, thumbstick, trigger, grip, and face-button actions present but still `is_active = false`
- controller aim/grip poses still invalid
- `CTLXR` motion still coming from hand-root fallback rather than controller action poses
- `InputXRGamepadSystem` active, but with no live controller action state for authored UI/gamepad behavior

So the WayVR-style action model did **not** make Focus 3 controller actions activate in Cat Engine.

---

## 11. Failure interpretation

If this experiment still produces:

- active profile = `none`
- inactive controller actions across the board
- invalid controller pose actions

then the likely conclusion becomes stronger:

- this runtime is not exposing controller actions to Cat Engine through the normal OpenXR action path we are using

At that point, more binding-path churn should be deprioritized in favor of:

- runtime/session behavior investigation
- app-manifest and session-setup comparison with known-working apps
- checking whether working apps are using a different backend path than the one we assume

---

## 12. Conclusion

This experiment ruled out one specific hypothesis:

- the main blocker is probably **not** Cat Engine's use of per-hand subaction paths for controller actions

What remains plausible is:

- the runtime is not exposing controller actions to this app/session at all
- there is a session/app-manifest/runtime-policy difference versus known-working apps
- the relevant working path on Focus 3 may differ from the normal OpenXR controller-action path we assumed

This result means future work should focus more on runtime/session investigation than on further
small action-binding shape rewrites.

---

## 13. Deliverable

The code outcome for this task should be:

- a WayVR-style controller action experiment inside `OpenXRSystem`
- runtime logs that clearly show whether behavior changed after the rewrite
- no premature config/MMS binding system added as part of this work

The primary question this task must answer is:

- does a no-subaction-path + `Path::NULL` controller action model make Focus 3 controller actions activate in Cat Engine?

Answer from the implemented experiment:

- no
