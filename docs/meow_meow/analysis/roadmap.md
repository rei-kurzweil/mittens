# MMS development roadmap

Phases are ordered by dependency. Each phase has a checklist and flags open design
decisions (🔷) that need a doc or a decision before implementation.

---

## Phase 1: Core component expressions ✅ DONE

- [x] Tokenizer (`Let`, `If`, `Else`, `Return`, `True`, `False`, `Null`, literals, idents)
- [x] Parser: component expressions, `let` bindings, `if`/`else`, `return`, blocks
- [x] AST: `Statement`, `Expression`, `ComponentExpression`, `ComponentBodyItem`
- [x] `EmitLiftTransform` — bare CE in statement position → `emit(ce)` call
- [x] Evaluator thread + ring buffer protocol (`EvalRequest` / `EvalResponse`)
- [x] `StoredValue` — `let x = T { }` stores the `ComponentExpression`
- [x] `SpawnComponentTree` intent + `RxIntentExecutor` handler
- [x] Component registry — 30+ component types
- [x] `MeowMeowRunner` — synchronous intent collection helper
- [x] `vr-input-mms.rs` end-to-end scene spawn

**Known gaps from Phase 1 (carry-forward):**
- [ ] Emit context stack — function calls inside component bodies should emit as children,
      not world roots. Gated on Phase 6 (reply channel).
      → [emission-and-component-value-model.md](emission-and-component-value-model.md)
- [x] Narrow live reply channel exists via `eval_with_world`: `let x = T { }` can bind a
      live `ComponentObject` in that path.
- [ ] Default `eval` path still stores `ComponentExpression`, not a live handle.
- [ ] Returned / re-emitted `ComponentObject` values are not fully wired yet.

---

## Phase 2: Expression evaluation ✅ DONE

- [x] Arithmetic: `+`, `-`, `*`, `/`, `%`
- [x] Comparison: `==`, `!=`, `<`, `>`, `<=`, `>=`
- [x] Logical: `&&`, `||`, `!`
- [x] Unary minus: `-x`
- [x] `Expression::BinaryOp { op, lhs, rhs }`
- [x] `Expression::UnaryOp { op, operand }`
- [x] `eval_expr()` — recursive expression evaluator, returns a runtime `Value`
- [x] Arithmetic on `Value::Number`
- [x] Comparison returning `Value::Bool`
- [x] Logical ops on `Value::Bool`
- [x] Number type: everything is `f64`, cast at component registry boundary

**Design decisions resolved:**
- Numbers are `f64` throughout; cast to `f32`/`usize`/etc. at the registry boundary.
- Precedence is standard C-style (PEMDAS + logical).

---

## Phase 3: `if`/`else` evaluation ✅ DONE

- [x] `eval_stmt` arm for `Statement::If` — evaluate condition, pick branch
- [x] `eval_stmt` arm for `Statement::Block` — evaluate inner statements
- [x] `if` without `else` — condition false → `StmtEffect::None`
- [x] Reassignment inside if-branch propagates to enclosing scope (`&mut Env` threading,
      no clone — see `docs/meow_meow/spec/env-and-context.md`)
- [x] Tests: if true/false, if/else, if with let-bound condition variable, if+reassign propagation

**Remaining gaps in this area:**
- [ ] `else if` chaining — the parser produces a nested `else { if ... }` tree correctly
      but `else if` as explicit syntax isn't handled; chains of conditions require nesting
- [ ] `if` as an expression (`let x = if cond { a } else { b }`) — not parsed; `if` is
      statement-only

---

## Phase 4: Functions ✅ DONE

- [x] `fn` keyword token
- [x] `Statement::Function { name, params, body }` — named function syntax `fn foo(args) { }`
- [x] `Value::Function { params, body, captured_env }` — full closure with env capture
- [x] `eval_call()` — look up callee in env, bind args, eval body in isolated env
- [x] `EmitLiftTransform` recurses into function bodies
- [x] `return` statement evaluation — `StmtEffect::Return(value)` unwinds call frame
- [x] `export fn` — named function export in module context
- [x] Tests: define + call, return value, args, closure capture

**Design decisions resolved:**
- Named functions (`fn foo(args) { }`) are `Statement::Function`; `let f = fn ...` is not a
  separate form — `fn` is always statement-level.
- Full closures: `captured_env: env.clone()` at definition time.
- Let inside a function body leaks to the function scope (v1 flat scoping; see env-and-context.md).
- No return type annotations yet (Phase 10).

---

## Phase 5: `for`/`in`, arrays, `break`, `continue` ✅ DONE

- [x] `for`, `in`, `break`, `continue` keyword tokens
- [x] `Statement::ForIn { binding, iterable, body }`
- [x] `Statement::Break`, `Statement::Continue`
- [x] `StmtEffect::Break`, `StmtEffect::Continue`
- [x] `eval_stmt` for ForIn — persistent `loop_env` across iterations (accumulator pattern works)
- [x] `range(n)` builtin in `eval_call` — evaluator builtin, hardcoded, no env entry
- [x] `Expression::Array(Vec<Expression>)` + `Value::Array(Vec<Value>)`
- [x] Array literal parsing and evaluation
- [x] Tests: `for x in [1,2,3]`, `for i in range(10)`, emit in a loop, break, continue,
      accumulator pattern, nested for

**Remaining gaps in this area:**
- [ ] Array indexing: `arr[i]` — `Expression::Index` not in AST; subscript reads are not
      supported. Arrays exist but elements cannot be read back after creation.
- [ ] Array mutation: `arr[i] = v` — depends on array indexing above.

---

## Phase 6: Live `ComponentId` reply channel

**Current status:** partially implemented.

What exists now:

- [x] Bidirectional evaluator/host round-trip using `EvalResponse::HostCall` and
      `EvalRequest::HostCallResult`
- [x] `HostCallKind::Spawn(MaterializedCE)`
- [x] `HostValue::ComponentId(ComponentId)`
- [x] `MeowMeowRunner::eval_with_world(...)` host servicing path
- [x] `let x = T { }` upgrades to `Value::ComponentObject(ComponentId)` in that live path

Remaining gap:

- only spawn is supported
- the handle only carries `ComponentId`, not GUID
- emit context stack is still missing
- `ObjectWorld` is not yet the actual evaluator environment
- query/method HostCalls are still missing

**What's needed:**
- [x] Reply channel from main thread → evaluator thread
- [x] `SpawnComponentTree` response carries the assigned `ComponentId`
- [x] `Value::ComponentObject(ComponentId)` for the live path
- [ ] Upgrade the live handle to carry both `ComponentId` and GUID
- [ ] Emit context stack — body-scoped calls emit as children of the enclosing component

**Checklist:**
- [x] Design the reply channel (implemented via `HostCall` / `HostCallResult`)
- [x] Host servicing path echoes the root `ComponentId` back
- [x] Evaluator receives the ID and upgrades the stored value → `ComponentObject`
- [ ] Change the reply payload to a handle carrying both `ComponentId` and GUID
- [ ] Emit context stack implemented in evaluator

🔷 **Design decision: handle payload shape**
The reply channel exists, but the payload should grow from `ComponentId` to a live handle
carrying both `ComponentId` and GUID.
→ [../../task/mms-reply-channel-objectworld-and-mmq-status.md](../../task/mms-reply-channel-objectworld-and-mmq-status.md)

---

## Phase 7: `ComponentObject` mutation API

Depends on Phase 6 (live `ComponentId`).

**What's needed:**
- [ ] Method call on a `ComponentObject` value: `x.set_color(1, 0, 0, 1)`
- [ ] MMS-side dispatch: `ComponentObject` method name → `IntentValue`
- [ ] A registry of per-component-type mutation methods (parallel to the constructor registry)

**Checklist:**
- [ ] Parse `expr.method(args)` as `Expression::MethodCall` (new AST node, distinct from
      `ComponentBodyItem::Call` which is constructor-time)
- [ ] Evaluate `MethodCall` on `Value::ComponentObject` → emit the appropriate intent
- [ ] Tests: spawn, mutate, verify intent emitted

🔷 **Design decision: mutation method registry vs intent literals**
Should `x.set_color(...)` be a hard-coded dispatch table (like the constructor registry),
or should MMS be able to emit arbitrary `IntentValue`s directly (the `intent()` builtin
discussed in [signal-emission-in-mms.md](signal-emission-in-mms.md))?

---

## Phase 8: Mutable rebinding / `while` — PARTIALLY DONE

**Done (landed ahead of schedule, implemented differently than planned):**
- [x] Variable reassignment: `x = expr` in statement position (no `var` keyword needed)
      — parser lookahead on `Ident` followed by `=`; `Statement::Reassign { name, value }`
- [x] `StmtEffect::Reassign(name, value)` — applied by `eval_block_stmts`, errors if name
      not already in scope
- [x] Accumulator pattern in `for` loops works: `sum = sum + i` across iterations
- [x] Tests: basic reassign, undefined name errors, for-loop accumulator, if-branch propagation

**Remaining:**
- [ ] `while` loop — `Statement::While { condition, body }`, `While` token, parser + evaluator arm
- [ ] `loop { }` — can be `while true { }`, probably not needed

**Design decision resolved:** `var` keyword dropped; plain `x = expr` is rebinding.
Let-bindings inside blocks leak to the enclosing scope (v1 flat scoping — documented).

---

## Phase 9: Module / import system ✅ DONE

- [x] `import { name, name } from "path"` statement
- [x] `export let` / `export fn` — named exports
- [x] `Value::Module { named: HashMap, sequence: Vec }` runtime type
- [x] `eval_as_module()` — sandboxed eval; emits collected into `sequence`, not world queue
- [x] `StmtEffect::Export(name, val)` — applied by module evaluator, not script evaluator
- [x] `StmtEffect::ImportBindings(HashMap)` — import binds names into local env
- [x] Module resolution: relative path with `.mms` extension (v1 — 12-line `resolve_import_path`)
- [x] Circular import detection (basic)

**Module resolution v2 (designed, not yet implemented):**
- [ ] Bare name resolution: `"noise"` → check relative file first, then stdlib registry
- [ ] Extension inference: `"math"` → try `math`, `math.mms`, `math/mod.mms`
- [ ] Project search path (reserved slot in data model)
- [ ] Stdlib registry: `HashMap<&str, StdlibModule>` with embedded MMS source (`include_str!`)
- [ ] Sentinel paths for stdlib (`"<std:noise>"`) in error messages
- [ ] Stdlib modules: `noise`, `math`, `color`, `easing`, `random` (see catalogue below)
→ Design: [../draft/module-resolution.md](../draft/module-resolution.md)

---

## Phase 9b: Standard library (MMS) — DESIGNED, NOT YET WRITTEN

All stdlib must be written in MMS (not Rust) so the eventual transpiler can see full AST
bodies and emit optimised native code for each target. A Rust-native stdlib function would
be opaque to the transpiler.
→ Rationale: [../spec/function-dispatch.md](../spec/function-dispatch.md)

**Catalogue:**

| Module | Functions | Status |
|---|---|---|
| `"math"` | `sin`, `cos`, `tan`, `sqrt`, `abs`, `floor`, `ceil`, `pow`, `clamp`, `lerp`, `map` | Not written; trig/sqrt need native bindings |
| `"easing"` | `ease_in`, `ease_out`, `ease_in_out`, cubic/back/elastic variants | Not written; pure MMS, can be written now |
| `"color"` | `hsv(h,s,v)` → rgba array, `mix(a,b,t)`, `temperature(k)` | Not written; pure MMS, can be written now |
| `"noise"` | `simplex(x,y)`, `simplex3(x,y,z)`, `perlin(x,y)`, `worley(x,y)` | Needs native PRNG + C noise lib binding |
| `"random"` | `seed(n)`, `rand()`, `rand_range(lo,hi)` | Needs native binding (PRNG state) |

**Native binding mechanism not yet designed.** Functions that need OS/native primitives
(`sin`, `rand`, etc.) require a `native fn` declaration form or similar, so the evaluator
and transpiler both know to lower them to a platform call. Without this mechanism, only
pure-MMS modules (`easing`, most of `color`, `lerp`/`clamp`/`map` in `math`) can be
written now. Everything else is blocked.

Random values in particular would need an intent + reply channel (same as Phase 6) to
request a PRNG value from the main thread — or a native binding that calls into a
thread-local PRNG on the evaluator thread.

---

## Language gaps summary ᓚᘏᗢ

These are missing features identified across phases that aren't covered by any phase above.
Address them as needed when authoring real scenes hits the wall.

| Gap | Severity | Notes |
|---|---|---|
| Array indexing `arr[i]` | High | Arrays exist but are write-once. Blocks most data-structure use cases. |
| `else if` chaining | Medium | Parser nests `else { if ... }` correctly but no `else if` keyword pair. Workaround: nest manually. |
| Math builtins (`sin`, `cos`, `sqrt`, `abs`, `floor`) | High | Required for any geometry / animation math. Blocked on native binding mechanism. |
| Map / object literals `{ key: val }` | Medium | No map type in AST or evaluator. Needed for structured data. |
| `while` loop | Low | Reassignment is done; only the loop construct is missing. Can loop with `for i in range(n)`. |
| Method call syntax `foo.bar(args)` on values | Low | `CallExpression.callee` is always a bare `Ident`. Needed for Phase 7 mutation API. |
| String interpolation | Low | String concat with `+` works; no `f"..."` or `\{expr\}` syntax. |
| `else if` / match expression | Low | Multi-way branching requires nesting if/else. |

---

## Phase 10: Type annotations (Option D emission)

Enables the typed emission policy described in
[emission-policy-options.md](emission-policy-options.md). Allows the compiler to know
statically which functions return `ComponentObject` vs other values, enabling deterministic
emission without the runtime Option B check.

- [ ] Type annotation syntax on function params and return types
- [ ] `ComponentObject` as a named type in MMS
- [ ] Static analysis pass (or just a type-checking evaluator mode)
- [ ] Update `EmitLiftTransform` to use static return type info when available

### ⚑ Static call validation against component method definitions

The type system is also the right place to validate **call expressions against the method
signatures they target**. Currently, argument count and type errors in component constructor
calls and body method calls are caught at spawn time (main thread, `component_registry.rs`)
and surfaced as `[SpawnComponentTree] error:` log lines. This is late and requires the engine
to be running.

Once the type system can resolve the component type of a CE (i.e. knows that `T.position(...)`
refers to `TransformComponent`), a static analysis pass can consult a **method signature table**
(mapping component type → method name → `(arity, [ArgType])`) and report arity/type mismatches
as parse-time or pre-spawn errors. This gives the author immediate feedback without needing to
run the scene.

The method signature table should be the single source of truth shared by:
- The static validator (MMS compile step)
- The component registry (runtime fallback, for dynamic/unknown types)
- IDE tooling / autocomplete (future)

Until Phase 10 lands, argument count errors are caught at spawn time via `arg(args, i)?` in the
registry returning `Err(String)` rather than panicking.

---

## Dependency graph

```
Phase 1 (done)
    └── Phase 2 (done)
            ├── Phase 3 (done)
            │       └── Phase 4 (done)
            │               └── Phase 5 (done)
            │                       └── Phase 8 (reassign done; while todo)
            └── Phase 6 (reply channel)
                    └── Phase 7 (mutation API)

Phase 9 (modules — done; stdlib todo)
Phase 10 (types) — depends on Phase 4
```

**Current state:** Phases 1–5, 8 (partial), and 9 are done. The evaluator is a working
scripting runtime. Remaining work:

- **Unblock authoring:** array indexing, math builtins (needs native binding design)
- **Unblock live mutation:** Phase 6 reply channel → Phase 7 mutation API
- **Unblock transpilation:** Phase 10 types; native binding mechanism for stdlib
