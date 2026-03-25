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
    pub fn eval(source: &str) -> EvalOutput

    /// Same, with a caller-provided timeout.
    pub fn eval_with_timeout(source: &str, timeout: Duration) -> EvalOutput
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

## What it does

1. Spawns a `MeowMeowEvaluator` thread (one-shot; not reused across calls).
2. Pushes `EvalScript { source }` then `Shutdown` onto the request ring buffer.
3. Drains `Intent` and `Error` responses until `ShutdownAck` or timeout.
4. Calls `shutdown_and_join()` before returning.

The evaluator thread is not kept alive between calls. Hot-reload or session reuse would
require a `MeowMeowSession` wrapper (not yet implemented).
