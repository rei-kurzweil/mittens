# meow-meow-script

The host-neutral Meow Meow Script language crate. It owns syntax, parsing,
runtime values, evaluation, and the synchronous host protocol. Engine-specific
component construction is provided by `mittens-engine`.

## Configurable runtime

Embedders create a `Runtime` with `Runtime::builder()` and register the script
surface they want to expose:

- `ComponentSpec` declares the canonical component name, aliases, constructors,
  builder calls, named properties, positional values, instance methods, and
  optional normalize/validate callbacks.
- `HostApiSpec` declares free functions or namespace methods such as
  `telemetry.record(...)`.
- Language builtins are tracked separately from host APIs so component names,
  namespaces, and builtins cannot collide.

Parsing is catalog-driven in a configured session. Registered component names
are matched case-insensitively, aliases are canonicalized before host dispatch,
and unknown constructors, methods, properties, APIs, or conflicting catalog
entries produce typed diagnostics with known-name suggestions.

## Sessions and hosts

`Runtime::session(host)` checks the host capabilities before evaluating any
script. A `Session` then owns its lexical scopes, heap-backed table objects,
and callback registry across repeated `eval(...)` calls. Hosts are selected per
session and are not hot-swapped.

The host boundary is the `Host` trait. New hosts usually implement
`dispatch_with_context(...)`, receive `HostRequest` values, and return
`HostResponse` values. Component handles identify host-owned resources, so a
host may return handles derived from native IDs; `HostContext` can allocate
synthetic component handles for simple logging hosts. Callback handles identify
MMS-owned closures and are allocated by the runtime.

Component expressions can also be parsed and materialized without attaching a
host by using `Runtime::materialize_component(...)`. A host is only required for
effects such as emit/register, query, component methods, and host APIs.

Tables are heap-backed inside MMS, so aliases observe mutation across
evaluations. When a table crosses into a host API, it is converted into an owned
`TransportValue::Table` snapshot. Cycles or non-transferable values fail with a
typed conversion error.

## Example hosts

The crate includes two generic hosts:

- `EventStreamHost` records ordered in-memory events suitable for forwarding to
  a socket, broker, or test harness.
- `JsonLinesHost` records the same events as JSON-lines.

Run them with:

```sh
cargo run -p meow-meow-script --example event_stream_host
cargo run -p meow-meow-script --example json_lines_host
```

These examples intentionally do not provide a socket implementation, filesystem
host, or standalone CLI. A future standalone REPL should be built on
`Runtime` plus persistent `Session`.
