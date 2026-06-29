# Task: XR Gamepad And Hand Input Refactor

Date: 2026-06-25

This task tracks the current XR input debugging work around:

- `InputXRGamepad`
- `CTLXR`
- OpenXR controller action polling
- OpenXR hand-root pose fallback

It also records the likely refactor direction if the current split between "hand pose driving transforms" and "controller/gamepad actions driving buttons/sticks/locomotion" remains the intended model.

Related code:

- `src/engine/ecs/system/openxr_system.rs`
- `src/engine/ecs/system/input_xr_gamepad_system.rs`
- `src/engine/ecs/component/controller_xr.rs`
- `src/engine/ecs/component/input_xr_gamepad.rs`
- `examples/input-xr-gamepad.mms`
- `examples/input-xr-gamepad.rs`

---

## 1. Current state

The engine now has two authored XR input surfaces with different responsibilities:

- `CTLXR`
  - tracked transform driver for XR hand/controller pose
  - used by authored XR rigs to place controller-space content and pointers
- `InputXRGamepad`
  - non-pose XR controller input surface
  - used for button/axis events and thumbstick locomotion

The current bug investigation started because:

- `examples/input-xr-gamepad.mms` renders correctly
- but button/stick UI was not responding

The example itself was not the root problem.

---

## 2. What has been verified

The following has already been verified in code:

- `InputXRGamepadSystem` is alive and ticking
- `OpenXRSystem` exposes an `xr_gamepad_state`
- XR gamepad signals are bridged into MMS handlers
- `XrAxisChanged` payloads reach MMS as structured arrays
- the example scene parses and the example binary builds

Extra logging was added during debugging to verify:

- when XR gamepad state becomes active
- emitted XR button/axis events in `InputXRGamepadSystem`
- active interaction profile per hand
- raw `is_active/current_state` for controller actions
- validity of aim/grip controller pose spaces

---

## 3. Current runtime evidence

On the target device/runtime under test:

- headset: HTC Vive Focus 3
- user reports that controllers work in other apps
- user reports that Cat Engine appears to get controller-like motion in-scene

But the current OpenXR action/path debug output shows:

- `/user/hand/left`: no active interaction profile
- `/user/hand/right`: no active interaction profile
- `thumbstick`, `select`, `trigger`, `grip`, `A/B/X/Y` actions all exist but report `is_active = false`
- controller aim/grip pose spaces also report invalid in the debug path

That means the action-based controller input path is not currently alive for this session, even when the scene is focused.

Additional hand-tracking runtime evidence gathered during this task:

- hand-root fallback can drive `CTLXR` even while controller actions remain dead
- the reduced hand-root source currently resolves to `WRIST` when available
- once both hands are visible, hand tracking can remain continuously present rather than dropping out every frame
- thumb-tip/index-tip pinch distance is valid enough to derive a simple engine-side pinch state
- pinch transitions appear believable from runtime data alone
- the main remaining hand-tracking problem is now subjective pose quality, not hand-data absence

Most importantly:

- the left/right hands can both stay `root=Some(WRIST)` for extended periods
- but the user still observes wrist orientation pitching/yawing back and forth while rotating a hand slowly

So the next debugging target is not "is hand tracking alive?" but:

- whether `WRIST` orientation itself is unstable on this runtime, or
- whether `WRIST` is simply the wrong semantic pose to drive authored hand transforms

---

## 4. Important architecture finding

`CTLXR` is not purely "controller action pose".

In `OpenXRSystem::preferred_pose(...)`, pose precedence is currently:

1. `hand_root_pose_cache`
2. `controller_pose_cache`
3. none

So a `CTLXR`-driven transform may still move from hand-root tracking even if controller action state is completely unavailable.

This is important because it means:

- visible motion of a `CTLXR` subtree does not prove controller action input is alive
- it may only prove that hand-root pose tracking is alive

That is the main conceptual mismatch in the current naming and debug expectations.

---

## 5. Naming problem

Given the current responsibilities, `CTLXR` is no longer a good name.

What it actually represents is closer to:

- tracked XR hand/controller pose input
- authored transform-driving XR hand source
- not "controller gamepad input"

So a future rename should be considered:

- `CTLXR` -> `InputXrHand`

Possible meanings:

- "pose-only tracked XR hand/controller input surface"
- sibling to `InputXRGamepad`

This would make the authored distinction much clearer:

- `InputXrHand`
  - pose / pointer / hand-space transforms
- `InputXRGamepad`
  - buttons / triggers / grips / sticks / locomotion

This matches the desired conceptual split better than the current `ControllerXRComponent` / `CTLXR` terminology.

---

## 6. Refactor direction

If the engine keeps both surfaces, the cleaner long-term design is:

### A. Pose path

- `CTLXR` renamed to `InputXrHand`
- explicitly documented as pose-only
- responsible for:
  - aim/grip/hand-root pose selection
  - driving authored transforms
  - feeding pointer origin transforms

### B. Gamepad path

- `InputXRGamepad` remains the button/axis surface
- explicitly documented as non-pose controller input
- responsible for:
  - thumbstick axes
  - trigger/grip analog and thresholded button events
  - face buttons
  - locomotion

### C. OpenXR runtime ownership

`OpenXRSystem` should likely be refactored internally into clearer logical layers even if it remains one type at first:

- hand/root pose sampling
- controller pose action sampling
- controller gamepad/button action sampling
- interaction profile / runtime diagnostics

The current `OpenXRSystem` mixes all of these in one place.

That is workable while debugging, but it makes it harder to answer:

- which runtime path is currently alive?
- which authored input surface depends on which OpenXR signal source?
- whether hand tracking is masking controller action failure

---

## 7. Potential redundancy to remove

The current implementation is at risk of redundant or overlapping concepts:

- `ControllerXRComponent`
- `CTLXR`
- hand-root pose fallback inside controller-facing logic
- `InputXRGamepad`
- older `InputXR` naming that does not distinguish pose-vs-buttons strongly enough

If we refactor this area, the goal should be to reduce overlap rather than add more aliases.

One likely target structure is:

- `InputXR`
  - rig/session ownership marker
- `InputXrHand`
  - pose surface
- `InputXRGamepad`
  - button/axis surface

With old names removed once migration is complete.

---

## 8. Immediate next debugging steps

Before renaming, finish proving which runtime path is alive on the Focus 3 session:

1. Log whether `CTLXR` transforms are currently sourced from:
   - `hand_root`
   - `controller_action`
   - `none`

2. Log whether hand tracking is active and whether left/right hand roots are present when controller actions are dead.

3. Compare a session where the controllers visibly move in-scene against the action debug output.

4. Only after that, decide whether the runtime issue is:
   - hand-tracking override/preference
   - controller activation/profile issue
   - incorrect action/profile setup
   - or a bug in our pose-vs-action branching

Updated follow-up after the above was verified:

5. Compare `WRIST` orientation against `PALM` orientation for each tracked hand.

6. Log large per-frame wrist rotation spikes rather than every small pinch-distance change.

7. Use those traces to decide whether the current jitter is:
   - raw runtime `WRIST` orientation noise
   - a bad choice of hand-root joint for authored pose driving
   - or an engine-side transform/application issue after sampling

---

## 9. Immediate code changes already made during investigation

The debugging work so far has already added:

- XR MMS event payload bridging
- `Transform.set_position(...)` for stick-dot UI movement
- `examples/input-xr-gamepad.mms`
- `examples/input-xr-gamepad.rs`
- Focus 3 profile binding suggestions in `OpenXRSystem`
- runtime debug logging for:
  - interaction profiles
  - action activity
  - controller pose validity
  - XR gamepad event emission

These changes are useful for triage now, but some of the logging should be removed or reduced once the runtime behavior is understood.

---

## 10. Decision to preserve

Even if implementation details change, one design decision now seems correct and should be preserved:

- authored XR gamepad/buttons/sticks should stay separate from authored XR pose tracking

That means the split introduced by `InputXRGamepad` is directionally right.

What likely needs adjustment is:

- naming
- clearer authored semantics
- clearer OpenXR runtime separation behind those authoring surfaces
