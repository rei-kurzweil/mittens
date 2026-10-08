# meow-meow-script 0.9.0

The host-neutral Meow Meow Script language crate. It owns syntax, parsing,
runtime values, evaluation, and the synchronous host protocol. Engine-specific
component construction is provided by `mittens-engine`.

## Documentation

- [Documentation index](docs/README.md)
- [MMS language guide](docs/guide/language.md)
- [Configure a script host](docs/how_to/configuring_a_script_host.md)
- [Use the standard host](docs/how_to/use_the_standard_host.md)

## Mittens integration

`meow-meow-script` does not know about Mittens components. It provides the
language runtime, typed catalog declarations, stateful sessions, host-boundary
DTOs, callbacks, and the generic `Host` request/response contract.

`mittens-engine` embeds that runtime by registering the Mittens catalog: engine
component names, aliases, constructors, builder calls, properties, component
methods, and host APIs. Its Mittens host maps script component handles to ECS
components and implements the actual effects for emission, registration,
attachment, queries, methods, signals, and callbacks.

This lets other hosts reuse `meow-meow-script` with their own component
catalogs and semantics, while Mittens keeps its engine-specific behavior in the
engine crate.

## Runtime specifications

The crate owns `RuntimeSpec` and its nested builder. Embedders can register the
script surface they want to expose:

- `ComponentSpec` declares the canonical component name, aliases, constructors,
  builder calls, named properties, positional values, instance methods, and
  optional normalize/validate callbacks.
- `HostApiSpec` declares free functions or namespace methods such as
  `telemetry.record(...)`.
- Language builtins and host APIs are validated as parts of that one
  specification.

Each specification chooses a component-name policy. `OpenUppercase` accepts
registered names plus unregistered ASCII `[A-Z][A-Za-z0-9_]*` structural
labels. `StrictRegistered` accepts only registered names and aliases; Mittens
uses this policy.

Runtime signatures expose fixed-width numeric types: `i8`, `i16`, `i32`,
`i64`, `u8`, `u16`, `u32`, `u64`, `f32`, and `f64`. New `RuntimeSpec`s should
use those types so the host boundary records its real contract. `Number`
remains as a compatibility type for the preliminary flat builder API.

The evaluator currently stores authored numeric values as `f64`. Until the
runtime value representation preserves width and signedness, fixed-width
integer signatures enforce integrality and range at the call boundary, and an
`f32` signature rejects finite values outside the `f32` range. This is
contextual boundary validation, not yet source-level numeric inference.

`Runtime::standard()` provides the crate-owned `OpenUppercase` runtime without
a builder. Unknown open components are unvalidated structural data, suitable
for standalone trees and tools.

## Sessions and hosts

A runtime creates a host-independent session. A `Session` owns its lexical
scopes, heap-backed table objects, modules, callbacks, and opaque handles
across repeated operations. `Runner::new(SessionClient)` and
`Repl::new(Runner)` are the configuration-independent core APIs.

The host boundary is the `Host` trait. New hosts usually implement
`dispatch_with_context(...)`, receive `HostRequest` values, and return
`HostResponse` values while an operation is driven. Component handles identify
host-owned resources; callback handles identify MMS-owned closures.

Component expressions can also be parsed and materialized without attaching a
host by using `Runtime::materialize_component(...)`. A host is only required for
effects such as emit/register, query, component methods, and host APIs.

`Runner::standard()` uses the crate-owned `StandardHost`. It collects emitted
roots into a component forest, allocates opaque local handles, and resolves
local attachment topology. Engine-only operations return typed unsupported
errors. Custom hosts can use `Runtime::standard()` without a builder.

The programmatic REPL, component reflection methods, and filesystem source
loading remain planned work.

Tables are heap-backed inside MMS, so aliases observe mutation across
evaluations. When a table crosses into a host API, it is converted into an owned
`TransportValue::Table` snapshot. Cycles or non-transferable values fail with a
typed conversion error.

Table dot reads match index reads, and function-valued dot calls receive the
table as implicit `self`. Component expressions and live component objects
share `type()`, ordered `children()`, and authored-field reflection.

## Example hosts

The crate includes two generic hosts:

- `EventStreamHost` records ordered in-memory events suitable for forwarding to
  a socket, broker, or test harness.
- `JsonLinesHost` records the same events as JSON-lines.

Run them with:

```sh
cargo run -p meow-meow-script --example standard_runtime
cargo run -p meow-meow-script --example event_stream_host
cargo run -p meow-meow-script --example json_lines_host
```

`standard_runtime` smoke-tests the builder-free open-name runtime and
crate-owned collecting host. The event-stream examples demonstrate custom
runtime specifications and hosts. They intentionally do not provide a socket
implementation or standalone CLI. See the crate-local
[standard-host guide](docs/how_to/use_the_standard_host.md) for current
capabilities and limitations.
