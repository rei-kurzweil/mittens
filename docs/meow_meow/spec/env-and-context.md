# ᓚᘏᗢ Env and Evaluation Context

How variable environments, scoping, and evaluation context work in the MMS evaluator.

---

## The two orthogonal "contexts"

MMS evaluation threads two distinct kinds of state through the evaluator:

| Thing | Type | What it holds | Mutable? |
|---|---|---|---|
| **Variable env** | `Env` = `HashMap<String, Value>` | Names bound in the current script | `&mut Env` |
| **Eval context** | `EvalContext<'a>` | Infrastructure: intent accumulator + source path | `&mut EvalContext` |

They are always passed as separate arguments. Env is the *script-visible* state; `EvalContext` is the *evaluator-visible* infrastructure.

---

## `Env` — the variable environment

```rust
type Env = HashMap<String, Value>;
```

A flat `HashMap<String, Value>`. Every name in scope lives as a key in the map. There is no scope chain or frame stack in v1 — just one map that grows as `let` statements add bindings.

### What goes in it

- `let x = expr` → `Bind("x", val)` effect → `env.insert("x", val)`
- `import { foo }` → `ImportBindings([("foo", val)])` effect → `env.insert`
- `x = expr` (reassignment) → `Reassign("x", val)` effect → `env.insert` (must already exist)
- For-loop binding variable → inserted at the start of each iteration

`Value::Function` closures snapshot the env at definition time into `captured_env` (see Functions below).

### How env flows through the evaluator

```
eval_block_stmts(stmts, env: &mut Env, ctx)
    └─ eval_stmt(stmt, env: &mut Env, ctx)
        ├─ eval_if(if_stmt, env: &mut Env, ctx)
        │   └─ eval_block_stmts(branch.statements, env: &mut Env, ctx)  ← same env
        ├─ eval_block_stmts(block.statements, env: &mut Env, ctx)        ← same env
        └─ (ForIn) eval_stmt(stmt, &mut loop_env, ctx)                   ← different env
```

`eval_block_stmts` and `eval_if` receive `env` by `&mut` and **do not clone it**. Bindings and reassignments applied inside an if-branch or nested block are immediately visible to the enclosing block. This is why:

```mms
let y = -2.0
if (z > 64) {
    y = 2.0   // propagates back — y in the outer block is now 2.0
}
T.position(0.0, y, 0.0) {}  // sees updated y
```

> **Scoping note (v1):** there is no block-level shadowing. `let` inside an if-block adds to the same env as the surrounding code; those names remain after the block exits. This matches Python / Lua semantics. A proper scope chain (shadowing, block-local vars) is deferred.

### `for` loop env

The `ForIn` evaluator arm clones env once before the loop to create `loop_env`:

```rust
let mut loop_env = env.clone();
'for_loop: for item in items {
    loop_env.insert(binding.0.clone(), item);  // set loop var
    for stmt in &body.statements {
        match eval_stmt(stmt, &mut loop_env, ctx)? { ... }
    }
}
```

`loop_env` is **shared across all iterations** — bindings and reassignments accumulate. This makes the classic accumulator pattern work:

```mms
let sum = 0
for i in [1, 2, 3] {
    sum = sum + i   // sum persists between iterations
}
```

If-blocks inside the loop body propagate to `loop_env` correctly because `eval_stmt` receives `&mut loop_env` and `eval_if` → `eval_block_stmts` mutate it in place.

**What doesn't propagate:** reassignments inside the loop do not escape back to the env that existed before the loop started. `loop_env` is a clone, not a reference into the outer env. This means:

```mms
let sum = 0
for i in [1, 2, 3] {
    sum = sum + i   // accumulates within loop_env — works across iterations
}
// sum in the outer env is still 0 here — loop_env was discarded
```

The `while` loop uses the same `loop_env` pattern with identical semantics.

This is a known v1 limitation. The fix is a proper scope chain (v2) where reassignment walks the frame stack to update the correct binding in whichever frame originally declared the name.

### Function call env

`eval_call` builds a completely fresh env for each call:

```rust
let mut call_env = captured_env;                // start from closure's snapshot
for (param, arg) in params.iter().zip(args) {
    call_env.insert(param.clone(), arg.clone()); // bind arguments
}
```

`call_env` is independent of the caller's env. Mutations inside the function body are invisible to the caller. Functions are closures — they capture a snapshot of the env at the `fn` expression site (`captured_env: env.clone()`).

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

| Site | env | source_path |
|---|---|---|
| `eval_script` (top-level script) | fresh empty `HashMap` | from `EvalRequest` |
| `eval_as_module` (imported file) | fresh empty `HashMap` | path of the imported file |
| `eval_call` (function body) | `captured_env` + args | `None` |

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
