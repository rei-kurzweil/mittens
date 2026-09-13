# Remove legacy Animation and Keyframe execution

Date: 2026-09-13

Status: proposed focused follow-up task

## Purpose

Finish the `Animation` and `Keyframe` RuntimeSpec cutover by deleting their
engine-local closure representation and legacy evaluation path. After this
task, executable keyframe bodies are owned and evaluated only by their
originating `meow-meow-script` session.

This task follows
[RuntimeSpec-native Animation and Keyframe direct path](mms-runtime-spec-animation-keyframe-direct-path.md).
That task establishes session-qualified callback references, direct component
construction, and the animation-phase session executor. This follow-up removes
the temporary dual execution model.

The general `src/scripting` compatibility layer remains in Mittens. This task
removes only its authority to construct or execute `Animation` and `Keyframe`.

## Decision

There is one executable representation for a keyframe body:

```text
meow-meow-script session
  owns callback body, lexical environment, heap, and module state
    -> SessionCallbackRef { session, callback }
      -> KeyframeComponent stores the opaque reference
        -> AnimationSystem invokes it through RuntimeSpecSession
```

The following path is removed:

```text
legacy world_evaluator
  -> scripting::object::RuntimeClosure
    -> KeyframeComponent.callback
      -> AnimationKeyframeEvaluator::eval_runtime_closure
```

There is no adapter, side table, feature flag, or evaluator selection for
legacy keyframe closures. A legacy runner asked to execute an `Animation` or
`Keyframe` must return a clear migration error directing the caller to a
RuntimeSpec session.

## Scope

### In scope

- Remove the engine-local `RuntimeClosure` field from `KeyframeComponent`.
- Remove legacy Animation/Keyframe construction from `component_registry`.
- Remove legacy keyframe execution and execution-mode filtering from
  `world_evaluator` and `AnimationKeyframeEvaluator`.
- Make remaining executable MMS launch paths that use Animation/Keyframe run
  through `RuntimeSpecSession`.
- Replace legacy animation/keyframe tests with direct-session integration
  tests.
- Define honest serialization behavior for opaque callback-bearing keyframes.
- Preserve AnimationSystem scheduling, audio lookahead, manual stepping, and
  loop bookkeeping.

### Out of scope

- Deleting all of `src/scripting`.
- Migrating unrelated legacy component construction or component methods.
- Removing legacy module, REPL, preview, or serialization code that does not
  depend on Animation/Keyframe execution.
- Redesigning animation syntax, interpolation, scheduling, or transitions.
- Making opaque callbacks portable between sessions.

## Current temporary state

The direct RuntimeSpec path stores `SessionCallbackRef`, but the ECS and
animation evaluator still accept both representations:

```rust
pub struct KeyframeComponent {
    pub beat: f64,
    pub callback: Option<scripting::object::RuntimeClosure>,
    pub session_callback: Option<meow_meow_script::SessionCallbackRef>,
}
```

`AnimationKeyframeEvaluator` branches between session invocation and
`world_evaluator::eval_runtime_closure`. This compatibility branch means the
engine still owns an executable MMS closure type and every animation semantic
must be reasoned about and tested twice.

Legacy construction currently enters through:

- `src/scripting/world_evaluator.rs`, which materializes a deferred keyframe
  block as an engine `RuntimeClosure`;
- `src/scripting/component_registry.rs`, which installs that closure on a
  `KeyframeComponent`; and
- `src/scripting/host.rs`, whose general external-tree converter can translate
  crate materialized trees into legacy trees.

The direct host already rejects Animation/Keyframe compatibility fallback.
This task makes that rejection structural instead of relying on the continued
existence of a second executable representation.

## Target component model

`KeyframeComponent` keeps timing and, when it has executable behavior, one
opaque callback reference:

```rust
pub struct KeyframeComponent {
    pub beat: f64,
    pub callback: Option<meow_meow_script::SessionCallbackRef>,
    component: Option<ComponentId>,
}
```

The callback remains optional so a keyframe may contain declarative children
such as `MusicNoteComponent` without an executable body. The field name may
remain `session_callback` during the change if that makes review clearer, but
there must not be a second engine-local callback field.

`KeyframeComponent` must not contain:

- a legacy or crate `RuntimeClosure`;
- a copied callback AST or source body;
- a heap or captured environment;
- an unqualified `CallbackHandle`; or
- a callable Rust closure.

## Legacy caller behavior

Legacy APIs may continue evaluating MMS that does not use Animation/Keyframe.
When legacy evaluation encounters either component, it must fail before
spawning a partial tree.

The error must identify:

- the unsupported component (`Animation` or `Keyframe`);
- that executable animation requires the RuntimeSpec runtime; and
- the caller-facing replacement, normally `RuntimeSpecSession` or the
  canonical engine runner backed by it.

Do not:

- discard a keyframe body and create an empty keyframe;
- eagerly execute the deferred body;
- translate it into a legacy closure;
- fall back only for imports or callback-created trees; or
- allow behavior to depend on which constructor spelling was used.

If a compatibility `MeowMeowRunner` entry point is intended to remain public,
it may delegate the whole evaluation to the crate runtime. It must not invoke
the old Animation/Keyframe implementation.

## Implementation slices

### 1. Lock the RuntimeSpec-only boundary with tests

- Add a direct-session fixture for each Animation constructor:
  `playing`, `paused`, and `looping`.
- Cover `length` and `scope` through every supported constructor/builder form.
- Assert each direct `Keyframe.at` contains only a session-qualified callback
  reference.
- Add a test that attempts legacy Animation/Keyframe evaluation and receives
  the intentional migration error before any component is spawned.
- Add an explicit assertion or test hook proving
  `external_tree_to_legacy` is never called for either component.

### 2. Remove the ECS dual representation

- Remove `KeyframeComponent.callback: Option<RuntimeClosure>`.
- Remove `KeyframeComponent::new_with_callback`.
- Rename or retain `session_callback` as the single callback field.
- Update `to_mms_ast` so it does not inspect or clone an executable closure.
- Update all Rust construction sites and unit-test fixtures.

The build must contain no conversion from an MMS deferred keyframe body into
an engine-local callback value.

### 3. Remove legacy construction

- Delete the Keyframe `deferred_block` installation branches from
  `component_registry`.
- Make the legacy evaluator/registry reject Animation and Keyframe
  deterministically.
- Remove Animation/Keyframe-specific conversion from
  `external_tree_to_legacy`; keep the general converter for unrelated
  components.
- Remove engine-local heap/environment traversal that exists only to convert
  keyframe callbacks.
- Preserve hostless crate materialization where it is a crate feature; a
  crate-owned `RuntimeClosure` used internally by a hostless crate operation
  is not an engine execution path.

### 4. Remove legacy playback

- Delete `eval_runtime_closure` calls from `AnimationKeyframeEvaluator`.
- Delete `RuntimeClosureExecMode::KeyframeAudioOnly` and
  `RuntimeClosureExecMode::KeyframeVisualOnly` when no remaining caller needs
  them.
- Delete `world_evaluator::eval_runtime_closure` if it becomes unused.
- Make audio-due and visual-due keyframes invoke only the session callback
  executor plus declarative MusicNote children.
- Preserve the rule that callbacks raised during keyframe execution remain
  queued and never recursively re-enter the session.

An executable keyframe encountered without its owning session executor must
produce a typed engine/runtime error. It must not log and continue as though
the keyframe succeeded.

### 5. Migrate callers and tests

- Inventory tests, examples, previews, asset factories, and engine utilities
  that currently call a legacy runner with Animation/Keyframe source.
- Move executable callers to a persistent `RuntimeSpecSession` so callback
  handles remain valid for the lifetime of their ECS keyframes.
- Rewrite AnimationSystem callback tests around a real or deliberately small
  crate session instead of hand-building a `RuntimeClosure`.
- Keep scheduler-only tests independent of MMS by using callback-free
  keyframes and declarative child components where possible.
- Delete tests whose only purpose was validating the removed legacy closure
  machinery.

Do not mechanically switch a temporary evaluation call to a session that is
dropped immediately. Any resulting keyframe component must not outlive its
session owner.

### 6. Define serialization behavior

An opaque callback reference is deliberately insufficient to reconstruct MMS
source. Removing `RuntimeClosure` must not cause `to_mms_ast` to silently emit
an empty keyframe body.

Choose and implement one explicit behavior:

1. Preferred: make scene serialization session-aware and ask the owning crate
   session for a serializable authored representation without moving callback
   ownership into the ECS; or
2. return a typed unsupported-serialization error for a callback-bearing
   runtime keyframe until session-aware serialization lands.

Storing a copied callback body or captured environment in
`KeyframeComponent` is not an acceptable serialization shortcut.

Add save/load coverage for the chosen behavior. If serialization is deferred,
the test must prove it fails explicitly rather than losing the body.

### 7. Validate the authored paths

- Run the 32-keyframe `ambient_eye_saccades` factory through a retained
  RuntimeSpec callback.
- Run both Mittens Corp scenes and confirm the eye animation executes without
  legacy conversion.
- Confirm desktop camera mounting still occurs after GLTF initialization.
- Confirm visual animation, manual stepping, and audio scheduling remain
  same-frame where required.
- Confirm callback/session teardown returns typed closed, foreign, and stale
  errors without panics or use-after-release.

## Required tests

At minimum, cover:

- direct `Animation.playing`, `paused`, and `looping` construction;
- direct `length` and `scope` parity;
- callback-free declarative keyframes;
- direct opaque keyframe callback construction;
- deterministic ordering for equal and unequal beats;
- one-shot and looping playback;
- paused animations and manual next/previous;
- visual-only filtering;
- audio lookahead, beat context, and loop-cycle deduplication;
- captured live component handles;
- nested helper closures;
- mutable captured table identity across repeated keyframe invocations;
- callback-created 32-keyframe animations;
- missing, closed, foreign, and stale session/callback failures;
- legacy evaluation rejection with no partial ECS mutation;
- zero Animation/Keyframe compatibility conversions; and
- explicit save/load behavior for callback-bearing keyframes.

Run:

```sh
cargo test -p meow-meow-script
cargo test animation --lib
cargo test keyframe --lib
cargo test ambient_eye_saccade --lib
cargo test mittens_corp --lib
cargo check -p mittens-engine --lib
```

Then run both Mittens Corp scenes with the growth audit enabled before retiring
the incident instrumentation tracked by the direct-path task.

## Exit criteria

- `KeyframeComponent` contains no engine-local executable closure, callback
  AST, captured environment, or heap.
- `AnimationKeyframeEvaluator` has no dependency on `world_evaluator`.
- No production Animation/Keyframe construction path reads a legacy
  `deferred_block`.
- No Animation/Keyframe tree reaches `external_tree_to_legacy`.
- Legacy evaluation fails clearly and atomically when it encounters either
  component, unless the compatibility entry point delegates the entire run to
  RuntimeSpec.
- All executable Animation/Keyframe callers retain an owning RuntimeSpec
  session for as long as their keyframes remain live.
- Visual, audio-lookahead, looping, and manual-step behavior is covered on the
  session callback path.
- Callback-bearing keyframe serialization succeeds through session-aware
  serialization or fails with a typed error; it never silently loses behavior.
- The focused and crate test suites pass.

## Relationship to the broader cutover

This completes the Animation/Keyframe portion of Phase 4 in
[MMS evaluator deduplication](mms-evaluator-deduplication.md). It establishes a
hard component-level cutover while allowing unrelated legacy scripting paths
to remain until their own focused migrations are complete.
