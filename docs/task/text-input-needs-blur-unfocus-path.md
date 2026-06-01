# TextInput needs a real blur / unfocus path

## Why

`TextInput` has focus and clear-focus machinery, but there is still no robust
way to blur an input when the user clicks away into empty space.

This is not exactly a bug in the current implementation. It is a missing
interaction contract.

The current behavior depends on receiving some other click-driven signal that
the text input system can interpret as "focus moved elsewhere". That works only
when another hit target actually produces a click event.

The gap is the empty-space case.

## 1. Current behavior

In
[src/engine/ecs/system/text_input_system.rs](../../src/engine/ecs/system/text_input_system.rs),
`TextInputSystem` listens to `SignalKind::Click` and:

- focuses the nearest `TextInput` when the clicked renderable belongs to one
- otherwise emits `TextInputClearFocus`

That means blur currently depends on a `Click` event existing at all.

But `Click` is only produced when the gesture pipeline has a click-capable hit
target. There is no obvious global "pointer pressed and nothing relevant was
hit" event in the current signal model.

So if the user clicks empty space with no click-capable renderable under the
pointer, there may be no signal for `TextInputSystem` to observe, and focus can
remain stuck.

## 2. Missing contract

The missing piece is not `TextInputClearFocus` itself. That intent already
exists.

The missing piece is a signal or hook that means something like:

- primary pointer press with no accepted click target
- primary click landed outside the focused text input
- pointer interaction should blur the current focused text input

Until that exists, blur-on-click-away is underspecified.

## 3. Likely event seam

The most likely place to introduce this is the input / gesture side of the
pipeline, not inside `TextInputSystem` itself.

Primary candidates:

- [src/engine/ecs/system/gesture_system.rs](../../src/engine/ecs/system/gesture_system.rs)
- [src/engine/ecs/rx/signal.rs](../../src/engine/ecs/rx/signal.rs)
- possibly the input bridge in [src/engine/user_input.rs](../../src/engine/user_input.rs)

The important ownership rule is:

- `TextInputSystem` should decide what to do when blur intent is observed
- gesture/input systems should own the observation that no click target was hit

## 4. Likely implementation options

### Option A: add an explicit miss event

Introduce a new event signal emitted when the primary pointer press/release
sequence qualifies as a click but no click-capable target was hit.

Conceptual examples:

- `ClickMiss`
- `PointerPrimaryClickMiss`
- `BackgroundClick`

This is the cleanest contract if multiple systems may later care about empty-
space clicks.

### Option B: add a more general pointer-down event

Introduce a more general global pointer press event and let consumers decide
whether that means blur.

This is broader, but it also risks adding a lower-level event that many systems
have to filter manually.

### Option C: special-case blur in input polling

Read raw input state from `TextInputSystem` and attempt to infer "click away"
there.

This is the least attractive option because it cuts across the existing
signal-driven interaction model and duplicates gesture ownership.

## 5. Recommended direction

Prefer an explicit miss-style event from the gesture layer.

Why:

- it fits the existing event-driven architecture
- it keeps empty-space click observation in the same place that already decides
  whether a pointer interaction counts as a click
- `TextInputSystem` can stay simple: focus on input click, blur on click miss

This is likely the most coherent extension of the current design.

## 6. Likely files involved

Event definition:

- [src/engine/ecs/rx/signal.rs](../../src/engine/ecs/rx/signal.rs)

Event production:

- [src/engine/ecs/system/gesture_system.rs](../../src/engine/ecs/system/gesture_system.rs)

Event consumption:

- [src/engine/ecs/system/text_input_system.rs](../../src/engine/ecs/system/text_input_system.rs)

Potential system ordering review:

- [src/engine/ecs/system/system_world.rs](../../src/engine/ecs/system/system_world.rs)

## 7. Acceptance criteria

1. Clicking empty space can blur a focused `TextInput`.
2. Blur behavior is driven by an explicit event/signal contract, not ad hoc raw
   input polling inside `TextInputSystem`.
3. Clicking another `TextInput` still transfers focus correctly.
4. Clicking visible UI that should keep focus semantics can do so explicitly.

## 8. Verification

Add focused tests covering:

- focused text input loses focus on empty-space click/miss event
- focused text input keeps focus on unrelated non-blur interactions if that is
  the intended policy
- focus transfer between two text inputs still works

Run:

- targeted `TextInputSystem` / gesture tests
- `cargo check --lib`

## 9. Non-goals

- keyboard Escape-to-blur in this task
- tab-navigation between text inputs
- whitespace caret placement

This task is specifically about giving the engine a real click-away blur path.