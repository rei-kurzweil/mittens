# ᓚᘏᗢ Env and Evaluation Context

How variable environments, scoping, and evaluation context work in the MMS evaluator.

---

## The two orthogonal "contexts"

MMS evaluation threads two distinct kinds of state through the evaluator:

| Thing | Type | What it holds | Mutable? |
|---|---|---|---|
| **Variable env** | `ObjectWorld` frame stack | Names bound in the current script / call | `&mut ObjectWorld` |
| **Eval context** | `EvalContext<'a>` | Infrastructure: intent accumulator + source path | `&mut EvalContext` |

They are separate concerns. `ObjectWorld` is the *script-visible* storage; `EvalContext` is the *evaluator-visible* infrastructure.

---

## Variable environment

The evaluator no longer uses a flat `Env = HashMap<String, Value>`. Variable storage lives in
`ObjectWorld` as a frame stack:

- `Block` frames are transparent for lookup/reassign
- `Function` frames are hard barriers
- function frames hold a shared captured snapshot plus a mutable local overlay

### What goes in it

- `let x = expr` → bind in the top frame
- `import { foo }` → bind in the top frame
- `x = expr` (reassignment) → walk outward to the declaring reachable frame
- loop binding variable → bind in the loop body's block frame

`Value::Function` closures snapshot the visible env at definition time into `captured_env`.

### How variable storage flows through the evaluator

```
eval_block_stmts(stmts, ctx)
    └─ eval_stmt(stmt, ctx)
        ├─ push Block frame → eval nested block / if / loop body
        └─ push Function frame(shared captured snapshot + overlay) → eval function body
```

Bindings inside a block frame disappear when that frame is popped. Reassignments of an outer
reachable binding walk up to the declaring frame and update it there.

```mms
let y = -2.0
if (z > 64) {
    y = 2.0   // propagates back — y in the outer block is now 2.0
}
T.position(0.0, y, 0.0) {}  // sees updated y
```

Inner `let` bindings now shadow outer names in the usual lexical way and do not leak after the block exits.

### Loop scope

Loop bodies run in block frames. Reassignments to an outer-declared name walk outward to the
declaring frame, so accumulator-style loops now persist after the loop as standard lexical scoping:

```mms
let sum = 0
for i in [1, 2, 3] {
    sum = sum + i   // sum persists between iterations
}
```

The same applies to `while`.

### Function call scope

Each function call pushes a `Function` frame containing:

- a shared captured snapshot from the closure definition site
- a mutable local overlay for params, local lets, and reassignments

Lookup checks the overlay first, then the shared captured snapshot, then stops at the function
barrier. Reassigning a captured name writes a shadowing value into the overlay. Mutations inside
the function body remain invisible to the caller.

---

## `EvalContext<'a>` — evaluator infrastructure

```rust
struct EvalContext<'a> {
    emits: &'a mut Vec<IntentValue>,
    source_path: Option<&'a str>,
}
```

`EvalContext` carries state that belongs to the evaluator, not to the script.

### `emits`

A `Vec<IntentValue>` that accumulates `SpawnComponentTree` intents as `emit(ce)` calls are encountered. After the top-level `eval_block_stmts` returns, the collected intents are flushed to the response ring buffer (or returned from `eval_as_module`).

`emits` is not passed to expression-level functions (`eval_expr`, `eval_binop`, …) individually — those take `emits: &mut Vec<IntentValue>` directly. `EvalContext` wraps it at the **statement** level because only statements can contain `import` (which needs `source_path`).

### `source_path`

The filesystem path of the file currently being evaluated. Used solely to resolve relative `import "…"` paths. It is `None`:

- When source was passed as a raw string (e.g. `MeowMeowRunner::eval`)
- Inside function call bodies — closures don't carry a path, so relative imports inside user-defined functions would fail

### Where contexts are created

| Site | variable storage | source_path |
|---|---|---|
| `eval_script` (top-level script) | fresh `ObjectWorld` with root frame | from `EvalRequest` |
| `eval_as_module` (imported file) | fresh `ObjectWorld` with root frame | path of the imported file |
| `eval_call` (function body) | pushed `Function` frame over current `ObjectWorld` | `None` |

---

## Thread protocol and `EvalContext` vs engine context

`EvalContext` is private to the evaluator worker thread. The engine never sees it. Communication with the engine happens through a lock-free ring buffer:

```
┌──────────────────────────────┐       rtrb ring buffer        ┌──────────────────────┐
│  Caller (engine thread, test)│  ── EvalRequest ──────────►  │  MeowMeowEvaluator   │
│                              │  ◄── EvalResponse ──────────  │  (worker thread)     │
└──────────────────────────────┘                               └──────────────────────┘
```

**`EvalRequest`** (caller → worker):

```rust
EvalScript { source: String, source_path: Option<String> }
ParseScript { source: String }   // parse-only, returns AST debug string
Shutdown
```

**`EvalResponse`** (worker → caller):

```rust
Intent(IntentValue)              // one per emit() call in the script
ParsedOk { debug_ast: String }
Error { message: String }
ShutdownAck
```

`MeowMeowRunner` is the synchronous convenience wrapper: it spawns the worker, sends one `EvalScript` + `Shutdown`, drains all `EvalResponse`s into an `EvalOutput { intents, errors }`, and joins the thread.

---

## `StmtEffect` — how statements communicate upward

`eval_stmt` does not mutate env or emit intents directly. It returns a `StmtEffect`:

```rust
enum StmtEffect {
    None,
    Bind(String, Value),              // let x = expr
    Export(String, Value),            // export let x = expr
    ImportBindings(Vec<(String,Value)>), // import { … } from "…"
    Reassign(String, Value),          // x = expr  (must already exist)
    Return(Value),
    Break,
    Continue,
}
```

`eval_block_stmts` is the only function that **applies** these effects. It applies `Bind`/`Export`/`ImportBindings`/`Reassign` to env in place, and propagates `Return`/`Break`/`Continue` upward as its own return value. The ForIn arm inlines the same logic for its `loop_env`.

`eval_if` and `Statement::Block` in `eval_stmt` forward control-flow effects transparently; env mutations propagate automatically through the `&mut Env` chain.

---

## Module evaluation (`eval_as_module`)

`import` triggers `eval_as_module`, which is essentially a second top-level eval with different collection rules:

- CE emits go to a local `Vec<IntentValue>` (nothing spawns in the engine yet)
- `export let` / `export fn` bindings go to a `named: HashMap<String, Value>` map
- After evaluation, the CEs are extracted from the intents and returned alongside `named` as `Value::Module { named, sequence }`

The caller (the `Statement::Import` arm) then unpacks this module value and binds the requested names into its own env.

---

## Summary diagram

```
eval_script / eval_as_module
│  env: HashMap (fresh)
│  ctx: EvalContext { emits, source_path }
│
└─ eval_block_stmts(&mut env, &mut ctx)
       │  applies Bind/Reassign to env in place
       │  forwards Return/Break/Continue
       │
       ├─ eval_stmt(let x = …)         → StmtEffect::Bind
       ├─ eval_stmt(x = …)             → StmtEffect::Reassign
       ├─ eval_stmt(if …) → eval_if(&mut env)
       │      └─ eval_block_stmts(&mut env)   ← same env, no clone
       ├─ eval_stmt(for … in …)               ← ForIn inlines block loop
       │      loop_env = env.clone()           ← isolated copy; changes don't escape
       │      eval_stmt(…, &mut loop_env) per statement
       ├─ eval_stmt(while …)                  ← same isolated copy pattern
       │      loop_env = env.clone()
       │      eval_stmt(…, &mut loop_env) per statement
       └─ eval_stmt(fn call) → eval_call
              call_env = captured_env + args   ← isolated copy
              EvalContext { emits, source_path: None }
              eval_block_stmts(&mut call_env, &mut func_ctx)
```
