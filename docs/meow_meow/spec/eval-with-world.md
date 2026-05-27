# ₊˚ʚ eval_with_world — Live Reply Channel

`MeowMeowRunner::eval_with_world` is the **live evaluation path**: it gives the MMS
evaluator access to a running `World` during script execution, so that component
expressions bound to variables produce live `ComponentObject(id)` handles instead of
dead `ComponentExpr` AST snapshots.

Implementation: `src/meow_meow/runner.rs`, `src/meow_meow/evaluator.rs`

The HostCall message types and per-variant servicer behaviour are documented separately
in [host-call-api.md](host-call-api.md). This doc covers the live-eval *lifecycle* —
when the channel opens, what threads are involved, how blocking works.

---

## Why it exists

The basic `eval` path is fire-and-forget: the evaluator collects all intents, returns
them, and the caller feeds them into the world afterward. Component bindings like
`let box = T { }` produce `Value::ComponentExpr` — the AST node, not a live ID.

That is sufficient for simple scene setup. It breaks down as soon as a script needs
to **navigate or mutate** a spawned component:

```mms
let box = T.position(0, 0, -1) { R { CUBE; C.rgba(1,1,1,1) } }

fn handle_press() {
    box."C".set_color(0, 1, 0, 1)   // needs box to be a real ComponentId
}
```

`box."C"` requires a live `ComponentId` to issue a HostCall query. Without
`eval_with_world`, `box` is just an AST node and the method call has nothing to target.

---

## What changes with eval_with_world

When a `let` binding evaluates to a `ComponentExpr` and a reply channel is open, the
evaluator performs a **HostCall round-trip** instead of storing the AST:

```
evaluator thread                         main thread (caller)
─────────────────────────────────────    ──────────────────────────────────
eval: let box = T { ... }
  → evaluates CE to ComponentExpr
  → emits HostCall::Spawn(ce)  ─────►   receives EvalResponse::HostCall
  → spin-yield, waiting                 calls spawn_tree(ce, world, emit)
                                        gets back root ComponentId
                               ◄─────   pushes HostCallResult::ComponentId(id)
  → receives id
  → binds Value::ComponentObject(id)
  evaluation continues
```

After the round-trip, `box` holds `Value::ComponentObject(id)` — a handle to the
live, spawned component. Later calls to `box."C"` or `box.method(...)` have a real
`ComponentId` to work with.

---

## API

```rust
pub fn eval_with_world(
    source: &str,
    world: &mut World,
    emit: &mut dyn SignalEmitter,
) -> EvalOutput
```

The caller provides mutable world access. The call blocks until the script finishes
(including all HostCall round-trips). `EvalOutput` is the same as `eval` — any
remaining intents (bare CE statements not bound to variables) are returned for the
caller to dispatch.

```rust
// Before — fire-and-forget, no live ids
let output = MeowMeowRunner::eval(source);
for iv in output.intents {
    universe.command_queue.push_intent_now(scope, iv);
}

// After — live reply channel, ComponentObject bindings work
let output = MeowMeowRunner::eval_with_world(
    source,
    &mut universe.world,
    &mut universe.command_queue,
);
// output.intents contains any bare CE emits that weren't bound to variables
```

---

## Threading model

The evaluator runs on a **dedicated thread** spawned by `MeowMeowEvaluator::spawn`.
The thread is sequential: one script at a time, statement by statement. It does not
evaluate other work while blocked on a HostCall — the thread is waiting for the main
thread to service the reply.

```
Main thread          Evaluator thread
    │                     │
    ├─ spawn eval ────────►├
    │                      ├─ parse + transform
    │                      ├─ eval stmt 1 (let box = T {})
    │  ◄─ HostCall ────────┤  (emits HostCall, blocks)
    ├─ spawn_tree ─────────►│
    ├─ push HostCallResult ─►│
    │                      ├─ continues (box = ComponentObject(id))
    │                      ├─ eval stmt 2, 3, ...
    │  ◄─ Intent ──────────┤  (bare CE emits)
    │  ◄─ ShutdownAck ─────┤
    ├─ join thread         │
```

The evaluator thread holds the world **indirectly** — the main thread does the actual
world mutation (via `spawn_tree`), then sends back the resulting ID. The world is never
accessed from the evaluator thread directly.

---

## Spin-yield: CPU usage during blocking

The evaluator thread waits for `HostCallResult` using `std::thread::yield_now()` in a
loop:

```rust
Err(rtrb::PopError::Empty) => {
    std::thread::yield_now();
}
```

`yield_now()` calls `sched_yield` (Linux) / `SwitchToThread` (Windows). This is **not
a tight spin** — the thread yields its CPU timeslice and re-enters the scheduler's run
queue. Other threads (including the main thread servicing the HostCall) can run before
the evaluator is rescheduled.

It is also not a sleep: the thread stays *runnable* and will be rescheduled within the
next scheduling quantum (typically ≤1ms on a loaded system). For a HostCall that
completes in ~100µs (spawning a small component tree), this means a handful of yields
before the reply arrives.

**Is this a problem?** For occasional script loads or REPL commands: no. The evaluator
thread is not hot in steady state — it only runs when there's a script to evaluate.
For a tight loop emitting hundreds of HostCalls per frame: worth profiling, but the
bottleneck is likely spawn_tree itself rather than scheduling overhead.

If sub-yield latency ever matters (e.g. a script spawns thousands of individual
components), a `std::hint::spin_loop()` hint inside the inner loop would reduce
latency at the cost of higher CPU burn while waiting. `yield_now()` is the right
default.

---

## Multiple concurrent scripts: is a scheduler needed?

Currently: **no scheduler**. Each `MeowMeowEvaluator::spawn()` creates one thread that
evaluates one script at a time. If you need to evaluate two scripts concurrently, spawn
two evaluators.

The main thread's drain loop in `eval_with_world` handles one evaluator at a time —
HostCalls from one evaluator are serviced synchronously before the drain loop moves on.

A scheduler would be warranted if:
1. You have many scripts evaluating concurrently and want to bound thread count.
2. You want to interleave HostCall servicing across multiple blocked evaluators
   (one big drain loop handling all of them).
3. Scripts are long-running and the main thread shouldn't block on a single one.

None of these apply to the current use case (scene load at startup, REPL one-shot
eval). A `MeowMeowSession` wrapper (persistent evaluator thread, not shut down between
evals) and a multi-evaluator drain loop are the natural next steps when needed.

---

## Fallback behaviour

If channels are not available (the `channels: None` path — `eval` without world, module
evaluation, or inside a function body), the `HostCall` is never emitted. The binding
falls back to `Value::ComponentExpr`. This means:

- `eval` continues to work unchanged for scene setup that doesn't need live IDs.
- Module evaluation is always no-world (CEs are collected statically, not spawned).
- Function bodies called during eval also use the `channels: None` context — closures
  that spawn components must be called at the top level where channels are live, not
  from inside an imported function.

The third restriction may be revisited when closures gain access to the reply channel
explicitly.

---

## Open questions

| Question | Stakes |
|----------|--------|
| Should function bodies inherit the reply channel from their call site? | Closures capturing spawned components need this |
| `eval_with_path` + world: add `eval_file_with_world`? | Convenience for loading from disk |
| Timeout for HostCall spin-wait: currently 5s (eval_with_world). Right value? | Hung spawn should not hang the main thread forever |
| Multi-evaluator drain loop | Required for concurrent script loading (e.g. streaming level load) |
