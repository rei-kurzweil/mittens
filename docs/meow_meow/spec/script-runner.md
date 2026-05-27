# MeowMeowRunner — synchronous script evaluation

`MeowMeowRunner` is the high-level entry point for evaluating an MMS script and
collecting the resulting intents. It wraps `MeowMeowEvaluator`'s ring-buffer protocol
into a simple synchronous call.

Implementation: `src/meow_meow/runner.rs`

---

## API

```rust
pub struct EvalOutput {
    pub intents: Vec<IntentValue>,
    pub errors:  Vec<String>,
}

pub struct MeowMeowRunner;

impl MeowMeowRunner {
    /// Evaluate `source`, collecting all emitted intents and errors.
    /// Times out after 2 seconds if the evaluator stalls.
    /// No world access — `let x = CE` binds a ComponentExpr snapshot, not a live id.
    pub fn eval(source: &str) -> EvalOutput

    /// Same, with a caller-provided timeout.
    pub fn eval_with_timeout(source: &str, timeout: Duration) -> EvalOutput

    /// Evaluate with live world access (reply channel open).
    /// `let x = CE` performs a HostCall round-trip and binds a ComponentObject(id).
    /// See docs/meow_meow/spec/eval-with-world.md for the full model.
    pub fn eval_with_world(source: &str, world: &mut World, emit: &mut dyn SignalEmitter) -> EvalOutput
}
```

`EvalOutput` is not a `Result` — partial failure is the normal case (some statements may
error while others succeed). Callers decide what to do with non-empty `errors`.

---

## Usage in examples

```rust
let output = MeowMeowRunner::eval(include_str!("scene.mms"));
for iv in output.intents {
    universe.command_queue.push_intent_now(scope, iv);
}
if !output.errors.is_empty() {
    eprintln!("MMS errors: {:?}", output.errors);
}
```

## Usage in tests

```rust
let output = MeowMeowRunner::eval("T { R.cube() { C.rgba(1,0,0,1) } }");
assert_eq!(output.errors, []);
assert_eq!(output.intents.len(), 1);
assert!(matches!(output.intents[0], IntentValue::SpawnComponentTree { .. }));
```

---

## Compilation pipeline

Every script goes through three stages before evaluation:

```
source: &str
  │
  ▼
[Tokenizer]  →  Vec<Token>           (src/meow_meow/tokenizer.rs)
  │
  ▼
[Parser]     →  Vec<Statement>       (src/meow_meow/parser.rs)
  │            (raw AST — sugar intact)
  ▼
[AstTransforms]  →  Vec<Statement>   (src/meow_meow/transform.rs)
  │
  ├─ EmitLiftTransform
  │    rewrites bare ComponentExpression statements → emit(ce) calls
  │
  └─ QueryDesugarTransform
       rewrites -> query/dispatch sugar → query()/query_all() calls
       (see mms-query.md for the rewrite rules)
  │
  ▼
[Evaluator]  →  EvalOutput           (src/meow_meow/evaluator.rs)
```

The parser produces a **raw AST** — it encodes `|>` as `BinOp(Pipe, lhs, rhs)` and `->`
as `BinOp(Query, lhs, rhs)` without interpreting the authoring sugar. Transform passes
rewrite the AST into a normal form the evaluator can handle directly.

**Why transforms, not parser rules?**
Keeping sugar detection out of the parser avoids context-dependent grammar rules. A string
literal is syntactically valid as the LHS of `->` — only the transform inspects whether it
should be rewritten as `query()` or `query_all()`. This keeps each stage single-responsibility.

**Adding a new transform:**
1. Add a struct implementing `fn apply(stmts: &mut Vec<Statement>)` in `transform.rs`.
2. Call it in the evaluation pipeline after parse and before eval (today this happens in `evaluator.rs`).
3. Document the rewrite rule in the relevant spec.

---

## What it does

1. Spawns a `MeowMeowEvaluator` thread (one-shot; not reused across calls).
2. Pushes `EvalScript { source }` then `Shutdown` onto the request ring buffer.
3. Drains `Intent` and `Error` responses until `ShutdownAck` or timeout.
4. Calls `shutdown_and_join()` before returning.

The evaluator thread is not kept alive between calls. Hot-reload or session reuse would
require a `MeowMeowSession` wrapper (not yet implemented).
