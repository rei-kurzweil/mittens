# MMS animation scheduling and keyframe effect profiles

Date: 2026-09-13

## Short version

An authored animation has two timing needs:

- audio must be scheduled slightly before its beat so the audio thread receives
  it on time;
- visual/world changes should happen when the beat is actually due.

That gives every animation tick two possible keyframe passes:

```text
audio lookahead -> keyframe due -> visual/manual execution
```

A `KeyframeEffectProfile` tells the engine whether an executable keyframe can
matter in either pass. The MMS session computes this profile once when it
retains the `Keyframe.at(...) { ... }` body. The ECS stores the profile and an
opaque session callback reference; it does not own or inspect the callback.

## Authored model and ownership

For this source:

```mms
Animation.looping() {
    Keyframe.at(4) {
        target.update_transform(position, rotation, scale)
    }
}
```

the responsibilities are split as follows:

| Owner | Keeps | Does not keep |
| --- | --- | --- |
| MMS session | callback body, captured environment, heap state, analysis | engine timing state |
| `KeyframeComponent` | beat, `SessionCallbackRef`, effect profile | AST, closure, captured values |
| `AnimationSystem` | registered keyframes, playback state, fired and audio-cycle bookkeeping | callback implementation |

The profile is calculated for the retained deferred keyframe body, not for
every ordinary `fn` declaration. A helper/function call whose contract is not
known makes the enclosing keyframe profile `Unknown`.

## What the animation system does

`AnimationSystem` keeps each animation's keyframes ordered by beat. On a tick it:

1. applies queued state changes and manual-step commands;
2. derives the animation-local beat and handles loop wrapping;
3. asks `AnimationScheduler` for audio work inside the lookahead window;
4. asks it for visual keyframes whose beat is due;
5. records visual firings and per-loop-cycle audio scheduling.

Audio lookahead defaults to 100 ms and is converted to beats using the current
BPM. Audio de-duplication records the last loop cycle in which each keyframe was
scheduled. This lets lookahead cross a loop boundary without scheduling the
same keyframe twice.

Paused animations do not run audio lookahead. Manual next/previous stepping is
visual-only and also suppresses declarative note playback.

Relevant code:

- [`animation_system.rs`](../../src/engine/ecs/system/animation_system.rs)
- [`animation_scheduler.rs`](../../src/engine/ecs/system/animation_scheduler.rs)
- [`animation_keyframe_evaluator.rs`](../../src/engine/ecs/system/animation_keyframe_evaluator.rs)

## Classification and scheduling policy

The profile has five values:

| Profile | Audio lookahead callback | Visual/manual callback |
| --- | --- | --- |
| `None` | skip | skip |
| `AudioOnly` | invoke | skip |
| `VisualOnly` | skip | invoke |
| `Mixed` | phase-selective invocation | phase-selective invocation |
| `Unknown` | conservative phase-selective invocation | conservative phase-selective invocation |

The useful optimization is simple: a proven phase-pure callback is never
evaluated in the irrelevant phase. The engine makes that decision from plain
metadata and never re-runs callback analysis.

Declarative `MusicNoteComponent` children are separate from executable
callbacks. The scheduler still considers a keyframe containing such children
for audio work even if its callback profile is `None` or `VisualOnly`.

## How a profile reaches the scheduler

```text
Keyframe.at body
    -> BlockEffectAnalyzer (once, during retention)
    -> MaterializedCE { deferred_callback, effect_profile }
    -> direct RuntimeSpec registry validation
    -> KeyframeComponent { session_callback, effect_profile }
    -> AnimationScheduler phase checks
```

The direct registry rejects a callback/profile mismatch. Callback-free
keyframes default to `None`. A profile does not make a callback serializable;
callback-bearing keyframe serialization remains unsupported because the body
is still session-owned.

Relevant code:

- [`block_effect_analyzer.rs`](../../crates/meow-meow-script/src/block_effect_analyzer.rs)
- [`evaluator.rs`](../../crates/meow-meow-script/src/evaluator.rs)
- [`object.rs`](../../crates/meow-meow-script/src/object.rs)
- [`keyframe.rs`](../../src/engine/ecs/component/keyframe.rs)
- [`configured_registry.rs`](../../src/scripting/configured_registry.rs)

## The important distinction: selecting a callback vs selecting its effects

The profile completely answers this scheduler question:

> Is it possible for this callback to produce useful work in this phase?

It does not, by itself, answer this evaluator question:

> Which statements and mutations inside a mixed or unknown callback may run in
> this phase?

For `AudioOnly` and `VisualOnly`, skipping the other invocation solves both
performance and duplicate-execution concerns. For `Mixed` and `Unknown`, the
callback may still need two invocations, but each invocation must suppress
out-of-phase effects before they happen.

The current work adds a host capability lease that blocks several
out-of-phase engine operations before dispatch, with returned-intent filtering
remaining as a defensive boundary. This is useful but is not yet a complete
mixed/unknown solution: evaluation can still traverse branches and mutate a
captured MMS table before the host sees any operation.

The complete design therefore needs a session-owned statement/effect plan or
equivalent evaluator-level phase gating. That plan must preserve local values
needed by the selected phase while preventing unrelated captured-state
mutation, component creation, attachment, and duplicate audio scheduling.

## Current implementation status

Implemented:

- profile type and one-time conversion from block analysis;
- profile transport beside the session callback reference;
- required callback/profile pairing at the direct registry boundary;
- profile storage on `KeyframeComponent`;
- scheduler and evaluator omission of irrelevant phase-pure callbacks;
- independent scheduling checks for declarative music-note children;
- an initial host-side phase capability gate;
- focused profile and scheduler tests.

Still needed before the task's full exit criteria are met:

- safe evaluator-level execution for `Mixed` and `Unknown`, including captured
  table mutation;
- a complete audited effect contract instead of the analyzer's current small
  list of recognized visual methods;
- end-to-end invocation-count and output tests for audio-only, visual-only,
  mixed, unknown, loop wrap, pause, and manual stepping;
- proof that analysis runs exactly once and is never repeated by the animation
  system.

The governing task is
[`mms-keyframe-callback-effect-profiles.md`](../task/mms-keyframe-callback-effect-profiles.md).
