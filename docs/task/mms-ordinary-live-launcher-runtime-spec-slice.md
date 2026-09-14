# Task: cut ordinary live MMS launchers over to retained crate sessions

Date: 2026-09-13

Status: ready for implementation and maintainer smoke testing

## Goal

Move one useful production-shaped slice of ordinary Mittens scene execution
from the frozen engine evaluator to the `meow-meow-script` evaluator:

```text
MMS scene source
  -> meow_meow_script::Session
  -> configured Mittens RuntimeSpec
  -> short-lived MittensHost leases
  -> Universe-owned retained session
  -> callbacks and keyframes serviced on later frames
```

This slice covers the shared live-example launcher plus representative
animation and signal-handler examples. It deliberately does not change every
`MeowMeowRunner` entry point yet. The result is a small end-to-end path that
can be tested interactively before broad caller migration.

## Why this slice comes next

The required crate mechanisms already exist:

- `RuntimeSpecSession` retains crate-owned scopes, tables, modules, and
  callbacks;
- `Session::with_host` lends live engine access without moving evaluator state
  into Mittens;
- `Universe` already owns one optional `RuntimeSpecSession` and services its
  callbacks each frame;
- the main `load` command already starts and retains such a session; and
- Animation and Keyframe callbacks now use opaque crate-owned callback
  references.

The remaining mismatch is at ordinary launch sites. The shared example
launcher and most handwritten examples still call
`MeowMeowRunner::eval_with_world*`, which unconditionally enters
`eval_with_legacy_world_evaluator`. That path now rejects Animation and
Keyframe, so the current focused scripting test run has failures that say to
use `RuntimeSpecSession`.

## Scope

### 1. Add one Universe-owned launch operation

Add a library-level operation with an API equivalent to:

```rust,ignore
impl Universe {
    pub fn load_mms_source_at_path(
        &mut self,
        source: &str,
        path: &str,
    ) -> Result<EvalOutput, String>;
}
```

The exact name and error wrapper may change during implementation, but the
ownership behavior must not:

1. Start evaluation through `RuntimeSpecSession::start_at_path`.
2. Preserve the canonical root `SourceId` used for nested relative imports.
3. Retain the returned session in `Universe` before the next engine frame.
4. Queue the immediate returned intents and process them through the normal
   command pipeline.
5. Continue servicing queued signal and keyframe callbacks through
   `Universe::service_runtime_spec_callbacks`.
6. Replacing a successfully loaded session closes and drops the previous
   session.
7. Return evaluation errors to the caller; do not print them or start a
   window inside this library operation.

Do not create a global session registry. The session has one explicit owner:
the `Universe` that owns the affected world.

Evaluation can mutate the world before returning an error, matching the
current live runner. Transactional world rollback is outside this slice.

### 2. Migrate the shared live-example launcher

Change `examples/mms_live_launcher.inc` to use the new Universe operation.
Remove its direct call to `MeowMeowRunner::eval_with_world_and_assets_at_path`.

Keep its observable bootstrap behavior:

- load the same `include_str!` source;
- use the same `examples/<scene>` source identity;
- report diagnostics and exit before window creation on failure;
- process immediate intents exactly once;
- retain the script session while the window runs; and
- keep native REPL enablement unchanged for this slice.

Because the Universe helper owns command processing, the launcher must not
enqueue or process the returned intents a second time.

### 3. Migrate two handwritten representatives

Migrate these launchers to the same Universe operation:

- `examples/signal-handler.rs`, covering retained `on(...)` callbacks; and
- `examples/gltf-pose-animation.rs`, covering file identity, imports,
  factories used during initial evaluation, Animation, and Keyframe.

If the GLTF pose example requires locally captured pose assets, keep that as a
documented manual prerequisite. Its headless evaluation test should use
fixtures or stop before asset-dependent runtime behavior where possible.

Do not mechanically migrate every handwritten example in this change. Record
any additional example that must move only because it shares code with these
three launcher paths.

### 4. Convert a small regression set to the canonical path

Add or update focused tests proving:

- the shared animation scene starts without the legacy
  `Animation evaluation is unsupported` error;
- the session is still installed in its Universe after initial evaluation;
- a queued signal invokes its MMS callback on a later service/frame pass;
- a visual Keyframe callback can update a captured live component;
- an audio Keyframe callback produces only the expected audio intent in its
  audio phase;
- nested imports resolve relative to the root source identity; and
- dropping or replacing the session prevents its old callbacks from running.

Prefer assertions on ECS state and emitted intents over assertions on log
text. It is acceptable to add a test-only `Universe` session-presence query,
but do not expose evaluator internals, closures, or heap values.

At least one regression must use a scene that the legacy path now rejects.
That makes accidental routing back to `world_evaluator.rs` observable without
adding a production evaluator-selection flag.

## Explicit non-goals

This task does not:

- change the public semantics of all `MeowMeowRunner::eval*` methods;
- migrate evaluation-only or hostless helpers;
- migrate `LoadedMmsModule`, asset templates, panel factories, or paint
  factories;
- migrate the engine MMS REPL or its worker protocol;
- complete all 139 component bindings or remove the current component
  construction fallback;
- remove legacy value conversion from `MittensHost`;
- delete `world_evaluator.rs`, `object.rs`, or either legacy registry;
- redesign native REPL attachment to share the scene's MMS session; or
- require the entire currently-red scripting suite to become green.

No new language semantics may be added to `src/scripting/world_evaluator.rs`
as part of this work.

## Files expected to change

- `src/engine/universe.rs`
- `src/scripting/runner.rs`, only if a small launch/result adapter is needed
- `examples/mms_live_launcher.inc`
- `examples/signal-handler.rs`
- `examples/gltf-pose-animation.rs`
- focused tests in `src/scripting/tests.rs` or `src/engine/universe.rs`

Changes to `crates/meow-meow-script` should be limited to a concrete missing
boundary behavior discovered by these tests. Do not broaden the generic
runner/worker API opportunistically in this slice.

## Automated gate

Run:

```sh
cargo fmt --check
cargo test -p meow-meow-script
cargo test -p mittens-engine runtime_spec_session --lib
cargo test -p mittens-engine migrated_keyframe_mms_examples_materialize_in_live_worlds --lib
cargo test -p mittens-engine gltf_pose_animation_example_imports_named_pose_factories --lib
cargo test -p mittens-engine mms_click_handler_can_emit_data_event --lib
```

Add a focused test filter for the new Universe launch operation and include it
in this gate once named.

The broader diagnostic run is:

```sh
cargo test -p mittens-engine scripting --lib
```

At task creation the broader run reports 311 passed and 23 failed. This slice
must not add failures. Failures owned by the migrated animation/signal launch
path must turn green; unrelated legacy module, round-trip, component-coverage,
or example-content failures may remain and should be recorded rather than
fixed incidentally.

Confirm that migrated launchers no longer name the legacy runner:

```sh
rg -n "MeowMeowRunner::eval_with_world" \
  examples/mms_live_launcher.inc \
  examples/signal-handler.rs \
  examples/gltf-pose-animation.rs
```

Expected result: no matches.

## Maintainer smoke test

### Animation and phase scheduling

```sh
cargo run --release --example animation-example
```

Verify:

- the scene opens without an MMS evaluation error;
- the cube moves and changes color across the loop;
- notes are scheduled without duplicate playback;
- animation continues after initial scene loading; and
- closing the window exits cleanly without a stuck evaluator thread.

### Retained signal callbacks

```sh
cargo run --release --example signal-handler
```

Verify:

- three cubes appear;
- clicking each cube invokes the corresponding MMS handler;
- callbacks continue working across many frames;
- each click runs once; and
- replacing or closing the scene does not leave callbacks from the old
  session active.

### Imports and pose animation

After capturing the pose assets documented by the example:

```sh
cargo run --release --example gltf-pose-animation
```

Verify:

- relative imports resolve without changing the process working directory;
- the scene loads without the legacy Animation rejection;
- imported pose factories produce the expected topology; and
- animation callbacks continue running after initial load.

## Completion criteria

- The shared live-example launcher enters MMS only through a retained
  crate-owned session.
- `signal-handler` and `gltf-pose-animation` use the same Universe-owned path.
- Immediate intents are applied once and delayed callbacks are serviced on
  later frames.
- Source identity and nested relative imports survive the move.
- The selected automated tests pass and the broader scripting run has no new
  failures.
- The three manual smoke tests exhibit the expected animation, audio, signal,
  and import behavior.
- The migrated paths contain no call to the engine-local evaluator.

## Follow-up after smoke approval

Once this slice has been manually accepted, use the same Universe-owned launch
operation to migrate the remaining live example wrappers and application scene
entrypoints. Only then change or retire the ordinary
`MeowMeowRunner::eval_with_world*` compatibility methods. Module/factory and
REPL migration remain separate focused tasks.

## Related tasks

- [MMS/Mittens runtime cutover and legacy deletion](mms-mittens-runtime-cutover-and-legacy-deletion.md)
- [MMS/Mittens 0.8 contract cutover](mms-mittens-0.8-contract-cutover.md)
- [MMS evaluator deduplication checklist](mms-evaluator-deduplication.md)
- [CLI runner for live MMS examples](mms-cli-example-runner.md)
