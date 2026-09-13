# RuntimeSpec keyframe callback effect profiles

Date: 2026-09-13

Status: proposed focused follow-up task

## Purpose

Avoid invoking a retained `Keyframe.at(...) { ... }` callback in both the
audio-lookahead and visual-due phases when its authored body can only affect
one phase. Classify its potential effects once, when the `meow-meow-script`
session retains the deferred body, then keep that immutable classification as
non-executable metadata on the corresponding `KeyframeComponent`.

This follows [Remove legacy Animation and Keyframe execution](mms-animation-keyframe-legacy-execution-removal.md).
It replaces the `RuntimeClosure.analysis` approach described by
[Keyframe audio lookahead and MusicContext removal](keyframe-audio-lookahead-and-musiccontext-removal.md),
whose engine-owned closure representation is no longer valid.

## Problem

The direct RuntimeSpec path currently has an opaque `SessionCallbackRef`, but
does not retain its effect classification in the ECS. Consequently the
animation system must pessimistically invoke every executable keyframe in both
phases when audio lookahead is enabled:

```text
lookahead window  -> invoke callback, retain audio scheduling intents
visual due        -> invoke callback, retain non-audio intents
```

This wastes evaluation for phase-pure callbacks. More importantly, filtering
intents *after* a whole callback has run is not an adequate long-term
implementation for mixed callbacks: mutation of captured tables, host calls,
or callback-created trees may already have happened during the unwanted pass.

## Decision

The originating RuntimeSpec session owns callback bodies and computes their
profile exactly once when a deferred keyframe body is retained. The ECS stores
only:

```rust
pub struct KeyframeComponent {
    pub beat: f64,
    pub session_callback: Option<SessionCallbackRef>,
    pub effect_profile: KeyframeEffectProfile,
    // ComponentId bookkeeping only
}
```

`KeyframeEffectProfile` is plain, serializable classification metadata, never
an AST, closure, environment, heap, or callback handle without its session:

```rust
pub enum KeyframeEffectProfile {
    None,
    AudioOnly,
    VisualOnly,
    Mixed,
    Unknown,
}
```

Profiles are conservative. `Unknown` means the analyzer cannot prove the
callback phase-pure; it must never be treated as `None`, `AudioOnly`, or
`VisualOnly`.

The session must expose a query or include the profile beside the deferred
callback reference at materialization time. The engine must not recover it by
looking up, cloning, or re-analyzing the callback body.

## Scheduling policy

| Profile | Audio lookahead | Visual due / manual step |
| --- | --- | --- |
| `None` | skip callback | skip callback |
| `AudioOnly` | invoke | skip callback |
| `VisualOnly` | skip callback | invoke |
| `Mixed` | phase-selective invocation | phase-selective invocation |
| `Unknown` | phase-selective invocation | phase-selective invocation |

Declarative `MusicNoteComponent` children remain independently schedulable and
do not require an executable callback.

For `Mixed` and `Unknown`, the RuntimeSpec callback executor must prevent
out-of-phase effects **during** evaluation. It is not sufficient to invoke the
whole callback and discard returned intents afterward. The chosen implementation
may use statement/effect plans retained by the session or a host capability
gate, but it must preserve these invariants:

- audio lookahead cannot mutate visual state, create/attach components, or
  advance mutable captured state solely because it inspected a callback;
- visual execution cannot repeat audio already scheduled for that loop cycle;
- callbacks that are provably phase-pure execute at most once per firing; and
- a callback body, lexical environment, and effect plan remain session-owned.

## Scope

### In scope

- Define `KeyframeEffectProfile` at the Mittens/RuntimeSpec boundary.
- Compute a profile once as `Keyframe.at` retains its deferred callback.
- Transfer only the profile and session-qualified callback reference into the
  direct component registry and `KeyframeComponent`.
- Use the profile to skip impossible scheduler passes.
- Make phase filtering safe before host side effects for `Mixed`/`Unknown`.
- Preserve audio beat-context rewriting, loop-cycle deduplication, paused
  behavior, and manual visual stepping.
- Add profile and execution-count coverage.

### Out of scope

- Restoring engine-local `RuntimeClosure` or its cached analysis.
- Making callbacks portable or serializing their bodies.
- Inferring effects dynamically on every animation frame.
- Changing animation syntax or audio lookahead duration.

## Implementation slices

### 1. Define the boundary type

- Add a transport-safe `KeyframeEffectProfile` to `meow-meow-script` (or a
  shared DTO layer owned by that crate).
- Map the existing `BlockEffectAnalysis` into the profile at deferred-body
  retention time.
- Treat calls whose effect cannot be proved, including helper/function calls
  unless their contract is known, as `Unknown`.
- Include the profile with `MaterializedCE.deferred_callback`, or offer a
  session query that validates the same session and callback identity.

### 2. Store metadata without restoring executable state

- Extend direct `KeyframeComponent::new_with_session_callback` to accept the
  profile.
- Default callback-free keyframes to `None`.
- Reject a callback reference without profile at the direct registry boundary;
  do not guess from constructor syntax.
- Keep callback-bearing serialization explicitly unsupported until
  session-aware authored serialization exists. The profile alone must not make
  a callback serializable.

### 3. Make scheduler decisions from the profile

- `AnimationScheduler::audio_due_keyframes` must omit `None` and `VisualOnly`
  executable callbacks while still considering declarative music children.
- Visual-due and manual stepping must omit `None` and `AudioOnly` callbacks.
- Preserve deterministic beat ordering and loop-cycle audio de-duplication.
- Do not re-run `BlockEffectAnalyzer` from the engine or on each invocation.

### 4. Make mixed/unknown execution safe

- Move phase selection into the RuntimeSpec evaluation/host boundary before
  effects are applied.
- Audit every callback-visible host operation and classify it as audio,
  visual, both, or forbidden in a given phase.
- Ensure an audio pass cannot mutate captured script tables merely because the
  visual branch of a callback was traversed.
- Keep callback invocations raised during execution queued; never recursively
  re-enter the session.

### 5. Tests

- Audio-only callback: one lookahead invocation, correct global beat context,
  no visual-due invocation.
- Visual-only callback: no lookahead invocation, one visual/manual invocation.
- `None` and callback-free declarative keyframes: no executable invocation.
- Mixed callback: audio and visual outputs occur in their respective phases,
  without duplicated captured-table mutation or component creation.
- Unknown callback: follows the safe mixed policy rather than being skipped.
- Loop wrap: audio is scheduled once per keyframe per cycle.
- Paused and manual next/previous: no audio lookahead for manual stepping.
- Closed, foreign, and stale callbacks fail with typed errors.
- A test hook proves profile analysis happens once at callback retention and is
  never recomputed by `AnimationSystem`.

## Exit criteria

- `KeyframeComponent` contains an opaque callback reference plus immutable
  effect profile, and no executable callback representation.
- Phase-pure callbacks are never evaluated in the irrelevant phase.
- `Mixed`/`Unknown` callbacks cannot leak side effects across phase boundaries.
- The engine does not inspect callback bodies or repeat effect analysis.
- Audio lookahead timing and loop-cycle de-duplication remain correct.
