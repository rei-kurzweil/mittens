# Task: hide live ECS access behind `MittensHost`

Date: 2026-09-14

Status: ready for implementation

## Goal

Make `World` and the other live engine services implementation details of the
Mittens host adapter, rather than parameters of the canonical MMS session and
evaluation APIs.

The desired application-facing shape is:

```rust,ignore
universe.load_mms_source_at_path(source, path)?;
```

The desired runtime boundary is:

```text
application / example
        |
        v
Universe scripting facade
        |
        v
crate-owned MMS Session  <---- HostRequest / HostResponse ---->  MittensHost
                                                            borrows World,
                                                            RxWorld, assets,
                                                            intents, and audio
```

`meow-meow-script` evaluates the language and owns persistent session state.
`MittensHost` temporarily borrows the authoritative ECS and services requests.
Neither application code nor the session abstraction should have to say
"evaluate with world."

## Clarification: this is not a one-way signal boundary

MMS host effects should look command-like, but some of them require a reply
before evaluation can continue:

- constructing or registering a component returns an opaque live handle;
- `query(...)` returns zero, one, or several component handles;
- a component method may return a value such as bounds or state;
- source loading returns resolved source identity and source text; and
- invalid, unavailable, stale, or foreign operations return typed errors.

Fire-and-forget mutations may enqueue engine intents, but the complete
boundary remains correlated `HostRequest` / `HostResponse`. Engine signals use
the opposite direction: they enqueue opaque callback invocations into the
retained MMS session. Neither direction requires MMS to know what a `World`
is.

## Boundary invariant

For the canonical RuntimeSpec path:

1. `meow-meow-script` public types never mention Mittens ECS types.
2. The persistent Mittens session wrapper owns only runtime/session state,
   callback queues, source/module state, and immutable configuration.
3. Session evaluation and callback APIs accept a host lease or a
   host-service closure, not `World`, `RxWorld`, `RenderAssets`, or
   `SignalEmitter` separately.
4. `MittensHost` is the only scripting-boundary object that directly borrows
   those engine services.
5. Application and example code starts scripts through `Universe`, not by
   assembling an evaluator plus a set of ECS borrows.
6. A host lease exists only while one evaluation or callback operation is
   being serviced. It is never stored in the MMS session.

The frozen legacy evaluator may temporarily retain its old
`eval_with_world*` compatibility APIs. Those must remain clearly quarantined
and must not be used by any path migrated under this task. Removing the legacy
surface belongs to the later evaluator-deletion task.

## Current problem

`RuntimeSpecSession` has the correct persistent ownership, but its public
methods currently expose engine implementation details directly:

```rust,ignore
RuntimeSpecSession::start_at_path(
    source,
    path,
    &mut world,
    &mut rx,
    Some(&mut render_assets),
    &mut command_queue,
)
```

The same pattern is repeated for callback and keyframe invocation. Each method
constructs `MittensHost` internally, so session ownership and host
implementation are mixed in `src/scripting/runner.rs`.

This makes callers understand the host's private dependency set, encourages
new `eval_with_world_and_assets...` variants, and makes it harder to evolve
the host toward queued or worker-driven dispatch.

## Scope

### 1. Separate retained MMS state from the engine host lease

Refactor `RuntimeSpecSession` or replace it with an equivalently scoped
Mittens session wrapper whose state layer does not name live ECS services.

It may own:

- the configured MMS runtime and opaque implementation bindings;
- `meow_meow_script::Session` or the later `SessionClient`;
- queued `CallbackInvocation`s;
- callback-delivery lifetime state; and
- session close/reset state.

Its operation layer should have a shape equivalent to one of these:

```rust,ignore
session.evaluate_with_host(host, source, source_id)
session.service_callbacks_with_host(host)
session.invoke_callback_with_host(host, callback, args)
```

or:

```rust,ignore
session.with_host(host, |operation| operation.evaluate(source, source_id))
```

The precise API is an implementation choice. It must take one host boundary,
not a growing list of engine borrows.

The retained session must remain idle and host-free between operations.

### 2. Centralize live service assembly in `MittensHost`

Move construction of short-lived host leases out of the session state layer
and into `src/scripting/host.rs` or a sibling host-service module.

The host adapter owns the knowledge that a live operation may require:

- `World`;
- `RxWorld`;
- `RenderAssets`;
- an intent or signal sink;
- the RuntimeSpec implementation bindings;
- the callback invocation queue and delivery gate; and
- an optional deferred-callback phase.

A small private service bundle is acceptable if it makes borrowing these
fields manageable. It must not become a second host trait, capability catalog,
or public application API.

Do not store `World` or any borrowed engine service in `RuntimeSpecSession`.
Do not place a `World` reference into a crate-owned request, response, runtime,
session, callback, or value DTO.

### 3. Add the Universe scripting facade

Add focused `Universe` operations for the canonical path:

```rust,ignore
impl Universe {
    pub fn load_mms_source_at_path(
        &mut self,
        source: &str,
        path: &str,
    ) -> Result<EvalOutput, String>;

    pub fn close_mms_session(&mut self);
}
```

Names and the error wrapper may be refined, but callers must provide source
information, not individual engine subsystems.

`load_mms_source_at_path` must:

1. create or prepare a crate-owned session;
2. construct one short-lived `MittensHost` lease internally;
3. evaluate with a canonical root `SourceId`;
4. retain the session in the same `Universe`;
5. enqueue immediate intents exactly once;
6. process them through the normal engine command pipeline; and
7. return diagnostics without printing or opening a window.

Replacing a session closes the previous session and makes its callback routes
inert. Do not introduce a global session registry.

`Universe::service_runtime_spec_callbacks` remains an internal frame-loop
operation. It should assemble a host lease and give that single lease to the
session; it should no longer pass individual world/services arguments through
the session API.

### 4. Route keyframe invocation through the same host boundary

The animation scheduler and keyframe evaluator currently call session methods
that accept live engine services. Change this integration so they request a
callback operation through the same short-lived `MittensHost` service path.

Preserve:

- opaque `(SessionHandle, CallbackHandle)` ownership;
- foreign-session and stale-callback errors;
- audio-only versus visual-only effect filtering;
- beat context on scheduled audio intents;
- no synchronous MMS re-entry from inside host dispatch; and
- one serialized evaluator entry at a time.

Phase filtering is host-operation policy. It may live in `MittensHost` or its
private lease/configuration, but it must not make the MMS evaluator aware of
Mittens audio or visual systems.

### 5. Migrate the existing RuntimeSpec entrypoints

Update these callers to use the Universe/host boundary rather than passing
world services into `RuntimeSpecSession`:

- the main `load` command;
- `examples/runtime-spec-smoke.rs`;
- `examples/runtime-spec-emissive-cubes.rs`; and
- RuntimeSpec session tests that are intended to model application launch.

Then use this boundary as the prerequisite for
[the ordinary live-launcher cutover](mms-ordinary-live-launcher-runtime-spec-slice.md).
That task migrates the shared example launcher and representative ordinary
scenes after this host boundary is in place.

Low-level unit tests for `MittensHost` may continue constructing it directly
with a test `World`; they are testing the implementation detail itself.

### 6. Reduce visibility of implementation-shaped APIs

After callers migrate:

- make direct RuntimeSpec session methods that accept `World`, `RxWorld`,
  `RenderAssets`, or `SignalEmitter` private or remove them;
- avoid adding another public `eval_with_world*` method;
- make raw `MittensHost` construction crate-private unless an external host
  embedding use case is explicitly documented; and
- keep public results in terms of `EvalOutput`, typed diagnostics, opaque
  handles, and session lifecycle—not ECS borrows.

Do not preserve a public compatibility facade if preserving it requires
session state to own or expose the live world.

## Error and completion behavior

- Every evaluation or callback operation completes once with a result or
  typed error.
- Missing render assets, Rx routing, audio service, or another required host
  service produces `UnavailableHostContext`, not a panic or silent success.
- A valid handle that no longer resolves is stale; a handle owned by another
  session is foreign. Do not collapse these into a generic host failure while
  changing the boundary.
- An evaluation error may leave already-applied live effects, matching the
  current behavior. Transactional world rollback is out of scope.
- Dropping or explicitly closing the Universe-owned session disables future
  callback delivery.

## Explicit non-goals

This task does not:

- turn every host mutation into an asynchronous engine signal;
- remove correlated host responses or make queries eventually consistent;
- implement the crate worker or `SessionClient` transport;
- migrate `LoadedMmsModule`, template/live factories, or editor asset modules;
- migrate the MMS REPL;
- complete all RuntimeSpec component bindings;
- remove legacy DTO conversions or string-dispatch fallbacks;
- delete `world_evaluator.rs`; or
- remove the frozen legacy `MeowMeowRunner::eval_with_world*` surface before
  its remaining callers migrate.

## Expected files

- `src/scripting/host.rs`
- `src/scripting/runner.rs`, or a new narrowly named session-state module
- `src/engine/universe.rs`
- `src/engine/ecs/system/animation_keyframe_evaluator.rs`
- `src/engine/ecs/system/system_world.rs`
- `src/main.rs`
- `examples/runtime-spec-smoke.rs`
- `examples/runtime-spec-emissive-cubes.rs`
- focused tests in `src/scripting/tests.rs` and session/host modules

Changes inside `crates/meow-meow-script` are warranted only if the existing
host-lease API cannot express this separation without an engine-specific
workaround.

## Automated gate

Run:

```sh
cargo fmt --check
cargo test -p meow-meow-script
cargo test -p mittens-engine runtime_spec_session --lib
cargo test -p mittens-engine retained_callback --lib
cargo test -p mittens-engine keyframe --lib
cargo test -p mittens-engine animation_keyframe_evaluator --lib
cargo check -p mittens-engine --lib
```

Add focused tests proving:

- the Universe facade installs and replaces a session;
- initial evaluation receives query/construction replies through
  `MittensHost`;
- callbacks run on later frame service with the same session state;
- keyframes use the same host-lease construction path;
- missing optional services return typed unavailable-context errors; and
- closing or replacing a session makes earlier routes inert.

Run this source-boundary check:

```sh
rg -n "(World|RxWorld|RenderAssets|SignalEmitter)" \
  src/scripting/runner.rs
```

Any remaining match in the canonical RuntimeSpec session implementation must
be removed or justified as unrelated legacy compatibility code. Prefer moving
the frozen legacy runner into an explicitly named compatibility module if that
makes the boundary mechanically enforceable.

Also confirm that application-shaped RuntimeSpec callers no longer assemble
engine borrows themselves:

```sh
rg -n "RuntimeSpecSession::(start|service_callbacks|invoke_)" \
  src/main.rs examples src/engine
```

Expected result: only low-level host/session integration sites explicitly
approved by this task, not launchers or application code.

## Maintainer smoke test

### Existing crate-session smoke

```sh
cargo run --release --example runtime-spec-smoke
```

Verify that the scene opens and that initial component construction succeeds.

### Retained callbacks and host rebinding

```sh
cargo run --release --example runtime-spec-emissive-cubes
```

Verify that:

- the scene opens with the expected camera, lights, and bloom;
- interactions continue to update emissive state across frames;
- repeated callbacks preserve captured table/session state;
- each callback effect occurs once; and
- closing the window exits cleanly.

### CLI file loading

```sh
cargo run --release --bin mittens-engine -- load examples/animation-example.mms
```

Use the actual CLI spelling if it differs. Verify that relative source
identity is preserved, animation continues after initial loading, and no
caller-facing API requires separate world, Rx, asset, or command arguments.

## Completion criteria

- The canonical MMS session abstraction contains no `World` or borrowed
  engine service.
- Canonical session methods accept one host lease, not individual ECS
  services.
- Only `MittensHost` and its private construction/service layer know which
  engine facilities a host request requires.
- Application-shaped callers launch and retain MMS through `Universe`.
- Initial evaluation, signal callbacks, and keyframe callbacks all use the
  same host-boundary pattern.
- Queries and value-returning operations still receive correlated replies;
  the change does not reduce the protocol to fire-and-forget signals.
- The automated gate passes and the three manual smoke paths behave as before.
- No new production reference to the frozen engine evaluator is introduced.

## Follow-up

After this boundary is accepted, execute the ordinary live-launcher task and
migrate remaining application callers. Later worker, module/factory, REPL, and
legacy-deletion tasks can then replace internals without re-exposing `World`
at the scripting API boundary.

## Related documents

- [Mittens host and MMS runtime boundary](../meow_meow/spec/mittens-host-and-runtime-boundary.md)
- [Ordinary live-launcher RuntimeSpec slice](mms-ordinary-live-launcher-runtime-spec-slice.md)
- [MMS/Mittens runtime cutover and legacy deletion](mms-mittens-runtime-cutover-and-legacy-deletion.md)
- [MMS evaluator deduplication checklist](mms-evaluator-deduplication.md)

