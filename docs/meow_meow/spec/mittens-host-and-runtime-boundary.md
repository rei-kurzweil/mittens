# Mittens host and MMS runtime boundary

Date: 2026-07-30

Status: normative

## Purpose

This document defines the ownership and execution boundary between
`meow-meow-script` and `mittens-engine`.

The central invariant is:

> `meow-meow-script` is the sole implementation of MMS parsing, evaluation,
> runtime values, heap/session state, modules, and callbacks. Mittens supplies
> one `RuntimeSpec` and services the host operations declared by that
> specification on the main thread.

Requirements in this document use **must**, **must not**, **should**, and
**may** in their normative sense. The migration checklist is
[MMS evaluator deduplication](../../task/mms-evaluator-deduplication.md).

## Ownership

### `meow-meow-script`

The language crate owns:

- tokenization, parsing, AST transforms, validation, and source diagnostics
- expression, statement, component-expression, and builtin evaluation
- the public runtime `Value` and component-tree DTOs
- scopes, frames, heap objects, closures, captures, and shared object identity
- module loading semantics, module state, imports, and exports
- callback registration and invocation state
- template and live evaluation modes
- typed evaluation, protocol, and host errors
- `RuntimeSpec`, its nested builder API, and specification validation
- `ComponentNamePolicy` and the standard open-name runtime
- persistent evaluator sessions and the transport-neutral worker protocol
- the host-generic `Runner`, operation/result events, and component sink DTOs
- the host-generic REPL, session-value navigation, and inspection DTOs
- `StandardHost`, including a collecting component forest, local handles,
  local reflection, and filesystem source loading
- optional blocking, polling, async, filesystem, and terminal adapters

There must be no second implementation of these responsibilities in Mittens.

### `mittens-engine`

Mittens owns:

- the one builder expression that assembles the `RuntimeSpec` given to MMS
- the engine implementations bound to host operations in that expression
- `MittensHost`, which translates host requests into engine operations
- opaque component-handle translation and ECS lifetime validation
- component construction, registration, attachment, and initialization
- queries, component-method dispatch, and engine API dispatch
- `World`, `RxWorld`, render assets, intents, audio, and engine mutations
- main-thread orchestration of the crate worker
- live world/component inspection for the generic REPL
- signal payload adaptation
- frame-loop and terminal-ownership integration
- engine-facing runner/REPL compatibility wrappers and temporary re-exports

The host may convert an already evaluated crate DTO into engine data. It must
not inspect an AST in order to decide language semantics or evaluate an
arbitrary MMS expression.

## Three runtime responsibilities

The architecture has three distinct objects. Their lifetimes and ownership
must not be conflated.

### 1. `meow_meow_script::RuntimeSpec` and `Runtime`

`RuntimeSpec` is the one immutable description of the MMS vocabulary exposed
by a runtime. Mittens assembles it with the crate-owned nested builder API and
gives the completed specification to `meow_meow_script::Runtime`. The crate
also owns the standard specification; using it does not require a builder.

Standard value types and pure builtins are seeded into the same builder before
Mittens adds engine vocabulary. Grammar remains unconditionally crate-owned.
This produces one completed runtime specification, not a core runtime
specification plus a host catalog.

It describes and validates:

- the component-name policy
- canonical component names and aliases
- constructors and builder calls
- named and positional properties
- component methods and signatures
- signals and their payload types
- pure and host-dispatched builtins
- global and namespaced engine APIs

`Runtime` is the crate's validated, executable language configuration compiled
from exactly one `RuntimeSpec`. It does not merge that specification with a
second host capability description.

Neither `RuntimeSpec` nor `Runtime` contains a script heap, a `World`, or
per-evaluation bindings. A configured runtime may be shared by multiple
sessions.

#### Component-name policy

Every `RuntimeSpec` contains exactly one `ComponentNamePolicy`:

- `OpenUppercase` accepts registered names and aliases plus unregistered names
  matching ASCII `[A-Z][A-Za-z0-9_]*`.
- `StrictRegistered` accepts only registered canonical names and aliases.

`Runtime::standard()` uses a crate-owned standard `RuntimeSpec` with
`OpenUppercase`; callers do not need to construct or invoke a builder.
Mittens' one assembled specification uses `StrictRegistered`, so an unknown
component name remains a validation error in engine scripts.

An unregistered component accepted by `OpenUppercase` is unvalidated
structural data. It has no inferred constructor, property schema, method
surface, signal surface, or host implementation. Its body follows these
rules:

- a direct assignment becomes an authored named field
- a component-expression item becomes an ordered child
- a string positional remains an authored positional
- any unrelated expression is evaluated for its ordinary effects and result,
  but its result does not become a field or child

Registered components keep the component-body behavior declared by the same
`RuntimeSpec`. This policy must not be reimplemented as an engine-local
component-name map.

### 2. Persistent crate-owned worker/session

A session owns all mutable MMS state:

- lexical scopes and frames
- the object heap and identity-bearing values
- evaluated modules and export state
- closure bodies and captures
- registered callbacks
- source/module context
- opaque live component handles

The session is host-independent and remains alive across initial evaluation,
module export calls, callbacks, keyframes, and REPL snippets. It runs inside a
crate-owned worker, one operation at a time. An operation may pause while a
correlated host request is serviced, but the session and its heap remain
resident.

The old `Session<H>` shape, where the session permanently owns a host, is not
the embedding boundary. A short-lived host is supplied indirectly for each
operation through the request/response protocol.

### 3. Short-lived `MittensHost`

`MittensHost` is constructed or borrowed on the main thread while servicing
an operation. It may temporarily borrow:

- `World`
- `RxWorld`
- a signal or intent sink
- render assets
- component and method registries
- other engine services required by operations in the specification

It services host requests and is then released. It is not stored in the MMS
session, and none of its engine borrows cross to the worker thread.

```text
main thread                                      crate worker
───────────                                      ────────────
one RuntimeSpec ─────── compile Runtime ────────► scopes / heap / modules

runner operation ──────────────────────────────► evaluate or invoke
                      HostRequest { id, ... } ◄── pause operation
short-lived MittensHost dispatch
                      HostResponse { id, ... } ─► resume same operation
operation result ◄────────────────────────────── completion or typed error
```

## Worker/session protocol

The protocol is transport-neutral. The crate defines message DTOs and state
transitions; the embedding chooses bounded channels, queues, and wake-up
mechanisms.

### Engine-to-worker operations

The protocol must support:

- source evaluation with source identity and explicit evaluation mode
- module loading
- named and sequence export calls
- callback invocation
- REPL snippet evaluation
- REPL navigation or inspection requests
- session reset
- orderly shutdown

Each operation has an operation ID. An operation completes exactly once with a
value/evaluation result or a typed error. Reset invalidates the session's
bindings, modules, callbacks, and handles before acknowledging completion.
Shutdown rejects new work, resolves or cancels outstanding work, acknowledges
completion, and permits the worker to be joined.

### Worker-to-engine messages

The protocol must support:

- a correlated `HostRequest`
- operation completion
- typed evaluation or protocol errors
- shutdown acknowledgement

Every host request contains both its parent operation ID and a request ID.
Every `HostResponse` echoes both identities. The worker must reject a missing,
duplicate, stale, or mismatched response rather than applying it to another
operation.

An operation may issue multiple host calls. The session is sequential unless a
future protocol explicitly defines concurrent operations; correlation must not
depend on that implementation detail.

### Source loading

Source loading is a host capability. Import evaluation and module caching are
crate responsibilities, but resolving or reading an engine-relative source is
not.

A source-load request includes:

- the importing source identity, when one exists
- the requested import specifier

A successful response includes:

- a stable resolved source identity
- source text

The crate uses the resolved identity for diagnostics, relative imports, and
module-cache identity. The evaluator must not call engine-specific filesystem,
asset, or URI APIs directly.

### Errors and recovery

Hostless operation attempts, unavailable host context, invalid requests,
foreign or stale handles, conversion failures, source-loading failures,
evaluation failures, timeouts, and protocol violations are distinct typed
errors.

An error completes the affected operation without discarding the session
unless the error is explicitly fatal. A runner timeout must not leave an
unidentified host response capable of resuming later work. Tests must prove
that the next operation can run after recoverable failures.

There is no production fallback to the engine evaluator.

## Runtime and transport values

`meow-meow-script` owns the only runtime `Value`, component-tree DTO, closure
representation, and heap model.

Identity-bearing heap values remain inside their originating session. Values
that cross the worker boundary must use crate-owned transport DTOs:

- owned scalar values and snapshots where identity is not required
- opaque component handles for host-owned ECS objects
- opaque callback references for session-owned closures
- crate-owned materialized component trees for construction

Mittens must not define an equivalent runtime object model. Legacy module
paths may temporarily re-export crate-owned DTOs so existing Rust callers
remain source-compatible.

## Callbacks and delayed execution

Delayed execution is identified by an opaque pair:

```text
(SessionHandle, CallbackHandle)
```

The session part selects the persistent heap and module state. The callback
part selects a crate-owned closure in that session.

The crate retains:

- the closure body
- lexical captures
- captured tables and arrays
- captured component handles
- all heap aliasing needed by the closure

Engine handler and keyframe components retain only the opaque callback
reference. `RuntimeClosure`, an AST function body, and a raw function `Value`
must not be stored in ECS components or sent through the host boundary.

Invoking a handler, keyframe, animation lookahead, or other delayed action
sends `InvokeCallback` to the originating session. A callback-bearing module
or template artifact must therefore retain a lease on that session. Releasing
the final lease may shut down the session and invalidate its callbacks.

## Component handles

A `ComponentHandle` is opaque to MMS. Before every query, method, attachment,
mutation, audio operation, or other component access, `MittensHost` must
validate:

1. the handle belongs to the requesting session; and
2. its encoded ECS identity still names the same live slotmap generation.

Failure of the first check is a typed foreign-handle error. Failure of the
second is a typed stale-handle error. Raw ECS identifiers and their generation
bits must not be truncated or exposed to scripts.

## Component values and reflection

Rust can already inspect a materialized component's type and ordered children.
The target MMS surface makes the same structural information available without
inventing another runtime value kind. Functions already return ordinary
component expressions, so there is no separate “function component” type.

The following behavior is universal for both a static `ComponentExpr` and a
live `ComponentObject`:

- `node.type()` returns the component's authored or canonical type name as a
  string.
- `node.children()` returns a new ordered array containing only immediate
  component children. Positionals, named fields, and unrelated expression
  results are excluded.
- `node.field` reads an authored named field.
- a missing local field returns `null`.

For a static or collected component expression, these operations inspect the
crate-owned artifact locally and never issue a host request. For a live
component object, they use generic host inspection. A successful live field
miss returns `null`; a host that cannot inspect the requested information
returns a typed `UnsupportedHostOperation`. Foreign and stale handles retain
their distinct typed errors and must not be converted to `null` or
“unsupported.”

`type()` and `children()` are reserved only while dispatching a call on a
component receiver. An authored component field named `type` remains legal
and readable as `node.type`; table fields named `type` or `children` remain
ordinary table fields.

## Table dot access and methods

For a heap-backed table:

- `table.name` is exactly the same lookup as `table["name"]`
- `table.method(args)` first reads `table["method"]`, requires it to be a
  function, and calls it with the same table as the first `self` argument,
  followed by the explicit arguments

Dot reads and method calls retain the table's heap identity. Mutation through
`self`, aliases, or captured closures is therefore visible through every
reference to that table. Missing dot reads have the same result as existing
missing-key indexing; calls through a missing or non-function field produce
the corresponding typed call error.

## One specification, assembled with nested builders

There is only one vocabulary specification: the crate-owned `RuntimeSpec`
that Mittens builds and gives to MMS. There is not a second Mittens catalog,
capability schema, parser-name list, or method-support specification.

The crate exposes a typed nested Rust builder. The intended shape is
illustrative here; exact type names may change:

```rust
let built = RuntimeSpec::builder()
    .with_standard_builtins()
    .component_name_policy(ComponentNamePolicy::StrictRegistered)
    .component("Transform", |component| {
        component
            .alias("T")
            .constructor("identity", |constructor| {
                constructor
                    .signature(sig!(() -> component))
                    .construct_with(mittens_components::transform_identity)
            })
            .constructor("position", |constructor| {
                constructor
                    .signature(sig!(
                        (x: Number, y: Number, z: Number) -> component
                    ))
                    .construct_with(mittens_components::transform_position)
            })
            .positional(0, "x", Type::Number)
            .property("position", |property| {
                property
                    .value_type(Type::Vec3)
                    .materialize_with(mittens_components::set_transform_position)
            })
            .method("set_position", |method| {
                method
                    .signature(sig!(
                        (x: Number, y: Number, z: Number) -> Unit
                    ))
                    .dispatch_with(mittens_dispatch::transform_set_position)
            })
            .signal("Changed", |signal| {
                signal
                    .field("position", Type::Vec3)
                    .dispatch_with(mittens_signals::transform_changed)
            })
    })
    .component("Color", |component| {
        component
            .alias("C")
            .constructor("rgba", |constructor| {
                constructor
                    .signature(sig!(
                        (r: Number, g: Number, b: Number, a: Number) -> component
                    ))
                    .construct_with(mittens_components::color_rgba)
            })
            .property("rgba", |property| {
                property
                    .value_type(Type::Vec4)
                    .materialize_with(mittens_components::set_color_rgba)
            })
            .method("set_color", |method| {
                method
                    .signature(sig!(
                        (r: Number, g: Number, b: Number, a: Number) -> Unit
                    ))
                    .dispatch_with(mittens_dispatch::color_set)
            })
    })
    .host_builtin("query", |builtin| {
        builtin
            .signature(sig!((selector: String) -> Component))
            .dispatch_with(mittens_dispatch::query)
    })
    .namespace("Audio", |namespace| {
        namespace.function("play", |function| {
            function
                .signature(sig!((source: Component) -> Unit))
                .dispatch_with(mittens_dispatch::audio_play)
        })
    })
    .build()?;
```

The nesting makes ownership visible:

- a component owns its aliases, constructors, component-expression builder
  calls, positionals, properties, methods, and signals
- a signal owns its payload fields and engine-to-MMS payload adapter
- a namespace owns its functions and nested namespaces
- each constructor, property, method, builtin, signal, or API declaration owns
  its signature/schema and its pure implementation or host-operation binding

`with_standard_builtins()` contributes crate-provided pure builtins and their
types to this same builder. A finished build produces:

- one `RuntimeSpec`, consumed by `meow_meow_script::Runtime`; and
- host implementation bindings indexed by opaque operation IDs, consumed by
  `MittensHost`.

```text
one nested Mittens builder expression
                    │
                    ▼
              build + validate
                ┌───┴──────────────────┐
                ▼                      ▼
        one RuntimeSpec       opaque implementation bindings
                │                      │
                ▼                      ▼
        MMS Runtime/worker          MittensHost
```

The implementation bindings are not a second specification. They contain no
names, aliases, signatures, signal shapes, or parser metadata. They are the
engine functions attached while building the one specification. The builder
must fail if a declared host operation lacks an implementation or if an
implementation is not reachable from a declaration.

Mittens must not hand-write either output after `build()`. In particular,
`SUPPORTED_COMPONENT_NAMES`, parser-only component lists, method-support
matches, separate `HostCapabilities`, and independently maintained API or
signal lists must be removed once their consumers migrate.

The crate validates duplicate names and aliases, invalid nesting, conflicting
signatures, missing host bindings, and unreachable bindings. Consistency tests
must prove that every item in the completed `RuntimeSpec` is parseable and
that every host-effectful item resolves to exactly one host implementation.

`CallApi` and component-method requests carry the opaque operation ID assigned
by this build, not a string looked up in an independent match. If the current
host context lacks a required service such as render assets, dispatch returns
a typed unavailable-context error. An operation absent from `RuntimeSpec` is a
validation error and must never reach `MittensHost`.

REPL operations that are part of MMS vocabulary follow the same rule. They
must be declared and bound in the one builder or remain unavailable; they
must not silently return no-op responses.

REPL shell commands such as `ls`, `pwd`, and `cd` are not MMS vocabulary.
They belong to the crate's generic REPL and generic inspection protocol, so
they require no `RuntimeSpec` declaration. A call-shaped operation such as
`tree(value)` must be classified explicitly as either a standard crate builtin
or a REPL command; it must not drift between a hidden Mittens builtin and a
REPL special case.

## Registry and construction boundary

The component registry consumes an evaluated, validated crate-owned component
tree. It may:

- construct concrete engine components
- invoke the opaque construction/property bindings produced by the completed
  `RuntimeSpec` builder
- create child topology
- register, attach, and initialize live subtrees
- use render assets required by construction

`component_registry::expression_to_value` must either serialize an already
ground AST without evaluation or be replaced by crate materialization. It must
not remain an alternate evaluator for names, calls, fields, indexes, or other
MMS expressions.

## Generic runner and REPL

`meow-meow-script` owns a runner and REPL that work with any host. They are
downstream of runtime configuration:

```text
RuntimeSpec ──► Runtime ──► SessionClient
                                  │
                         ┌────────┴────────┐
                         ▼                 ▼
                    generic Runner    generic REPL
                         │                 │
                         └──── HostService ┘
```

Their core constructors are `Runner::new(SessionClient)` and
`Repl::new(Runner)`. They accept an already-created session or runner and must
not accept, construct, inspect, or modify a `RuntimeSpec` or
`RuntimeSpecBuilder`.

The crate also provides configuration-independent convenience paths:

- `Runtime::standard()` creates the standard open-name runtime.
- `Runner::standard()` creates a standard runtime/session driven by
  `StandardHost`.
- `Repl::standard()` wraps that runner.

Custom hosts remain usable with `Runtime::standard()` and do not need a
builder. These convenience constructors do not change the core constructor
boundary.

### Runner boundary

The generic runner owns:

- source, module, export, callback, reset, and shutdown operations
- operation/host-call correlation
- timeout, cancellation, completion, and typed error handling
- blocking, polling, and async driving over the same session protocol
- crate-owned output events and result DTOs

For each host request, it calls a supplied `HostService` and returns the
correlated response to the session. It never mentions `World`, `RxWorld`,
render assets, `ComponentId`, `IntentValue`, or another engine type.

Component output is represented by crate-owned commands for emit, register,
and attach. A `ComponentSink` may be exposed as a convenience sub-interface or
adapter over those `HostRequest` variants. The crate should provide collecting
and rejecting sinks. `MittensHost` provides the ECS-backed implementation.
This sink is behavior, not a second runtime specification.

`StandardHost` is the crate-owned default implementation. It:

- collects emitted roots into a component forest
- allocates opaque local handles for registered nodes
- records local attachment while preserving child order
- reflects locally collected or registered components
- loads filesystem sources and returns canonical source identities

Operations that inherently require an engine, including ECS queries and
registered engine methods/APIs, return typed unsupported errors. `StandardHost`
does not silently emulate Mittens.

`run_file` and module-file entry points canonicalize the root file before
evaluation. That identity is used for nested relative imports, diagnostics,
and the module cache. Raw-source entry points may accept an explicit
`SourceId`; without one, a relative import fails deterministically with a typed
source-resolution error rather than resolving against the process working
directory.

### REPL boundary

The generic REPL wraps the same `Runner`/`SessionClient`. It owns:

- input classification and multiline completion
- persistent snippet evaluation
- cursor and breadcrumb semantics
- navigation over session-owned tables, arrays, and component artifacts
- pure value/component-artifact formatting
- reset and structured output events

The REPL is programmatic: callers submit input and receive `ReplEvent` values.
Stdin, stdout, ANSI clearing, GUI consoles, sockets, and engine frame loops are
adapters.

Live navigation uses a generic inspection protocol. Session-owned values stay
behind session `ValueRef`s and are inspected by the worker. World and
component targets use typed host requests for validation, listing, child
resolution, parent lookup, description, and optional MMS-source rendering.
The inspection protocol contains no component names, method signatures,
builtins, signals, or parser metadata and therefore is not a runtime
specification.

Unsupported live inspection must not prevent pure table, array, or component
artifact navigation. `StandardHost` supports inspection of its local component
forest; a custom host may optionally support inspection of its live handles.

The detailed dependency inventory and proposed interface shapes are in
[Generic runner and REPL boundary](../analysis/generic-runner-and-repl-boundary.md).

## Runner compatibility and evaluation modes

The engine-facing `MeowMeowRunner` entry points should remain source-compatible
while their internals move to the crate worker. This compatibility does not
permit a legacy evaluator fallback.

### Compatibility and versioning

For ordinary Mittens applications and MMS authors, this migration is intended
to be non-breaking:

- existing MMS source continues through the canonical evaluator
- established `MeowMeowRunner` method names, parameters, and return types stay
  available
- legacy engine module paths may re-export crate-owned DTOs
- template versus live behavior remains an explicit caller choice

This is not inherently non-breaking for every direct Rust consumer.
`meow-meow-script` embedders must migrate from the old runtime/session, host
capability, callback, and worker APIs to `RuntimeSpec` and the persistent
host-independent session protocol. That is a breaking public API release.

The current release pairs `meow-meow-script 0.9.0` with
`mittens-engine 0.9.0`. Under Cargo's pre-1.0 compatibility convention,
the minor version marks a breaking compatibility boundary. A jump to `1.0.0`
is warranted only if the project is also ready to promise a stable 1.x API;
it is not required merely to signal this break.

`mittens-engine 0.8.0` is reserved for the preceding avatar/editor reliability
release. The MMS ownership cutover therefore targets `mittens-engine 0.9.0`.
That is also an appropriate pre-1.0 compatibility boundary because the current
public fields of `LoadedMmsModule` and related surfaces expose legacy
export/heap representation that the target session architecture cannot expose
unchanged without violating the runtime boundary.

Before the 0.9 release, Mittens must choose one of these outcomes for each
supported surface:

1. preserve it through a boundary-safe compatibility wrapper; or
2. deliberately change it and document the 0.9 migration.

Changing observable error ordering, callback lifetime, module identity, or
template/live behavior can also be breaking even when Rust signatures compile.
Compatibility tests must cover behavior as well as source compilation.

#### Known Mittens compatibility hazards

The ordinary source runners are straightforward to preserve. The uncertain
surface is the public Rust representation around them:

| Current public Mittens surface | Conflict with the target boundary | Compatibility choices |
|---|---|---|
| `scripting::object::Value` | Public variants expose engine `ComponentId`, raw function/module bodies, heap IDs, and legacy maps | Preserve a deprecated facade with honest semantics, or change the type and treat it as breaking |
| `MaterializedCE` and `CeChild` public fields | They contain legacy `Value`, `RuntimeClosure`, and `CeChild::Attach(ComponentId)`; callback-bearing trees instead need opaque handles and a retained session | Keep a callback-free legacy template adapter, or change callback-capable APIs |
| `RuntimeClosure`, `HeapHandle`, `ObjectWorld`, `ObjectId`, and `Object` | Keeping these as functional engine-owned runtime state would violate the single MMS heap/session rule | A thin alias is possible only where the crate type has equivalent API; otherwise removal is breaking |
| `LoadedMmsModule` public fields and `named_export() -> &Value` | Named function exports and heap identity must remain inside a persistent crate session | Introduce an opaque/session-backed module facade; exact public-field compatibility may be impossible |
| `call_mms_module_fn(..., Option<&mut EvalChannels>, ...)` | Its signature directly exposes the protocol scheduled for deletion | Retain a deprecated adapter protocol, add a new API while keeping the old one, or make a breaking signature change |
| Public `world_evaluator` exports (`EvalRequest`, `EvalResponse`, `HostCallKind`, `HostValue`, `EvalChannels`, `MeowMeowEvaluator*`) | Their ring-buffer and engine-value representations are precisely the legacy architecture being removed | A full adapter is possible but costly and may preserve semantics the migration intends to forbid; otherwise removal is breaking |
| `KeyframeComponent::callback: Option<RuntimeClosure>` and `new_with_callback` | ECS must retain `(SessionHandle, CallbackHandle)`, not an AST closure and heap | Preserve a deprecated constructor through a conversion/registration facade, or change the field and constructor |
| Public `MittensHost` fields and raw `component_handle`/`component_id` conversions | The new host needs operation/session context, and raw conversion bypasses ownership validation | Keep safe constructors while deprecating raw conversions; public struct-literal and conversion compatibility may not be preservable |
| `IntentValue::SpawnComponentTree { root: Box<MaterializedCE>, ... }` | The public intent transitively exposes the legacy tree and callback representation | Preserve a legacy callback-free intent adapter or change the variant payload |
| `AssetModule`, `PaintAssetTemplate`, `PanelShellSpec`, `PanelLayoutMountSpec`, and asset accessors | These public engine types transitively expose `LoadedMmsModule`, `Value`, or `MaterializedCE` | Wrap the new session/tree types behind compatible containers where possible; field-level callers may still break |

Public enum variants and public struct fields matter even when most in-tree
callers do not use them: external crates may construct them, destructure them,
or exhaustively match them. Merely keeping the type name is not source
compatibility.

This audit should classify each item as one of:

- supported and preserved exactly
- supported through a deprecated, boundary-safe compatibility facade
- previously public but explicitly unstable/internal
- deliberately breaking

The 0.9 migration guide must classify every supported item and its observable
behavior, even when a boundary-safe compatibility facade is retained.

### Ordinary evaluation

- Standard standalone evaluation uses the same `Runtime` and crate evaluator
  with `StandardHost`; it collects component output while rejecting
  engine-only operations with typed unsupported errors.
- Live evaluation creates or selects a persistent session and services its host
  requests with short-lived `MittensHost` instances.
- Source-path entry points establish a canonical source identity so
  host-provided relative imports and module caching behave consistently. Raw
  source without an identity rejects relative imports.

### Modules and factories

Template and live factory modes remain explicit.

- **Template mode** returns a crate-owned component-tree artifact and performs
  no live ECS construction.
- **Live mode** evaluates the factory in a session with host access and returns
  or attaches a live component handle.

Both modes use the same module/session implementation and host protocol.
Template artifacts without callbacks may be detached snapshots. A template
artifact containing callbacks must retain its originating session so callback
captures and heap identity remain valid when the artifact is later spawned.

Exported calls in one loaded module session must observe shared heap/table
identity across calls.

### REPL

REPL snippets, callback calls, module export calls, and ordinary evaluations
are operations on the same session abstraction. REPL bindings, heap objects,
module cache, and current source/navigation context persist until reset or
shutdown.

The generic REPL and session-value navigation live in `meow-meow-script`.
Engine-specific tree traversal, source snapshots, frame polling, and terminal
ownership live in `MittensHost` or a Mittens REPL adapter. MMS expression
evaluation does not.

## Threading and reentrancy

MMS evaluation and heap mutation occur on the crate worker. `World`, `RxWorld`,
render assets, component registries, and intent sinks remain on the main
thread.

The main-thread runner drives an operation by repeatedly servicing correlated
host requests until completion, error, or timeout. It must not hold an engine
borrow in persistent session state.

Host dispatch is not an evaluator callback: it performs one
specification-bound engine operation and returns a DTO. A host implementation
must not re-enter the same session synchronously. Work caused by signals
during dispatch is queued as a later callback operation.

The transport should block or wake efficiently. The legacy ring-buffer
spin/yield protocol is not part of the public or target architecture.

## Migration constraints

During migration:

- `src/scripting/world_evaluator.rs` is frozen; no new language semantics may
  be added there
- parity harnesses may run both evaluators in tests
- production code must not select an evaluator per syntax feature
- legacy public paths may re-export crate-owned DTOs temporarily
- each caller category must pass through the crate worker before its legacy
  implementation is deleted

The gated order is: pure parity, single-spec builder and host completion, ordinary
runners, modules/factories, callbacks/keyframes, REPL/worker, then legacy
deletion.

## Conformance

The boundary is complete only when:

- `src/scripting/world_evaluator.rs` and its ring-buffer protocol are deleted
- the crate owns the only evaluator, runtime `Value`, heap, module state, and
  closure model
- one `RuntimeSpec` contains all configured vocabulary and every host-effectful
  item has exactly one builder-bound engine implementation
- all component operations reject foreign and stale handles
- no engine helper evaluates an arbitrary MMS expression
- handlers, keyframes, and callback-bearing templates preserve session heap
  identity after initial evaluation returns
- all examples and engine-facing runners use the crate worker/session path
- the generic crate runner and REPL work with `StandardHost` and fake/custom
  hosts, without Mittens types or a configuration builder
- open runtimes accept arbitrary uppercase component trees, while Mittens'
  strict runtime rejects unknown names
- static, collected, attached, live, stale, and foreign component values have
  the reflection behavior defined above
- table dot access and implicit-`self` methods preserve heap identity
- the parity, specification, integration, lifetime, worker, factory-mode, and
  workspace test suites pass

## Related specifications

- [Host API](host-call-api.md)
- [MeowMeowRunner](script-runner.md)
- [`eval_with_world`](eval-with-world.md)
- [Environment, heap, and object world](env-heap-object-world.md)
- [Module imports and exports](module-import-export.md)
- [Generic runner and REPL boundary](../analysis/generic-runner-and-repl-boundary.md)
- [Standalone runner and source loading](../../task/mms-standalone-runner-and-source-loading.md)
- [Component reflection and table dot access](../../task/mms-component-reflection-and-table-dot-access.md)
- [Generic MMS REPL migration and navigation](../../task/mms-repl-navigation-and-cat-unification.md)
