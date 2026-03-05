# Signal Emitter Component (ActionComponent) — evaluation

Date: 2026-03-04

This document evaluates the current “keyframes emit effectful signals at tempo-relative times” use case, and what it implies for:

- where **transport** (beat/bpm) lives
- whether **handlers** need transport access
- whether `ActionComponent` should evolve into a more explicit “signal emitter component”

## TL;DR

- The current animation pipeline already works *without handler transport access* as long as signals that need timing context carry explicit beat data.
- Today, `AnimationSystem` stamps `beat_context` into tempo-relative scheduling signals before emitting them.
- The main remaining transport leak is **non-`beat_context` scheduling inside `ActionSystem`** (e.g. audio filter changes currently schedule at `beat_now = 0.0`).
- If the executor is the only place allowed to read transport, either:
  - make all transport-relative actions *data-complete* (carry `beat_context` / absolute `beat`) before handlers run, or
  - move the transport-sensitive subset of `ActionSystem` out of handlers and into the executor stage.

---

## Current data model (as implemented)

### `AnimationComponent`

- Holds an animation state (`Playing | Looping | Paused`).
- Registered via `SignalValue::RegisterAnimation`.

Code: `src/engine/ecs/component/animation.rs`

### `KeyframeComponent`

- Holds `beat: f64` (when the keyframe should fire, in beats).
- Registered via `SignalValue::RegisterKeyframe`.

Code: `src/engine/ecs/component/keyframe.rs`

### `ActionComponent` (already a “signal emitter component”)

- Holds a single `SignalValue` called `signal`.
- Serialization intentionally only supports *action-ish* `SignalValue` variants; non-action variants are coerced to `Noop` on encode.

Code: `src/engine/ecs/component/action.rs`

Important: `ActionComponent` is currently used as *data*: “when this keyframe fires, emit this signal”.

---

## Current runtime behavior: keyframes emitting signals

### Wiring

- `AnimationSystem` maintains runtime state for each animation, including a list of keyframe component ids.
- Keyframes are discovered by topology: `KeyframeComponent` nodes under an `AnimationComponent` root.
- Actions are discovered as children of a keyframe node: any child with an `ActionComponent` is treated as “emit this signal when keyframe is due”.

### Two phases

`AnimationSystem::tick_with_beat(world, beat_now, bpm, rx)` runs with explicit transport arguments provided by `SystemWorld`.

1) **Audio lookahead scheduling**
   - For keyframes due within a lookahead window, `AnimationSystem` emits only tempo-relative scheduling signals.
   - For these signals, it sets `beat_context = Some(kf_global_beat)` so `beat_offset` becomes relative to the keyframe’s intended global beat.
   - Non-audio-scheduled actions are explicitly skipped during lookahead because they have immediate side effects.

2) **Visual / immediate keyframe firing**
   - When the current local beat passes a keyframe, `AnimationSystem` emits all action signals for that keyframe.
   - For tempo-relative scheduling signals, it sets `beat_context = Some(beat_now)` so offsets are relative to “now”.

Code: `src/engine/ecs/system/animation_system.rs`

### Key point about transport

Even though handlers don’t have transport access, the animation use case works because the system that *does* have transport (`AnimationSystem`) stamps the signal’s timing context into the payload before emitting.

---

## Where transport is needed today

### 1) Absolute-beat scheduling signals (already transport-complete)

The following signals are already “data-complete” for scheduling because they carry an explicit `beat: f64`:

- `ScheduleAudioOp { beat, ... }`
- `ScheduleAudioGraphSwap { beat, ... }`
- `ScheduleAudioPitchSetHz { beat, ... }`
- `ScheduleAudioOscillatorEnabled { beat, ... }`
- `ScheduleAudioGainSet { beat, ... }`

These can be executed by the default executor with no additional context.

### 2) Tempo-relative intent signals (need a beat context)

These signals represent a “schedule relative to some beat context”:

- `OscillatorScheduleSetPitch { beat_offset, beat_context, ... }`
- `OscillatorScheduleSetNote { beat_offset, beat_context, ... }`
- `OscillatorScheduleMusicNote { beat_offset, beat_context, ... }`

Today they are resolved inside `ActionSystem` by computing:

- `beat = beat_context.unwrap_or(beat_now) + beat_offset`

`AnimationSystem` currently ensures `beat_context` is filled in before emission (lookahead uses keyframe beat; realtime uses `beat_now`).

### 3) ActionSystem’s implicit “now” scheduling (still transport-leaky)

`ActionSystem` currently hardcodes `let beat_now = 0.0;`.

Most tempo-relative scheduling uses `beat_context`, but there are still action variants that schedule at `beat_now` directly, e.g.:

- `AudioLowPassSetCutoffHz` emits `ScheduleAudioOp { beat: beat_now, ... }`
- `AudioBandPassSetCenterHz` emits `ScheduleAudioOp { beat: beat_now, ... }`

This is the concrete reason transport keeps coming up for handlers.

Code: `src/engine/ecs/system/action_system.rs`

---

## Implication: “executor has transport” is compatible with animation keyframes

Because `AnimationSystem` already receives `(beat_now, bpm)` from `SystemWorld` and stamps beat context into signals, we can keep transport in systems/executor and still support:

- keyframe-relative scheduling (lookahead)
- realtime scheduling at “now”

…as long as we adopt a rule:

> Any signal that needs transport must be *data-complete* before handler dispatch, OR it must be executed in a stage that has transport.

The animation pipeline already follows this rule for the schedule signals it emits.

---

## Should ActionComponent become SignalEmitterComponent?

### What it already is

`ActionComponent { signal: SignalValue }` is already effectively a “signal emitter component” used as data.

It is not a generic emitter (it doesn’t emit by itself). Instead, other systems interpret it as “emit this signal when appropriate”.

### What’s missing for the animation use case

Today, the animation use case needs two additional semantics that are *not expressed in ActionComponent itself*:

- **Which actions are eligible for lookahead** (currently hardcoded: only the `OscillatorSchedule*` family).
- **How beat context is chosen** (lookahead uses keyframe beat; realtime uses `beat_now`).

So the pressure isn’t primarily naming; it’s expressing emitter semantics as data.

### Potential next shapes (data-only)

1) Keep `ActionComponent`, but treat it as an “emit template”
   - Add a second component adjacent to it to express scheduling policy (lookahead-only / realtime-only / both).

2) Replace `ActionComponent` with an explicitly-named data type
   - Example conceptually: `SignalTemplateComponent` or `EmitSignalComponent`.
   - If we do this, do it as a real rename (no aliases), consistent with the no-compat policy.

3) Expand ActionComponent to store multiple signals
   - A keyframe often wants multiple coordinated changes; today it’s “one ActionComponent per signal”.
   - A multi-signal component would reduce node count but complicate edit tooling/serialization.

---

## Options for resolving transport needs without giving handlers transport

### Option A: Make tempo-relative scheduling signals require `beat_context: Some(...)`

- Treat `beat_context: None` as invalid for `OscillatorSchedule*`.
- Then:
  - `AnimationSystem` continues stamping it.
  - Any other producer (e.g. user input systems) must also stamp it.

This keeps handlers transport-blind.

### Option B: Move “resolve schedule intent into schedule ops” into the executor stage

- Add an executor-stage transform:
  - `OscillatorSchedule*` → emits `ScheduleAudio*` with absolute `beat`.
- This stage can read `ClockSystem::beat_now()` for `beat_context=None`.

This makes the rule “executor owns transport” literal, but it does mean the executor is doing more than just immediate mutations.

### Option C: Keep ActionSystem, but remove implicit beat usage

- Replace the remaining `beat_now = 0.0` cases by requiring a beat context in those signals too (or by converting those actions into absolute-beat schedule ops at emission time in systems that have transport).

---

## Open questions

- Do we want any handler to *ever* need transport, or is that a hard rule?
- Are keyframe actions intended to schedule only audio, or eventually also time transforms/visual effects?
  - If we want non-audio scheduling, we likely need a more general “schedule at beat” mechanism beyond audio ops.
- Should lookahead scheduling be an explicit data flag on the emitter component rather than hardcoded by signal variant?

---

## Suggested next investigation steps

- Inventory all action-ish signals that implicitly assume “now” (like the filter cutoff actions) and decide whether to:
  - require beat context in the signal, or
  - resolve them in an executor stage that can read `ClockSystem`.
- Decide whether keyframe actions should remain “one node per signal” or become a multi-signal emitter component.
