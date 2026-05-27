# ᓚᘏᗢ MMS Function Dispatch

How a function call `foo(args)` is resolved and executed — and what a transpiler must do at each dispatch kind.

---

## Dispatch kinds

There are four distinct ways a call in MMS can be fulfilled:

| Kind | Who executes | Transpiler action |
|---|---|---|
| **MMS closure** | Evaluator thread, by walking the AST body | Recurse into body — static code generation, no boundary |
| **Evaluator builtin** | Evaluator thread, special-cased in `eval_call` | Emit the per-target equivalent in-place — no boundary |
| **Stdlib (MMS)** | Evaluator thread, same as closure (body is MMS) | Same as closure — body is in the AST |
| **Host call** | The script's host, via whatever boundary the target requires | Emit a per-target boundary crossing (direct call / await / IPC / network) |

The critical distinction is between **kinds 1–3** and **kind 4**:

- Kinds 1–3 involve **no runtime boundary**. The transpiler has the full source (kinds 1 and 3) or a fixed code pattern (kind 2). Transpilation is purely static code generation — the output is self-contained and runs wherever it's deployed with no round-trip to anything.
- Kind 4 involves a **runtime boundary crossing**. The script needs something that only the host can provide at runtime. How that crossing happens depends entirely on the deployment target.

---

## 1 — MMS closure

A function defined with `fn` in user code. After evaluation it becomes a
`Value::Function { params, body, captured_env }` and is stored in the env like any other value.

```mms
fn lerp(a, b, t) { return a + (b - a) * t }
let x = lerp(0.0, 1.0, 0.5)
```

**Evaluator:** `eval_call` recognises `Value::Function`, binds args into `call_env`, runs
`eval_block_stmts` on the body. Closures capture the env at definition time
(`captured_env: env.clone()`).

**Transpiler:** sees the full `BlockStatement` AST. Recurses into it and emits the equivalent
function in the target language. No special casing, no per-target table lookup — it is just
MMS code. The output is entirely self-contained.

---

## 2 — Evaluator builtin

A small fixed set of names intercepted before env lookup in `eval_call`. Currently `range`
and `emit`:

```rust
// evaluator.rs — eval_call
if call.callee.0 == "range" { ... return Ok(Value::Array(...)); }
```

These are hardcoded in Rust. They have no MMS body and do not exist in the env.

**Evaluator:** handled inline by a match arm in `eval_call` or `eval_expr_stmt`. Runs
synchronously on the evaluator thread. No env lookup, no ring buffer.

**Transpiler:** must recognise the callee name and emit the in-place equivalent for the
target. Like kind 1, there is no runtime boundary crossing — the emitted code runs inline
wherever it appears.

| Builtin | Rust | JavaScript |
|---|---|---|
| `range(n)` | `(0..n as usize).map(|i| i as f64)` | `Array.from({length: n}, (_, i) => i)` |
| `emit(ce)` | `scene.push(ce)` or similar | depends on target scene model |

The transpiler carries a dispatch table mapping builtin names to per-target code generators.
If a target has no equivalent, it's a compile error.

---

## 3 — Stdlib function (MMS)

A function exported from a stdlib module (`"math"`, `"easing"`, `"color"`, etc.). After
import it is just a `Value::Function` in the env — indistinguishable from a user closure
at eval time.

```mms
import { lerp } from "math"
let x = lerp(0.0, 1.0, 0.5)
```

**Evaluator:** no special casing. Runs the MMS body like any other closure. No round-trip
to the main thread — stdlib runs entirely on the evaluator thread.

**Transpiler:** the stdlib body is MMS AST, so the transpiler processes it identically to
user code. It may inline the body, emit a named helper function, or replace known stdlib
functions with optimised target-language intrinsics (e.g. `lerp` → a SIMD intrinsic in
Rust). The key point: the full source is available for any of these strategies.

> This is why stdlib must be written in MMS rather than Rust. A Rust-native stdlib function
> has no AST body — the transpiler would have to special-case it by name anyway, giving up
> composability and inspectability.

---

## 4 — Host call (future)

A call that needs data or behaviour that only the **script host** can provide at runtime:
the current world position of a component, the current audio level, elapsed time, a random
number from a PRNG seeded by the host, a physics query result, etc.

MMS cannot compute these itself. There is no MMS source body to transpile. The script must
call out, wait for a reply, and continue.

This dispatch kind does **not exist yet**. See current state below.

---

### What is "the host"?

The host is whatever embeds and runs the MMS runtime. This is the same sense as in
WebAssembly — the WASM module declares what it needs (imports), and the host satisfies
those imports with concrete implementations.

The host changes depending on the deployment target:

| Deployment | Host | Calling convention |
|---|---|---|
| cat-engine, evaluator thread | main thread via ring buffer | `EvalResponse::Query` → spin-wait on `EvalRequest::QueryResult` |
| cat-engine, transpiled Rust (same thread) | engine API directly | direct function call |
| cat-engine, transpiled async Rust | engine async API | `await engine.world_position(id)` |
| JavaScript / WASM, client in browser | browser runtime + server | `await` Promise, or WebSocket round-trip |
| Multiplayer game server | server-side simulation | local function call into simulation state |
| Offline / baked scene | no host | compile error — live queries have no meaning |

The script does not care which host it is running in. From MMS's perspective, `world_position(id)` is just a function call. The host contract is an **interface**: a set of named functions with defined signatures that any conforming host must provide.

This is an RPC pattern in the general sense — "remote" meaning "across a boundary" (thread,
process, or network), not necessarily a network call.

---

### Host interface contract

Host functions are declared as an interface — the set of functions that must be provided by
any host that wants to run this MMS script. Each function has a name, a signature, and a
per-target binding strategy.

```mms
// hypothetical — not yet implemented
import { world_position, elapsed_time, rand } from "host"

let pos   = world_position(my_component_id)   // -> Vec3
let t     = elapsed_time()                     // -> Num (seconds)
let noise = rand()                             // -> Num in [0, 1)
```

The `"host"` module is special — it's not a file on disk. It's backed by the runtime's
host binding table, resolved differently per target. Importing from `"host"` signals to the
transpiler that this call crosses a boundary.

A host function registration (conceptual Rust):

```rust
HostFn {
    name: "world_position",
    signature: Fn(ComponentId) -> Vec3,
    bindings: {
        Target::CatEngineThread  => ring_buffer_query(QueryKind::WorldPosition),
        Target::CatEngineDirect  => |id| engine.world().world_position(id),
        Target::AsyncRust        => |id| async { engine.world_position(id).await },
        Target::JavaScript       => "await host.worldPosition(id)",
        Target::Baked            => CompileError("world_position not available at bake time"),
    }
}
```

---

### Current state in the evaluator

This dispatch kind does not exist yet. The thread protocol currently only flows one way
during evaluation: MMS → engine (via `EvalResponse::Intent`). There is no mechanism for
MMS to pause mid-evaluation and wait for a reply.

The signal system does have a `Sender<T>` reply pattern for engine-internal queries
(`QueryFindComponent { reply: Sender<Option<ComponentId>> }`), but that is Rust-to-Rust
only and is not reachable from the evaluator thread.

To implement evaluator-side host calls, the evaluator would need to:
1. Send `EvalResponse::Query { id, kind: QueryKind::WorldPosition(cid) }` on the ring buffer
2. Spin-wait on `EvalRequest` for `EvalRequest::QueryResult { id, value }`
3. Resume evaluation with the returned value

---

## Current dispatch flow in `eval_call`

```
eval_call(call, env, emits)
  │
  ├─ name == "range"?   → Evaluator builtin (hardcoded, no env lookup)
  │
  └─ env.get(name)?
       │
       ├─ Value::Function { body, captured_env }
       │    └─ eval_block_stmts(body, call_env)   ← MMS closure or stdlib function
       │
       └─ other value   → error: "cannot call X as a function"

eval_expr_stmt (statement position)
  │
  └─ name == "emit"?    → Evaluator builtin (intercepted before eval_call)
```

Host calls are not yet in this flow. When added, they would appear as a third branch in
`env.get(name)?` — a `Value::HostFn(HostFnKind)` that triggers the boundary-crossing path.

---

## Transpiler dispatch table (sketch)

Each dispatch kind maps to a different transpiler strategy:

```
DispatchKind::MmsBody(stmts)         → recurse into stmts, emit in target language
                                        (no boundary; purely static code generation)

DispatchKind::EvalBuiltin(name)      → look up in per-target builtin table, emit inline
                                        (no boundary; code pattern substitution)

DispatchKind::StdlibMms(stmts)       → same as MmsBody (optionally inline or intrinsify)
                                        (no boundary; stdlib body is full MMS AST)

DispatchKind::HostCall(host_fn)      → look up in per-target host binding table
                                        emit a boundary crossing appropriate for the target:
                                        direct call / await / ring buffer / network / error
```

The transpiler resolves `call.callee` to a dispatch kind by:
1. Checking the evaluator builtin name set first (`range`, `emit`)
2. Looking up the value in the type-annotated env:
   - `Value::Function` with an AST body → `MmsBody` or `StdlibMms` (origin tracked in env)
   - `Value::HostFn` → `HostCall`
3. The origin of the binding (which module it came from) determines kind 2 vs 3 vs 4:
   - `import { x } from "math"` → `StdlibMms`
   - `import { x } from "host"` → `HostCall`
   - user-defined `fn` → `MmsBody`

This resolution works as long as the type **and origin** of every binding is tracked through
the transpiler's env — which it must be for type inference anyway.

---

## Summary: static vs boundary-crossing

The simplest mental model:

- **Kinds 1, 2, 3** — the transpiler has everything it needs at compile time. Output is self-contained. No runtime dependencies beyond what the target language itself provides.
- **Kind 4 (HostCall)** — the transpiler emits a call that will cross a boundary at runtime. The shape of that boundary is a deployment decision. A script that imports from `"host"` is declaring a dependency on its runtime environment.

A script with no host calls is fully portable — it can be transpiled to any target and run without any engine. A script with host calls is environment-dependent — it can only run in a host that satisfies its declared host interface.
