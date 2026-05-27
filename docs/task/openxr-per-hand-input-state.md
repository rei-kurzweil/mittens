# Task: OpenXR per-hand input state

Date: 2026-05-27

This task is about adding a runtime-owned OpenXR controller input state that is equivalent in role to the current winit-backed `InputState`, but scoped to XR hands/controllers rather than desktop keyboard/mouse.

This is intentionally **not** a gesture task.

The immediate goal is lower-level:

- track per-hand/controller input transitions such as `down`, `pressed`, and `released`
- keep that state owned by `OpenXRSystem`
- verify first, with print statements, that the runtime can reliably toggle trigger/select state per hand/controller

This is a docs/task note only. No `src/` changes are proposed here yet.

Related code:

- `src/engine/user_input.rs`
- `src/engine/ecs/system/openxr_system.rs`
- `src/engine/ecs/system/raycast_system.rs`
- `src/engine/ecs/system/gesture_system.rs`
- `docs/spec/vr-input.md`

---

## 1. Problem statement

The engine already has a clear desktop input layer:

- `InputState` stores low-level button/key state
- systems consume that state to derive behavior
- gesture semantics sit above that layer

For XR, the spatial side exists already:

- `OpenXRSystem` samples HMD/controller poses
- `ControllerXRComponent` drives transforms from those poses
- pointer/raycast code can already use controller-driven transforms as ray origins

What is missing is the non-spatial controller input layer.

Right now `OpenXRSystem` creates and samples pose actions, but it does not expose per-hand button/trigger state analogous to:

- `mouse_down`
- `mouse_pressed`
- `mouse_released`

Because of that, XR interaction work risks jumping straight from raw OpenXR actions into gesture/raycast logic without a reusable intermediate state model.

That would be the wrong boundary.

The missing abstraction is:

- a small OpenXR-owned input snapshot for left/right controllers
- with persistent `down` state and per-frame edges
- separate from drag/click/gesture interpretation

---

## 2. Scope for this task

This task should establish the per-hand/controller input state layer only.

It should cover:

- which OpenXR actions to create for controller button/trigger input
- how to sample them per hand
- how to store current-frame and edge-triggered state
- where in `OpenXRSystem` that state should live and refresh each frame
- how to verify the runtime actually toggles those states on the target hardware/runtime

It should not cover:

- drag/click/gesture generation
- raycast behavior changes
- changing `GestureSystem` in this task
- hand-tracking-as-gesture inference
- authored MMS surface changes

---

## 3. Desired architecture boundary

The intended layering should be:

```text
OpenXR runtime actions
  -> OpenXRSystem per-hand input state
  -> higher-level systems consume that state
  -> gesture/raycast semantics happen elsewhere
```

That mirrors the desktop path:

```text
winit events
  -> UserInput / InputState
  -> higher-level systems consume that state
  -> gesture semantics happen elsewhere
```

Important constraint:

- the XR input state should be the equivalent of desktop input state, not the equivalent of desktop gesture state

So the XR layer should answer questions like:

- is left trigger currently down?
- did right trigger become pressed this frame?
- did left select release this frame?
- what is the current analog value for the trigger, if available?

It should not answer questions like:

- did a drag start?
- what renderable is clicked?
- should a gizmo capture input?

---

## 4. Initial state surface to target

The first version should stay narrow.

At minimum, we need one per-hand trigger/select input that can drive future pointer work.

Recommended initial surface:

```rust
enum XrHand {
    Left,
    Right,
}

struct XrButtonState {
    down: bool,
    pressed: bool,
    released: bool,
    value: f32,
}

struct XrHandInputState {
    trigger: XrButtonState,
}

struct OpenXrInputState {
    left: XrHandInputState,
    right: XrHandInputState,
}
```

Notes:

- `value` is included because many runtimes expose trigger/select as a float axis even when the engine wants boolean edges
- `down` should be derived from the sampled value/state using one clear threshold policy
- `pressed` and `released` should be per-frame edges, just like desktop input
- the first pass should resist adding thumbsticks, menu buttons, squeeze, etc. until trigger/select sampling is proven stable

If the runtime exposes a clean boolean select action and a float trigger action, we can still begin with a single logical `trigger` field and refine later.

---

## 5. Verification-first requirement

Before designing the final state shape in `src/`, verify that the target runtime/hardware actually reports usable per-hand input transitions.

This verification should happen with print statements first.

The purpose of this phase is not elegance. The purpose is to answer these questions with evidence:

1. Can we bind a per-hand trigger/select input action on the current OpenXR runtime?
2. Do left and right hands report independently?
3. Do we get stable transitions frame-to-frame?
4. Is the signal effectively boolean, analog, or both?
5. Does the runtime report the same control consistently across the controller profiles we care about?

### Verification strategy

Use the existing XR example path, preferably:

- `cargo run --release --example vr-input`

or, if the avatar scene is the more realistic target:

- `cargo run --release --example bisket-vr-demo`

Add temporary print statements in `OpenXRSystem` after action sync/sample, with output shaped roughly like:

```text
[OpenXR][input] left trigger value=0.00 down=false pressed=false released=false
[OpenXR][input] right trigger value=1.00 down=true pressed=true released=false
```

Prefer printing only on changes or edges once the first raw signal is confirmed, so logs stay usable.

### What must be proven before moving past prints

We should be able to observe:

- left trigger press/release without right changing
- right trigger press/release without left changing
- repeated press/release cycles across frames
- no obvious frame-sticky bug where `pressed` stays true for multiple frames
- no obvious runtime mix-up where both hands mirror each other unexpectedly

If this cannot be verified on the target runtime, then the follow-up implementation task needs to stop and document what the runtime actually exposes.

---

## 6. Where this state should live

The state should live inside `OpenXRSystem`, near the existing controller input/session-owned OpenXR action objects.

Rationale:

- `OpenXRSystem` already owns action creation and action sync
- it is the only subsystem that should know the OpenXR runtime details directly
- higher-level systems should consume an engine-level state struct, not raw OpenXR action handles

Important boundary:

- keep raw action sampling in `OpenXRSystem`
- expose a small engine-owned state snapshot above it
- do not let higher layers depend directly on OpenXR action names or bindings

---

## 7. Likely implementation direction after verification

After print-based verification succeeds, the follow-up implementation should likely proceed in this order:

1. add OpenXR action(s) for trigger/select input per hand
2. sample them during the existing OpenXR action sync path
3. store previous/current state per hand
4. derive `down` / `pressed` / `released`
5. make that state queryable from `OpenXRSystem` by higher-level systems
6. only then design how pointer/raycast/gesture code consumes it

The key sequencing rule is:

- do not bundle gesture behavior into the same first pass as XR input-state acquisition

That separation keeps debugging tractable.

---

## 8. Non-goals for this task

This task should not try to solve any of the following:

- XR click/drag semantics
- controller-driven `GestureSystem` refactors
- `PointerComponent` behavior changes
- `RayCastSystem` trigger policy changes
- generalized hand-tracking pinch/grab input
- an MMS-exposed XR input authoring API

Those may become follow-up tasks once the lower-level input state is real and verified.

---

## 9. Acceptance criteria

This task is complete when:

- there is a confirmed design for a small per-hand/controller XR input state owned by `OpenXRSystem`
- the first tracked control is intentionally narrow, preferably trigger/select only
- a print-first verification pass is defined and executed before broader refactors
- the verification shows independent left/right toggling on the target runtime/hardware
- the task remains explicitly separate from gesture/raycast behavior changes

This task is not complete merely because OpenXR actions were added.

The proof requirement is:

- we can observe stable per-hand state transitions and trust them enough to build later gesture integration on top

---

## 10. Follow-up tasks after this one

Once this task is done, likely follow-ups are:

1. consume XR per-hand input state from pointer/raycast trigger policy
2. generalize gesture input so it is not mouse-only
3. decide whether XR pointer inputs should be continuous, event-driven, or mixed
4. decide whether additional XR controls beyond trigger/select belong in the shared input state