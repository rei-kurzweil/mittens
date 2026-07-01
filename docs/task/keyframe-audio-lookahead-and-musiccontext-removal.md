# Keyframe Audio Lookahead And MusicContext Removal

## Problem

`audio-music-demo` plays again after the `Keyframe { ... }` refactor,
but the current imperative `MusicNote...` path appears to have reintroduced
timing jitter / latency.

The strongest current hypothesis is:

- legacy audio auth (`ActionComponent` audio intents and `MusicNoteComponent`
  children under a keyframe) participates in the animation system's audio
  lookahead scheduling
- imperative `Keyframe.at(...) { MusicNote.e(...) }` callbacks currently emit
  `AudioSchedulePlay` only when the keyframe becomes visually due
- that means the callback path is missing the normal 100 ms lead time and is
  more vulnerable to frame jitter, especially near loop wrap

Observed symptom:

- loop restarts can sound slightly late or inconsistent
- the regression is most noticeable in `examples/audio-music-demo.mms`

## Goal

Restore deterministic audio scheduling for imperative keyframe blocks, remove
the redundant `MusicContext` string-lookup layer from authored MMS, and stop
treating `MusicNote` as a component-shaped MMS expression.

## Why `MusicContext` Should Go

The current authoring shape:

```mms
MusicContext {
    voice("synth_lead", "[name='synth_lead']")

    Animation.looping() {
        Keyframe.at(0.0) {
            MusicNote.e(4, 0.25, "synth_lead")
        }
    }
}
```

adds an extra indirection layer:

- `MusicNote` constructor takes a string key
- the key resolves through `MusicContext`
- `MusicContext` resolves that key to an audio source handle

That is redundant when authored MMS already has direct live component handles.
The preferred direction is:

```mms
let synth_lead = AudioOscillator.square() { ... }

Animation.looping() {
    Keyframe.at(0.0) {
        synth_lead.play_note(MusicNote.e(4, 0.25))
    }
}
```

or equivalent direct-handle sugar where the singleton audio source is the
method receiver and note selection is just a host-provided helper, not a
routing object.

## Why `MusicNote` Should Stop Being A Component

The current `MusicNote...` path appears to rely on awkward expression-shape
special handling:

- authored `MusicNote.e(...)` looks component-like in MMS
- evaluation then overrides that component expression to behave like a method
  call / host dispatch
- that creates a janky hybrid model where `MusicNote` is neither a real
  component instance nor a normal built-in function namespace

That shape is not buying us anything:

- we do not have a meaningful need to construct and persist `MusicNote`
  instances in MMS state
- a note here is not durable scene data; it is just a request to invoke a host
  audio action with `(octave, amplitude, duration, voice)`-style parameters
- the useful authored operation is "play C/E/G with these arguments", not
  "materialize a `MusicNote` object"

Preferred direction:

- `MusicNote` becomes a built-in table exposed by the script host
- built-in table keys `a`, `b`, `c`, `d`, `e`, `f`, `g` point directly to
  built-in host functions
- `MusicNote.c(...)` should resolve to a host call immediately, without
  evaluating more MMS functions or pretending `MusicNote` is a component
- this generalizes cleanly to other host-provided namespaces: built-in tables
  can expose functions and data without faking component semantics

Example target shape:

```mms
let synth_lead = AudioOscillator.square() { ... }

Animation.looping() {
    Keyframe.at(0.0) {
        synth_lead.play_note(MusicNote.e(4, 0.8, 0.25, "lead"))
    }
}
```

or, if the host call should target the source directly:

```mms
Animation.looping() {
    Keyframe.at(0.0) {
        synth_lead.play_note_e(4, 0.8, 0.25, "lead")
    }
}
```

The exact call surface can still be chosen later. The important semantic change
is that `MusicNote` is a built-in host namespace, not a component.

## Investigation Tasks

1. Instrument animation audio timing.
   - Log when lookahead scheduling happens.
   - Log when imperative keyframe callbacks emit audio intents.
   - Log resolved target source, requested beat, current transport beat, and
     whether the intent entered the pending queue or the ready queue.
   - Pay special attention to loop wrap boundaries.

2. Compare the three audio paths side by side.
   - `ActionComponent` with `AudioSchedulePlay`
   - `MusicNoteComponent` keyframe children
   - imperative `Keyframe { MusicNote... }` callback emits

3. Verify whether the callback path entirely bypasses lookahead.
   - If yes, fix that instead of trying to compensate elsewhere.
   - The intended semantics are that authored keyframe audio should schedule
     against the keyframe beat, not "play now at visual due time".

4. Audit loop-cycle bookkeeping.
   - Confirm that lookahead dedupe state is correct across wrap.
   - Confirm the same keyframe is not skipped or scheduled too late when the
     cycle increments.

## Implementation Direction

### Phase 1: Fix timing without changing the public API yet

- Make imperative keyframe audio participate in the same lookahead scheduling
  pass as legacy keyframe audio
- avoid emitting callback-authored audio directly at visual due time when the
  lookahead pass could have scheduled it earlier
- keep non-audio side effects in callbacks on the visual-due path

One plausible shape:

- evaluate the callback in a "collect audio intents only" mode during the
  lookahead pass
- rewrite collected audio intents with `beat_context = keyframe_global_beat`
- enqueue them through the existing pending-intent path
- continue executing the full callback normally on the visual-due pass, but
  suppress duplicate audio re-emission for the same keyframe cycle

### Phase 2: Replace `MusicContext`

- deprecate string-key voice lookup in authored MMS
- introduce direct audio-source methods on live handles, for example:
  - `audio_source.play_note(note)`
  - `audio_source.play_note_at(note, beat_offset)`
  - `audio_source.play_clip(...)` or equivalent if needed
- replace `MusicNoteComponent` / component-expression special casing with a
  host-owned built-in `MusicNote` table
- `MusicNote.a/b/c/d/e/f/g` become built-in host functions, not component
  constructors
- preserve direct component-handle targeting instead of forcing name-based
  lookup through a separate context object

### Phase 3: Clean Up MMS Runtime Semantics

- remove the evaluator hack that treats `MusicNote` component expressions as a
  special method-call-like path
- teach the MMS runtime a first-class built-in table concept for host-exposed
  namespaces
- use built-in tables for future host function/data surfaces where a component
  model would be artificial

## Acceptance Criteria

- `examples/audio-music-demo.mms` plays with stable timing across loop
  boundaries
- imperative keyframe-authored audio is scheduled ahead of time, not only at
  visual due execution
- instrumentation can prove when and where an audio intent was queued
- authored MMS no longer requires `MusicContext` voice names for the common
  direct-handle case
- `MusicNote` is no longer authored or implemented as a component expression;
  it is a built-in table whose note keys dispatch directly to host functions

## Notes

- The recent "make imperative `MusicNote` audible" fix solved silence by
  translating callback-authored `MusicNote` expressions into
  `AudioSchedulePlay`, but that was only the first half of the job.
- The remaining issue is scheduling semantics, not basic note resolution.
- Once the scheduling regression is fixed, we should simplify the authoring and
  evaluator model instead of preserving the current `MusicNote` special case.
