# Task: OpenXR runtime/session comparison with WayVR

Date: 2026-06-25

Status: follow-up investigation after the WayVR-style controller action experiment failed to activate controller actions.

This task captures the next comparison step:

- compare Cat Engine's OpenXR app/session setup against the local WayVR checkout
- identify differences that could explain why controller actions never become active
- focus on runtime/session behavior rather than more action-binding churn

Related context:

- [docs/task/openxr-wayvr-style-controller-action-experiment.md](./openxr-wayvr-style-controller-action-experiment.md)
- [docs/task/openxr-controller-actions-and-default-stick-locomotion.md](./openxr-controller-actions-and-default-stick-locomotion.md)
- [src/engine/ecs/system/openxr_system.rs](../../src/engine/ecs/system/openxr_system.rs)
- [/tmp/wayvr/wayvr/src/backend/openxr/mod.rs](/tmp/wayvr/wayvr/src/backend/openxr/mod.rs)
- [/tmp/wayvr/wayvr/src/backend/openxr/helpers.rs](/tmp/wayvr/wayvr/src/backend/openxr/helpers.rs)
- [/tmp/wayvr/wayvr/src/backend/openxr/input.rs](/tmp/wayvr/wayvr/src/backend/openxr/input.rs)

---

## 1. Why this task exists

The completed WayVR-style action experiment changed Cat Engine to:

- create controller actions with `&[]`
- create controller pose spaces with `Path::NULL`
- poll controller action state through `Path::NULL`

That experiment did **not** change the runtime result:

- active interaction profiles still stayed `none`
- controller actions still stayed inactive
- controller action poses still stayed invalid

So the remaining investigation should move outward from action shape and toward:

- instance creation differences
- enabled extension differences
- session creation differences
- reference-space differences
- any runtime-policy difference between Cat Engine and a known-working app

---

## 2. Main comparison findings so far

### A. Cat Engine uses a normal Vulkan session; WayVR uses an overlay session

Cat Engine creates its OpenXR session through:

- `instance.create_session::<openxr::Vulkan>(...)`

WayVR creates its OpenXR session through:

- `EXTX_overlay`
- a custom `create_overlay_session(...)` path

This is a major behavioral difference.

It may matter because:

- overlay sessions can have different runtime policy or visibility behavior
- the runtime may treat controller input routing differently for overlay-style apps versus primary scene apps

It may also *not* matter, because:

- controller input should still normally be available to ordinary focused sessions

So this is an important difference, but not yet proof.

### B. Cat Engine uses `LOCAL` reference space; WayVR uses `STAGE` and `VIEW`

Cat Engine currently creates:

- `ReferenceSpaceType::LOCAL`

WayVR creates:

- `ReferenceSpaceType::STAGE`
- `ReferenceSpaceType::VIEW`

This difference is likely important for:

- pose interpretation
- rig grounding
- playspace-relative behavior

It is less likely to explain:

- `current_interaction_profile == none`
- all controller actions staying inactive

So this is probably not the main blocker, but it is still a meaningful runtime difference.

### C. WayVR requires and enables a different extension set

Cat Engine currently enables a relatively small set:

- `khr_vulkan_enable`
- `ext_hand_tracking` when available
- `ext_hand_interaction` when available
- `htc_vive_focus3_controller_interaction` when available
- `htc_hand_interaction` when available

WayVR requires/enables a different set centered around its overlay model:

- `khr_vulkan_enable2`
- `extx_overlay`
- optional `khr_binding_modification`
- optional `ext_dpad_binding`
- optional `ext_hand_interaction`
- optional eye-gaze and composition-layer related extensions

Important implications:

- Cat Engine is still on the legacy Vulkan enable path
- WayVR is using `khr_vulkan_enable2`
- WayVR does not appear to depend on the HTC Focus 3 interaction extension the way Cat Engine expects to

This makes extension-set differences one of the strongest remaining comparison areas.

### D. WayVR's input model is pointer-oriented, not gamepad-oriented

WayVR's `OpenXrInputSource` is built around:

- left pointer source
- right pointer source
- fallback "handsfree" pointer source

with actions like:

- pose
- click
- grab
- alt-click
- scroll
- haptics

Cat Engine's experiment is built around:

- aim/grip pose
- select
- sticks
- triggers
- grip
- face buttons

This means WayVR is not a direct proof that:

- Focus 3 stick/button/gamepad-style actions must work under the same conditions

It only proves that:

- WayVR is a useful reference for a simpler action architecture

### E. Session state behavior still differs in practical effect

Cat Engine logs show:

- session reaches `FOCUSED`
- controller action set attaches
- action sync runs
- but interaction profiles remain `none`

So the key unresolved question is:

- what else about the app/session/runtime contract causes the runtime to withhold controller-action activation?

---

## 3. Most plausible remaining hypotheses

After the action-shape experiment, the strongest remaining hypotheses are:

### Hypothesis 1: Vulkan/session creation path matters

Cat Engine uses:

- legacy `khr_vulkan_enable`
- normal `create_session::<Vulkan>(...)`

WayVR uses:

- `khr_vulkan_enable2`
- overlay-session creation via `EXTX_overlay`

This may mean:

- the runtime behaves differently for action activation under those app/session types

### Hypothesis 2: Cat Engine's extension mix is still not the one the runtime expects

Even though Cat Engine enables HTC-related interaction extensions when available, the runtime may still expect:

- a different interaction extension set
- a different app/session role
- a different binding/profile activation path

### Hypothesis 3: WayVR is not actually exercising the same controller surface

WayVR is useful as an action-model reference, but it may not be testing the same:

- thumbstick
- trigger
- grip
- face-button
- profile-activation

surface that Cat Engine needs for `InputXRGamepad`.

So "WayVR works" may not translate to:

- "Focus 3 gamepad-style controller actions should activate in Cat Engine under the same assumptions"

### Hypothesis 4: The runtime simply is not exposing controller actions to this app/session

The current evidence still fits:

- hand tracking is available
- controller actions are not
- focused session alone is not enough

If so, the remaining work is not about API shape. It is about discovering the runtime policy boundary.

---

## 4. Recommended next debugging steps

### Step 1: Log the full instance/session setup surface

Cat Engine should emit one-time logs for:

- application name / engine name / API version
- enabled extensions
- whether `khr_vulkan_enable2` is available but unused
- session creation path in use
- reference space type in use

Some of this is already logged, but the comparison should be made more explicit.

### Step 2: Compare against a minimal focused session variant

Create a narrow experiment that changes only one of these at a time:

- switch `LOCAL` to `STAGE`
- test `khr_vulkan_enable2` if feasible
- test whether any session-creation policy changes are possible without adopting the full WayVR overlay model

This should stay one-variable-at-a-time.

### Step 3: Separate pointer-style proof from gamepad-style proof

We should avoid treating WayVR as stronger evidence than it is.

Future comparison should ask:

- is there a known-working app on this runtime that clearly receives stick/button controller actions, not just pointer-like pose/click input?

### Step 4: Deprioritize more small binding rewrites

The recent evidence is strong enough that:

- more binding-path churn
- more subaction-path permutations
- more small polling-shape changes

should no longer be the default next step.

---

## 5. Conclusion

The WayVR comparison is still useful, but the main value has changed.

It no longer looks like:

- "copy WayVR's action shape and the runtime will wake up"

It now looks like:

- "WayVR helps identify the remaining app/session/runtime differences that might matter more than action shape"

The next investigation should therefore focus on:

- session type
- extension set
- Vulkan enable path
- reference-space policy
- whether the runtime exposes controller actions differently for different app classes
