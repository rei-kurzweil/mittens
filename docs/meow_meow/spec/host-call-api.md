# ૮ ˙Ⱉ˙ ა Host API

Status: target architecture

The host API is the typed, host-neutral effect boundary owned by
`meow-meow-script`. Its ownership and lifetime rules are normative in
[Mittens host and MMS runtime boundary](mittens-host-and-runtime-boundary.md).

No `World`, `RxWorld`, `ComponentId`, render-asset handle, intent, or other
engine type crosses this boundary.

## Two layers

The API has two related layers:

1. `HostRequest` and `HostResponse` describe one engine operation.
2. The worker protocol correlates that operation with the persistent MMS
   session operation that requested it.

A direct embedding may implement the synchronous host trait:

```rust
pub trait Host {
    fn dispatch_with_context(
        &mut self,
        context: &mut HostContext,
        request: HostRequest,
    ) -> Result<HostResponse, HostError>;
}
```

The host does not advertise a second `HostCapabilities` schema. Its operation
bindings are attached while Mittens builds the one `RuntimeSpec`; construction
fails unless every host-effectful declaration has exactly one binding.

A **host-effectful declaration** is a `RuntimeSpec` vocabulary declaration
whose implementation crosses the MMS host boundary. This distinguishes it
from pure crate-implemented declarations and from deferred callback execution,
which may evaluate multiple pure and host-effectful calls over its lifetime.

Mittens uses the worker form because its evaluator state persists off the main
thread while `World` and related services must remain on it:

```text
worker                                      main thread
──────                                      ───────────
HostRequest {
  operation_id,
  request_id,
  request,
} ────────────────────────────────────────► short-lived MittensHost::dispatch

HostResponse {
  operation_id,
  request_id,
  result,
} ◄──────────────────────────────────────── same session operation resumes
```

Queue/channel selection is an embedding detail. The correlation fields and
validation behavior are not.

## Correlation rules

- Operation IDs identify source evaluations, export calls, callback calls, or
  REPL operations.
- Request IDs identify host calls within an operation.
- A response must echo both IDs.
- The worker rejects stale, duplicate, unknown, or mismatched responses.
- A completed, timed-out, reset, or cancelled operation cannot later consume a
  response.
- Every accepted host request produces exactly one response, including
  failures, so evaluation cannot deadlock waiting for an error path.

The legacy engine-local `EvalRequest`/`EvalResponse` ring buffer is superseded
by this crate-owned protocol and is not a compatibility surface.

## Values and identities

Requests and responses contain only crate-owned DTOs.

- Scalar transport values are owned.
- Tables or arrays crossing as transport values are snapshots unless the
  operation explicitly returns a session-owned value.
- `ComponentHandle` is an opaque identity for a host-owned ECS object.
- `CallbackHandle` is an opaque identity for a closure retained by the MMS
  session.
- `MaterializedCE` is an evaluated component-tree DTO, not an AST evaluation
  request.

A raw closure/function `Value` must not be sent to or stored by the engine.
Handler registration carries an opaque callback reference associated with its
originating session.

## Required request families

Exact Rust variant names may evolve, but the public protocol must represent
these operations without engine types.

| Family | Request data | Response |
|---|---|---|
| Source loading | importer identity, import specifier | resolved identity and source text |
| Component construction | evaluated tree, mode | root component handle |
| Registration | evaluated tree | detached root handle |
| Attachment | optional parent and child handles | unit |
| Query | selector, optional scope, cardinality | component handle(s) |
| Component method | handle, specification operation ID, arguments | value or unit |
| Handler registration | scope, specification signal ID, optional name, callback reference | unit |
| Engine API | specification operation ID, arguments | transport value or unit |
| Audio | specification operation ID and typed arguments | value or unit |

## Mittens `JSON` namespace

`JSON` is a Mittens host API namespace, not an evaluator-provided MMS built-in
table. It is declared in Mittens' `RuntimeSpec`, so another MMS embedding does
not expose it unless that host registers its own namespace and implementations.

| MMS call | Result | Error condition |
|---|---|---|
| `JSON.parse(text)` | MMS null, bool, number, string, array, or heap-backed table | invalid JSON or a number MMS cannot represent |
| `JSON.stringify(value)` | JSON string | non-finite number, function, component handle, cycle, or another non-JSON value |

Mittens implements the conversion with its JSON dependency at the host
boundary. Parsed JSON objects are rehydrated into the receiving MMS session's
heap, so their fields can be reassigned and captured like ordinary MMS table
literals. Transport values remain snapshots while crossing the boundary.
| Engine mutation | specification operation ID, targets, arguments | value or unit |
| Component inspection | handle and type/children/field operation | string, ordered handles, value, or missing-field marker |
| REPL inspection | world/component target and navigation operation | structured entries, target, description, or rendered source |

Pure evaluation uses `Hostless`, which returns a typed
`UnsupportedHostOperation` error for every host-effectful request.

The crate-owned `StandardHost` is distinct from a rejecting `Hostless`
implementation. It services component collection, opaque local handles, local
component inspection, attachment, and filesystem source loading. It returns
typed unsupported errors for operations that require an engine.

### Component sink adapter

Component construction requests form a useful generic sub-boundary. The crate
may expose a `ComponentSink` adapter for hosts that only need to accept emit,
register, and attach commands.

The adapter consumes crate-owned component trees and returns opaque component
handles or unit. A collecting sink stores emitted artifacts; a rejecting sink
provides pure evaluation; `MittensHost` constructs ECS trees. This adapter is
not a capability schema or runtime specification.

### REPL inspection

The crate's generic REPL navigates session-owned values inside the worker.
Live world/component navigation uses host inspection requests for:

- validation
- listing
- child resolution
- parent lookup
- descriptions/labels
- optional rendering or snapshotting as MMS source

Targets are `World`, an opaque session `ValueRef`, or an opaque
`ComponentHandle`; session-value requests do not cross to the host. Inspection
responses are structured DTOs rather than terminal-formatted lines.

This protocol does not declare MMS vocabulary and requires no
`RuntimeSpecBuilder`. A host may reject live inspection while the REPL
continues to support pure table, array, and component-artifact navigation.

## Host API namespaces

### Named data events in Mittens

Mittens registers `emit_data(scope, name, payload?)` as a global RuntimeSpec
host API. `scope` is a live component handle, `name` is a string, and `payload`
is an optional live component handle (omitted or `null` means no payload).
The call returns unit/null and queues a named `DataEvent` at that scope.
Wrong argument counts/types and stale component references are rejected.

Delivery uses the normal Rx and retained MMS callback boundary; host dispatch
does not invoke handlers synchronously. MMS `DataEvent` callbacks receive the
event name string under the existing callback convention. The optional component
payload remains on the engine event. Generic accordion assets use this API to
announce minimize and restore requests to their owning panels.

### Service namespaces

Hosts advertise third-party formats and local services through `RuntimeSpec`
API namespaces. Mittens provides `File.read_text(path: string) -> string` and
`JSON.parse` / `JSON.stringify`. Declaring `File` as a namespace also reserves
its uppercase spelling from component-expression parsing, so
`File.read_text("data.json")` is a host API call rather than a `File`
component expression.

Mittens exposes `GraphicsSettings.mirrors` as a nested host namespace table.
`toggle(bool)` sets the scene-wide capture-enabled state and returns unit/null;
`enabled()` returns the authoritative bool. Both use a live engine host lease
to `GraphicsSettingsSystem` and work without an editor or a panel. Setter
application is synchronous with dispatch; change notifications are queued
`GraphicsSettingsChanged` data events.

`GraphicsSettings.mirrors.quality(i32)` sets an optional scene-wide resolution
preference, clamped to 64..=2048, without rewriting authored mirror qualities.
`quality_override()` returns the applied integer or null when authored qualities
apply; `reset_quality()` clears the override. Changes while captures are paused
take effect on resume, with no capture allocation while off. Actual image
dimensions also depend on mirror aspect ratio.

`GraphicsSettings.secondary_motion.toggle(bool)` and `enabled()` use the same
authoritative owner and notification path. Off restores imported joint rotations
once and stops simulation/constraints/debug visualization; ordinary avatar
animation, tracking, and skinning continue. Resume resets simulation from the
live pose, preserving authored per-chain enabled flags. This takes effect at
the next post-pose stage. Scene replacement/session close reset captures and
secondary motion to enabled and clear the quality override.

### Universal component inspection

Language-level `node.type()`, `node.children()`, and `node.field` require no
host request for a static `ComponentExpr`: the session reads its own artifact.
For a live `ComponentObject`, the session issues generic inspection requests:

- read the component type name
- list immediate component children in host order
- read one authored named field

A confirmed missing field returns the protocol's missing-field result, which
the evaluator maps to `null`. Lack of inspection support is
`UnsupportedHostOperation`. Handle ownership and generation are checked first,
so `ForeignHandle` and `StaleHandle` remain distinguishable and are never
collapsed into missing or unsupported.

These inspection request shapes are universal protocol operations, not
registered component methods. `type()` and `children()` are reserved only for
component receivers; namesakes in authored component fields or tables do not
create host calls.

## Source loading

Import syntax and module evaluation belong to the crate. Access to
engine-relative files, assets, or URIs belongs to the host.

```rust
LoadSource {
    importer: Option<SourceId>,
    specifier: String,
}

SourceLoaded {
    resolved: SourceId,
    source: String,
}
```

`SourceId` is stable and host-neutral. The crate uses the resolved identity for
diagnostics, relative resolution of nested imports, and module caching.
Mittens may resolve that identity using a filesystem, asset database, or other
engine policy.

`StandardHost` canonicalizes filesystem paths before returning `SourceId`.
File entrypoints establish the initial canonical identity. A relative
specifier with no importer identity is a typed source-resolution failure; it
is never resolved against an ambient working directory. Canonical identities,
not spelling variants of paths, key the per-session module cache.

## Component lifecycle

### Register

Registration constructs a detached, uninitialized component subtree and
returns its root handle. The host:

- uses the construction/property bindings produced by the one `RuntimeSpec`
  builder
- applies already evaluated constructors, properties, and positionals
- constructs children without exposing ECS identifiers
- does not attach the root to a parent
- does not perform the final initialization walk

This gives MMS a live handle for method calls and queries before the subtree is
emitted.

### Attach

Attachment validates the parent and child handles, optionally adds the child
to the parent, and initializes the newly rooted subtree. A missing parent means
top-level attachment.

### Spawn/emit

A combined spawn/emit operation may construct and initialize a fresh tree in
one request. Existing component references embedded as children are opaque
handles and must be validated before splicing.

Re-emitting an already attached component remains governed by the component
emission policy; it is not permission to skip handle validation.

## Handle validation

Before every operation using a component, `MittensHost` checks:

1. ownership by the requesting session; and
2. that the complete generational ECS key still identifies a live component.

The first failure is `ForeignHandle`; the second is `StaleHandle`. Handle
conversion must preserve all generation bits.

## Specification-bound dispatch

Mittens constructs one crate-owned `RuntimeSpec` with the nested builder
described in
[Mittens host and MMS runtime boundary](mittens-host-and-runtime-boundary.md).
The same builder calls attach opaque host implementation bindings to every
host-effectful declaration. `Runtime` consumes the specification, while
`MittensHost` consumes those implementation bindings.

The binding table is not another specification: it contains no names,
signatures, aliases, signal schemas, or parser metadata.

The host must not:

- maintain an independent capability or vocabulary list
- use a blanket unsupported `CallApi` branch for registered APIs
- silently return success for unsupported REPL requests
- interpret AST expressions in the component registry

A request uses the opaque operation ID assigned when the single specification
was built. A name unknown to the configured runtime fails during validation
and must not become a host request. An operation whose current short-lived
host lacks a required service is unavailable in that context. A bound
dispatch function that fails returns a host failure with the operation and
engine cause preserved.

## Host lifetime

`MittensHost` borrows main-thread engine state only while servicing requests
for one runner operation. The MMS session does not own the host.

Host dispatch performs one specification-bound operation. It must not synchronously
re-enter the same session. Signals produced during dispatch enqueue callback
operations for later processing.

## Errors

At minimum, callers can distinguish:

- unsupported host operation
- unavailable host context
- invalid request
- foreign component handle
- stale component handle
- value/DTO conversion failure
- source resolution/loading failure
- host/engine failure
- protocol/correlation failure

Errors are returned as the correlated response and complete or fail the
originating MMS operation according to the worker protocol. They are not
encoded as `Null`, logged-only side effects, or no-op success.

## Related specifications

- [Mittens host and MMS runtime boundary](mittens-host-and-runtime-boundary.md)
- [`eval_with_world`](eval-with-world.md)
- [Environment, heap, and object world](env-heap-object-world.md)
- [Module imports and exports](module-import-export.md)
- [Generic runner and REPL boundary](../analysis/generic-runner-and-repl-boundary.md)
