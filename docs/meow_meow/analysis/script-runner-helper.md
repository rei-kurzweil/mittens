# Script runner helper: synchronous intent collection

## The problem

Every caller that wants to evaluate an MMS script and collect the resulting intents must
manually manage the ring-buffer protocol — spawn evaluator, push two requests, drain
responses until `ShutdownAck` or timeout. This is ~25 lines of boilerplate that belongs in
the library, not in every example and future test.

The pattern is:

```rust
let mut eval = MeowMeowEvaluator::spawn(64);
eval.requests.push(EvalRequest::EvalScript { source }).unwrap();
eval.requests.push(EvalRequest::Shutdown).unwrap();

let mut intents = Vec::new();
let deadline = Instant::now() + Duration::from_secs(2);
loop {
    match eval.responses.pop() {
        Ok(EvalResponse::Intent(iv))          => intents.push(iv),
        Ok(EvalResponse::Error { message })   => eprintln!(...),
        Ok(EvalResponse::ShutdownAck)         => break,
        Ok(EvalResponse::ParsedOk { .. })     => {}
        Err(PopError::Empty) => {
            if Instant::now() > deadline { break; }
            std::thread::yield_now();
        }
    }
}
```

---

## What the helper needs to do

1. Accept a source string (or `&str`).
2. Spawn the evaluator thread (or reuse one — see below).
3. Drain all `Intent` responses.
4. Collect errors without panicking.
5. Return the intents and errors to the caller.
6. Clean up the evaluator thread.

It does **not** need to:
- Keep the evaluator alive between calls (one-shot is fine for v1).
- Support async — callers like `vr-input-mms.rs` run this synchronously before the engine
  starts. Tests are synchronous too.
- Expose the ring buffer at all — that's an implementation detail of `MeowMeowEvaluator`.

---

## Return type

```rust
pub struct EvalOutput {
    pub intents: Vec<IntentValue>,
    pub errors:  Vec<String>,
}
```

Simple pair. No `Result` wrapping — partial failure (some intents succeeded, one statement
errored) is the normal case for script evaluation. Callers decide what to do with errors.

---

## Name options

### Option 1: `MeowMeowRunner`

```rust
let output = MeowMeowRunner::eval(source);
// output.intents, output.errors
```

- Follows the existing `MeowMeowEvaluator` / `MeowMeowParser` naming convention.
- "Runner" is a standard name for "thing that runs a script and gives you output."
- Clearly a higher-level wrapper over `MeowMeowEvaluator`.
- Could later grow a `MeowMeowRunner::new()` + `.eval()` form if it needs config
  (timeout, queue size, etc.).

### Option 2: `MmsRunner` (or `MmsEval`)

```rust
let output = MmsRunner::eval(source);
```

- Shorter. Consistent with the `mms` module prefix style.
- Less distinctive in a codebase that could have many `Runner` types.

### Option 3: free function `eval_script`

```rust
let output = meow_meow::eval_script(source);
```

- Most direct. No struct, no type to import.
- Works well as a `pub fn` in `evaluator.rs` or a new `runner.rs`.
- Harder to extend later (can't add methods, harder to mock in tests).
- Fine for v1 if we're okay with a plain function.

### Option 4: `MeowMeowSession`

```rust
let mut session = MeowMeowSession::new();
let output = session.eval(source);
let output2 = session.eval(other_source); // reuses the thread
```

- Implies stateful, reusable session — the evaluator thread stays alive between calls.
- Useful if we want hot-reload (re-eval a file without teardown/spawn overhead).
- More API surface than needed for v1.

---

## Recommendation

**`MeowMeowRunner` with a static `eval()` method for v1**, with a path to `MeowMeowSession`
later:

```rust
pub struct MeowMeowRunner;

impl MeowMeowRunner {
    /// Evaluate `source` synchronously, collecting all emitted intents and errors.
    /// Spawns an evaluator thread, drains it to completion, and returns.
    pub fn eval(source: &str) -> EvalOutput {
        Self::eval_with_timeout(source, Duration::from_secs(2))
    }

    pub fn eval_with_timeout(source: &str, timeout: Duration) -> EvalOutput {
        // ... the 25-line boilerplate, centralized
    }
}
```

Usage in examples:

```rust
let output = MeowMeowRunner::eval(include_str!("vr-input.mms"));
for iv in output.intents {
    universe.command_queue.push_intent_now(scope, iv);
}
```

Usage in tests:

```rust
let output = MeowMeowRunner::eval("T { R.cube() { C.rgba(1,0,0,1) } }");
assert_eq!(output.errors, []);
assert_eq!(output.intents.len(), 1);
assert!(matches!(output.intents[0], IntentValue::SpawnComponentTree { .. }));
```

---

## Where it lives

`src/meow_meow/runner.rs` — separate from `evaluator.rs` (which stays as the low-level
thread protocol). Export from `mod.rs` alongside the evaluator.

---

## Blockers and decisions before implementing

- **Name**: `MeowMeowRunner` vs `MmsRunner` vs free function — pick one.
- **`let`, `if`, `for`**: the runner helper is a prerequisite for cleanly writing tests for
  these features. Implement the runner first, then add evaluation tests for new language
  constructs on top of it.
- **Error handling policy**: should `EvalOutput::errors` being non-empty cause a test
  failure by default, or is that the caller's job? Probably caller's job — the runner just
  collects.
