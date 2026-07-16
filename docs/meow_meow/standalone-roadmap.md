# Meow Meow standalone roadmap

## 1. Delivered by the workspace split

`meow-meow-script` now owns the tokenizer, parser, AST, lowering/transforms,
runtime values, pure evaluator, unparser, opaque component handles, and a
synchronous host contract. `HostlessRunner` evaluates ordinary language code
without linking Mittens. Engine-only expressions fail with the typed
`UnsupportedHostOperation` host error rather than panicking.

A minimal custom host implements one method:

```rust
use meow_meow_script::{Host, HostError, HostRequest, HostResponse, eval_with_host};

struct MyHost;

impl Host for MyHost {
    fn dispatch(&mut self, request: HostRequest) -> Result<HostResponse, HostError> {
        match request {
            HostRequest::ReplHelp => Ok(HostResponse::Unit),
            request => Err(HostError::unsupported(request.operation_name())),
        }
    }
}

let result = eval_with_host("1 + 2", &mut MyHost)?;
# Ok::<(), meow_meow_script::EvalError>(())
```

Mittens provides `mittens_engine::scripting::MittensHost`, which translates
script DTOs into ECS queries, component construction, methods, handlers,
audio operations, and mutations.

## 2. Standard standalone host

Add an opt-in standard host with explicit capabilities for filesystem imports,
stdout/stderr, process arguments, and time. Networking remains optional and is
disabled by default. Filesystem access should be rooted and capability-checked;
time should offer a deterministic test clock.

## 3. `meow-meow` CLI

Add a CLI target that supports script paths, `-e`, and an interactive REPL.
Define deterministic exit codes for parse, runtime, host-capability, and I/O
failures. Mittens-only component expressions and methods must produce clear
capability errors that name the unavailable operation.

The CLI and standard host are follow-up work and are not shipped by this split.

## 4. Stable extension APIs and compatibility

Stabilize custom builtin and component registration APIs without exposing host
implementation types. Add integration examples for embedding, capability
selection, custom builtins, and custom component DTO handling. Maintain a
compatibility suite that runs every pure script under both the standalone host
and `MittensHost` and compares values, output, errors, and deterministic side
effects.
