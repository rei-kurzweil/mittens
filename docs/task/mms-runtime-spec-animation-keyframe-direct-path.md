# RuntimeSpec-native Animation and Keyframe direct path

Date: 2026-09-13

Status: proposed focused implementation task

## Purpose

Move `Animation` and `Keyframe` across the Mittens `RuntimeSpec` boundary
without waiting for the complete MMS ownership cutover. The result must remove
their `external_tree_to_legacy` fallback, keep deferred keyframe closure state
inside the originating `meow-meow-script` session, and preserve current
animation, manual stepping, and audio-lookahead behavior.

This is a focused pull-forward from Phase 4 of
[MMS evaluator deduplication](mms-evaluator-deduplication.md). It addresses the
OOM documented in
[Mittens Corp retained callback animation materialization](../bugs/mittens-corp-vehicle-prefab-memory-growth.md).

## Why this can move before the full cutover

Most of the component-facing work is already complete:

- `Animation` and `Keyframe` are declared in the Mittens `RuntimeSpec`;
- their constructors and `Animation` builder calls are typed;
- animation component methods dispatch through opaque operation IDs;
- `AnimationComponent` and `KeyframeComponent` already exist;
- registration intents and `AnimationSystem` scheduling already exist; and
- both components already serialize back to MMS syntax.

The checked entries in
[MMS component migration checklist](mms-component-migration-checklist.md) refer
to component serialization, not RuntimeSpec-native runtime execution. They do
not mean that deferred keyframe bodies have crossed the new ownership boundary.

The work can therefore be isolated to component-tree registration, deferred
callback ownership, and the animation callback execution seam. Modules, the
REPL, asset previews, and unrelated components do not need to migrate first.

## Current path and defect

The current callback-time path is:

```text
RuntimeSpec session materializes Animation + deferred Keyframes
  -> MittensHost receives RegisterComponent(Animation tree)
  -> configured_registry declines the tree
  -> external_tree_to_legacy converts the whole tree
  -> legacy component_registry creates AnimationComponent/KeyframeComponent
  -> KeyframeComponent stores scripting::object::RuntimeClosure
  -> AnimationSystem calls world_evaluator::eval_runtime_closure
```

There are three concrete reasons the configured registry declines it:

1. `Animation` and `Keyframe` are absent from its direct-component set.
2. `tree_is_direct` rejects every tree containing a `deferred_block`.
3. A direct `KeyframeComponent` has nowhere RuntimeSpec-native to store or
   invoke that deferred behavior: its callback field is the engine-local
   `RuntimeClosure` type.

The fallback is unsafe for callback-created animations. Every deferred
keyframe carries a crate-owned lexical environment. `external_tree_to_legacy`
recursively converts those environments and nested function environments,
losing their shared graph structure. A 32-keyframe animation created inside a
retained `GLTFInitialized` callback expanded until it consumed about 20 GiB.

## Ownership invariant

Mittens must not copy a deferred MMS closure body, heap, module environment, or
captured function into an engine-local runtime representation.

The originating `meow-meow-script` session owns:

- the keyframe body;
- its captured lexical environment and heap objects;
- imported module and mutable table identity; and
- the executable callback identity.

The ECS stores only an opaque, session-qualified reference that it can ask the
session owner to invoke.

## Target model

Introduce a session-qualified callback reference, conceptually:

```rust
struct SessionCallbackRef {
    session: SessionHandle,
    callback: CallbackHandle,
}
```

The exact public names may differ, but both identities are required. A bare
`CallbackHandle` is valid only inside its originating session and is
insufficient once scenes, callback-bearing templates, or multiple sessions can
coexist.

When the crate materializes a deferred `Keyframe` body, it must retain the
closure in that session and put an opaque callback reference in the host DTO.
The host creates the ECS tree directly:

```text
Animation MaterializedCE
  -> direct AnimationComponent
  -> direct child KeyframeComponent { beat, callback_ref }
```

No legacy `Value`, `MaterializedCE`, `RuntimeClosure`, or fresh legacy heap is
created on this path.

## Execution seam

`AnimationSystem` currently evaluates keyframe closures synchronously during
its tick so their intents can be processed before downstream transforms,
skinning, and rendering. Moving invocation to the end-of-frame retained
callback drain would introduce a behavioral delay and is not sufficient.

Add a narrow session callback executor available at the animation phase. It
must accept at least:

- the session-qualified callback reference;
- keyframe component/scope identity;
- execution mode (`AudioOnly` or `VisualOnly`);
- beat context for audio scheduling; and
- short-lived access to the live Mittens host services.

Two reasonable implementation shapes are:

1. pass a small `DeferredCallbackExecutor` interface from `Universe` into the
   relevant `SystemWorld` animation phase; or
2. split the update at the animation drain point so `Universe` asks
   `RuntimeSpecSession` to service scheduled keyframes before downstream
   systems continue.

Prefer the first if it preserves Rust borrowing cleanly. Do not put a second
evaluator or copied heap inside `AnimationSystem` merely to avoid changing the
tick boundary.

Callbacks raised while a keyframe callback is executing must remain queued;
do not recursively re-enter the same MMS session.

## Audio and visual execution parity

The direct path must retain the current two-pass semantics:

- audio lookahead evaluates audio effects early using the keyframe's intended
  global beat as `beat_context`;
- visual-due evaluation applies non-audio effects at the visible keyframe;
- the visual pass suppresses audio already scheduled for that loop cycle;
- manual stepping is visual-only; and
- loop-cycle deduplication remains owned by `AnimationSystem`.

Execution policy should be represented in the crate/session invocation API or
in a bounded Mittens host intent sink. It must not require evaluating the body
with `world_evaluator::eval_runtime_closure`.

## Implementation slices

### 1. Add the failing retained-callback regression

- Create a minimal retained session with an event callback that constructs and
  attaches a multi-keyframe animation containing a nested helper closure.
- Prove the callback completes and creates the expected animation/keyframe
  topology.
- Keep the fixture small while developing; the final regression should also
  cover the 32-keyframe ambient-eye factory.
- Assert the new path records zero Animation/Keyframe legacy fallbacks. Avoid a
  wall-clock-only assertion as the primary proof.

### 2. Retain deferred blocks in the crate session

- Allocate an opaque callback identity for a deferred `Keyframe` body.
- Keep the body, captures, heap, and module state inside its session.
- Extend the materialized host DTO with the opaque deferred callback reference
  needed by `Keyframe`.
- Define stale/foreign-session errors and session lease behavior.

### 3. Add direct component construction

- Add `Animation` and `Keyframe` to the configured registry without relaxing
  deferred bodies for arbitrary components.
- Construct `AnimationComponent` state, length, and scope directly from bound
  constructor/initializer operation IDs.
- Construct `KeyframeComponent` beat and callback reference directly.
- Preserve parent-before-child attachment and initialization so
  `RegisterKeyframe` still finds its ancestor animation.
- Reject a deferred block on any component not explicitly declared to own one.

### 4. Invoke keyframes through the originating session

- Replace `AnimationKeyframeEvaluator` calls to
  `world_evaluator::eval_runtime_closure` with the session executor.
- Preserve immediate same-frame signal/intent draining before downstream
  transform work.
- Preserve audio-only, visual-only, beat-context, manual-step, and loop-wrap
  semantics.
- Return typed errors for closed sessions and stale callback references.

### 5. Remove the Animation/Keyframe compatibility path

- Change direct-path tests so falling back for either component is a failure.
- Remove the engine-local `RuntimeClosure` field from `KeyframeComponent` once
  all construction and playback callers use opaque references.
- Remove Animation/Keyframe-specific dependencies on engine-local `Value`,
  heap state, and `world_evaluator`.
- Leave the general compatibility converter only for components not covered by
  this task.

### 6. Restore the affected authored behavior

- Restore `ambient_eye_saccades` in both Mittens Corp scenes.
- Verify desktop still mounts its camera after GLTF initialization.
- Verify both scenes reach startup completion without runaway memory.
- Verify eye animation loops and does not interfere with blink tracking.

### 7. Retire incident instrumentation

- Keep `MITTENS_DEBUG_GROWTH_AUDIT` phase, callback, and host-operation tracing
  enabled as an opt-in diagnostic through implementation and live validation.
- Capture one successful desktop trace and one successful XR trace after
  restoring `ambient_eye_saccades`.
- Then remove the incident-specific per-system phase markers and per-host-call
  tracing from `SystemWorld`, `Universe`, `RuntimeSpecSession`, and
  `MittensHost`.
- Remove instrumentation-only accessors if they have no remaining consumer.
- Retain a compact periodic growth audit only if it is deliberately adopted as
  a general engine diagnostic; do not keep it accidentally as residue from
  this bug.

## Tests

Add or update tests for:

- direct `Animation.playing/looping/paused` construction;
- `length` and `scope` constructor/builder parity;
- direct `Keyframe.at` construction with an opaque callback reference;
- deterministic keyframe ordering;
- a callback-created animation attached to a live component;
- captured component handles and nested helper closures;
- shared mutable captured tables retaining identity across repeated loops;
- one-shot, looping, paused, and manual next/previous behavior;
- visual-only and audio-lookahead execution;
- loop-wrap audio deduplication and beat context;
- callback removal/session close producing a typed failure;
- save/load behavior for keyframe bodies; and
- zero calls to `external_tree_to_legacy` for an Animation/Keyframe tree.

Run at minimum:

```sh
cargo test -p meow-meow-script
cargo test animation --lib
cargo test keyframe --lib
cargo test ambient_eye_saccade --lib
cargo test mittens_corp --lib
cargo check -p mittens-engine --lib
```

Then run both Mittens Corp scenes with the growth audit enabled and confirm
that startup completes and RSS remains bounded.

## Exit criteria

- `Animation` and `Keyframe` trees never call `external_tree_to_legacy`.
- `KeyframeComponent` stores no engine-local executable MMS closure or heap.
- `AnimationSystem` does not call the engine-local evaluator.
- Deferred bodies and captured state remain owned by their originating crate
  session.
- Callback-created 32-keyframe animations complete without material memory
  growth.
- Current visual, manual-step, and audio-lookahead semantics pass.
- The ambient Bisket eye animation is restored in both Mittens Corp examples.
- Incident-specific phase and host-operation instrumentation is removed after
  the successful traces are captured.

## Out of scope

- deleting the entire legacy evaluator and component registry;
- migrating unrelated component types or the REPL;
- redesigning animation interpolation or authored animation syntax;
- changing attachment, mount, rider, or input-routing semantics; and
- replacing `AnimationSystem` scheduling and loop bookkeeping.

## Relationship to the broader cutover

Completing this task satisfies the Animation/Keyframe vertical slice of Phase
4 in [MMS evaluator deduplication](mms-evaluator-deduplication.md). It does not
complete the full callback/module/REPL cutover, but it removes one of the most
dangerous compatibility paths and establishes the session-owned deferred
callback pattern those later migrations can reuse.
