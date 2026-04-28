# ᓚᘏᗢ Reply Channel, Component Methods, and Session Model — Design

> **Status: draft / design exploration.**
> No source changes yet. This doc works through the design of Phase 6 (ComponentId
> reply channel), Phase 7 (method calls on ComponentObjects), the async session model,
> and independently timed background processes in MMS.

---

## The problem in one paragraph

After `let hero = T { R.cube() { C.rgba(1,0,0,1) } }`, the script has no way to
reference `hero` as a live component. It holds a `Value::ComponentExpr` — a snapshot
of the AST — not a `ComponentId`. To call `hero.set_rgba(0,1,0,1)` the script needs a
real `ComponentId` back from the engine. Getting that ID requires a round-trip: MMS
emits a spawn intent, the host processes it and assigns an ID, MMS receives the ID and
resumes. This round-trip is the **reply channel**. Once it exists, `expr.method(args)`
on a live `ComponentObject` can emit mutation intents — and the same mechanism covers
`query("#hero")`, `world_position(id)`, and every other future HostCall, including
background process timers.

---

## The four things and why they're the same mechanism

| Feature | What it needs |
|---|---|
| Phase 6: ComponentId reply | MMS → host: "spawn this"; host → MMS: "here's the ID" |
| Phase 7: `component.method()` | MMS parses `expr.method(args)`; emits intent or makes HostCall |
| Session model | Host → MMS: "handle this event"; MMS runs handler, emits intents |
| Background processes | MMS → host: "wait N ms"; host → MMS: "time elapsed, resume" |

All four share the same protocol: **bidirectional message passing between the MMS
evaluator and the host, with the evaluator suspending at boundary-crossing points and
resuming with a reply value.**

---

## Current protocol (one-shot, one-directional during eval)

```
      Host                              MMS evaluator thread
        │                                      │
  1. ──►│ EvalRequest::EvalScript              │
  2. ──►│ EvalRequest::Shutdown                │
        │                                      │ (parse + eval script)
        │                                      │ (for each emit(ce):)
  3.    │        EvalResponse::Intent ◄────────│
  4.    │      EvalResponse::ShutdownAck ◄─────│
  5.    │ (join thread)                        │
```

During evaluation, communication is **one-way outward**: MMS pushes intents, host
collects them. The host only sends to MMS before and after evaluation, never during.

---

## Proposed protocol: bidirectional during eval

```
      Host                              MMS evaluator thread
        │                                      │
  1. ──►│ EvalRequest::EvalScript              │
        │                                      │ (eval... hits a spawn CE)
  2.    │    EvalResponse::HostCall ◄──────────│  { id: 1, kind: Spawn(ce) }
        │ (process spawn → get ComponentId)    │ (spin-polling requests)
  3. ──►│ EvalRequest::HostCallResult          │  { id: 1, value: ComponentId(42) }
        │                                      │ (resume: hero = ComponentObject(42))
        │                                      │ (eval continues...)
  4.    │       EvalResponse::Intent ◄─────────│  SetColor { id: 42, ... }
  5. ──►│ EvalRequest::Shutdown                │
  6.    │     EvalResponse::ShutdownAck ◄──────│
  7.    │ (join thread)                        │
```

### New message types

```rust
// outgoing (MMS → host):
EvalResponse::HostCall {
    id: u32,           // correlation id
    kind: HostCallKind,
}

enum HostCallKind {
    Spawn(ComponentExpression),
    Query { selector: String },
    QueryAll { selector: String },
    Wait { ms: u64 },              // background process timer
    // future: WorldPosition(ComponentId), Rand, ElapsedTime, ...
}

// incoming (host → MMS):
EvalRequest::HostCallResult {
    id: u32,
    value: HostValue,
}

enum HostValue {
    ComponentId(ComponentId),
    ComponentIds(Vec<ComponentId>),
    Null,
    Unit,    // for Wait — "you may resume now"
    // future: Vec3, f64, ...
}
```

---

## Spin-wait vs proper suspend

Two options for the evaluator pausing at a HostCall:

### Option A: Spin-wait (simple, works now)

```rust
responses.push(EvalResponse::HostCall { id, kind })?;
loop {
    match requests.pop() {
        Ok(EvalRequest::HostCallResult { id: reply_id, value }) if reply_id == id => {
            break value;
        }
        Ok(other) => handle_other(other),
        Err(Empty) => std::thread::yield_now(),
    }
}
```

Works with the current tree-walking evaluator. No coroutines. The evaluator thread blocks
but does no harm — it's dedicated. **Recommended for Phase 6/7.**

### Option B: Coroutine yield (needed for background processes)

The evaluator is a generator. A HostCall becomes a `yield` — the scheduler switches to
another runnable task and resumes this one when the reply arrives. No spin. Required for
multiple concurrent tasks on one thread. **Design target for background processes.**

---

## Session model

A session is a one-shot evaluator that does not shut down after the first script and
accepts new work via `FireEvent`:

```
      Host                              MMS session thread
        │                                      │
  1. ──►│ EvalRequest::EvalScript              │  (init script)
        │   ... HostCall round-trips ...       │  (registers handlers, spawns scene)
  2.    │     EvalResponse::ScriptDone ◄───────│
        │                                      │  (thread stays alive)
        │
        │  (later — button clicked:)
  3. ──►│ EvalRequest::FireEvent               │  { name: "clicked", args: { target: 42 } }
        │                                      │  (runs registered handlers)
  4.    │       EvalResponse::Intent ◄─────────│  (mutation from handler)
  5.    │     EvalResponse::EventDone ◄────────│
        │
        │  (later — shutdown:)
  6. ──►│ EvalRequest::Shutdown                │
  7.    │     EvalResponse::ShutdownAck ◄──────│
  8.    │ (join thread)                        │
```

New response `ScriptDone` (replaces `ShutdownAck` for sessions) and `EventDone` signal
the end of one unit of work without shutting down the thread.

---

## Background processes

### The idea

A script may want to run work on its own schedule, independently of the host's frame
rate. Examples:

```mms
// pulse a light every 2 seconds
spawn fn() {
    while true {
        wait(2000)
        "#point_light" |> set_intensity(1.5)
        wait(200)
        "#point_light" |> set_intensity(1.0)
    }
}

// cycle through colors every 500ms
spawn fn() {
    let colors = [[1,0,0,1], [0,1,0,1], [0,0,1,1]]
    let i = 0
    while true {
        let c = colors[i % 3]
        "#hero" |> set_rgba(c[0], c[1], c[2], c[3])
        i = i + 1
        wait(500)
    }
}
```

These loops run forever, at their own pace, without blocking the host's render loop or
each other.

### The `wait` primitive

`wait(ms)` suspends the current task for at least `ms` milliseconds. It is a HostCall —
the evaluator sends a `Wait` request and the host replies when the time has elapsed:

```
      Host                              MMS task (inside spawn)
        │                                      │
  1.    │    EvalResponse::HostCall ◄──────────│  { id: 7, kind: Wait { ms: 2000 } }
        │ (sets a 2s timer)                    │ (suspended)
        │
        │  (2000ms later:)
  2. ──►│ EvalRequest::HostCallResult          │  { id: 7, value: Unit }
        │                                      │ (resumes)
  3.    │       EvalResponse::Intent ◄─────────│  SetIntensity { ... }
        │                                      │ (loops back to next wait)
```

The host controls the clock. MMS does not sleep OS threads — it sends a wait request
and the host delivers the wake-up at the right time (via a timer, via tick accumulation,
or via a dedicated timer thread).

### `wait` is not tied to host fps

This is the key point. The host can deliver `HostCallResult { Unit }` for a `Wait` at
any point — it does not have to align with a frame boundary. The host might use:

- A dedicated timer thread that sleeps and then pushes `HostCallResult`
- A `tokio::time::sleep` in an async context
- Accumulated tick time checked each frame (`if elapsed >= requested_ms { reply }`)

The last option (frame-aligned timer) means `wait(2000)` fires at the next frame
boundary after 2 seconds — close enough for most use cases and the simplest to implement.
The first option (dedicated timer thread) gives exact timing but adds a thread.

### Language primitives for background processes

#### `spawn expr` — fire and forget

```mms
spawn fn() { ... }         // spawn an anonymous task
spawn color_cycler()       // spawn a generator call
```

`spawn` is a statement-level expression that creates a new independent task from any
callable. The task runs concurrently with the current context (and with other spawned
tasks). The spawning script does not wait for it.

`spawn` returns a handle that can be used to cancel the task:

```mms
let handle = spawn fn() { while true { wait(1000); do_thing() } }
// later:
handle.cancel()
```

#### `wait(ms)` — suspend current task

A HostCall builtin. Only meaningful inside a `spawn`ed task (or a generator driven by
the session scheduler). Calling `wait` in a one-shot script would block the evaluator
thread for the duration — probably fine for short waits, but not idiomatic.

#### `every(ms, fn)` — periodic callback sugar

```mms
every(2000, fn() {
    "#hero" |> set_rgba(rand(), rand(), rand(), 1)
})
```

Desugars to:

```mms
spawn fn() {
    while true {
        wait(2000)
        (fn() {
            "#hero" |> set_rgba(rand(), rand(), rand(), 1)
        })()
    }
}
```

`every` is a stdlib function, not a keyword. No language changes needed beyond `spawn`
and `wait`.

#### Generators (`fn*` / `yield`) — resumable sequences

For tasks that need to pass values between yield points or be driven externally:

```mms
fn* fade_in(target) {
    let t = 0.0
    while t < 1.0 {
        target |> set_opacity(t)
        t = t + 0.05
        yield wait(16)   // ~60fps
    }
    target |> set_opacity(1.0)
}

// driven by the session scheduler:
spawn fade_in(query("#hero"))
```

`yield wait(16)` yields the generator to the scheduler with a wait request. The scheduler
resumes it after 16ms. This is the same `Wait` HostCall, just surfaced differently.

---

## Thread models for background tasks

Three options for where spawned tasks run:

### Option A: One OS thread per task (simplest)

Each `spawn` creates a new `MeowMeowEvaluator` thread with its own ring buffer to the
host. The host multiplexes HostCalls from all evaluator threads.

```
Host thread
  ├── ring buffer ──► MMS session thread (main script + event handlers)
  ├── ring buffer ──► MMS task thread 1  (spawn 1)
  └── ring buffer ──► MMS task thread 2  (spawn 2)
```

**Pros:** simple — each task is already an evaluator, no scheduler needed.
**Cons:** OS thread per task is heavy. 100 spawned tasks = 100 threads.
**Good for:** a small number of long-running tasks (< ~10). Not for fine-grained tasks.

### Option B: Coroutines within the session thread (recommended)

The session thread runs a **scheduler** that manages multiple coroutines. Each spawned
task is a coroutine (generator). The scheduler advances whichever coroutine is runnable,
suspending at HostCall yield points.

```
MMS session thread
  ├── scheduler
  │     ├── task 1: fade_in generator (waiting for wait(16) reply)
  │     ├── task 2: color_cycler generator (waiting for wait(500) reply)
  │     └── task 3: event handler (runnable — just received FireEvent)
  │
  └── ring buffer ──► Host thread (all HostCalls multiplexed over one buffer)
```

The host only sees one ring buffer regardless of how many tasks are running. Wait replies
include the task id so the scheduler knows which coroutine to resume.

**Pros:** lightweight — no extra OS threads. Natural fit for many fine-grained tasks.
**Cons:** requires coroutine implementation in the evaluator (not trivial).
**This is the right long-term target.**

### Option C: Async Rust task (future / external runtime)

If the evaluator ever moves to an async runtime (tokio), `spawn` → `tokio::spawn`,
`wait` → `tokio::time::sleep`. No custom scheduler needed.

**Pros:** proven runtime, exact timing.
**Cons:** requires restructuring the evaluator as async code. Major investment.

---

## How `spawn` fits with cron-like scheduling

`every(ms, fn)` covers the simple periodic case. For more complex schedules, a cron-like
form could be added to the stdlib:

```mms
// hypothetical stdlib function
cron("0 * * * *", fn() {
    // every hour
    update_hourly_stats()
})

// desugars to (simplified):
spawn fn() {
    while true {
        let ms_until_next = next_cron_tick("0 * * * *")
        wait(ms_until_next)
        (fn() { update_hourly_stats() })()
    }
}
```

`cron` is pure MMS stdlib — no new language features needed beyond `spawn`, `wait`,
and a `next_cron_tick` helper that computes time-until-next-tick from a cron expression.
`next_cron_tick` would itself be a HostCall (needs the current wall time from the host).

---

## Phase 7: `expr.method(args)` — call-callee shape and eval

### AST shape

No special `Expression::MethodCall` node is required.

`expr.method(args)` is represented as a normal call expression:

```rust
Expression::Call(CallExpression {
    callee: Box::new(Expression::BinaryOp {
        op: BinOpKind::Dot,
        lhs: Box::new(expr),
        rhs: Box::new(Expression::Identifier(Ident("method".into()))),
    }),
    args,
})
```

### Parser disambiguation

Component expression constructor calls (`T.cube()`) are handled by the CE parser path
when the leading identifier is a component type name. Value method-style calls
(`obj.method()`) parse as `Expression::Call` whose callee is a dot-expression.

Whether non-call field access should exist as a separate runtime feature is a different
question; it is not needed for `expr.method(args)`.

### Evaluator dispatch

```
eval CallExpression { callee = Dot(receiver, method), args }
  │
  ├─ Value::ComponentObject(id) ──► ComponentObject method table
  │       fire-and-forget (set_rgba, set_position) ──► emit Intent, return Null
  │       query methods (query, query_all)         ──► HostCall round-trip, return Value
  │
  ├─ Value::Struct { type_name, fields } ──► impl table for type_name
  │       call as MMS closure (self = struct value)
  │
  └─ other ──► runtime error
```

---

## Implementation order

```
      Step 1: HostCall infrastructure
        │  Add EvalResponse::HostCall + EvalRequest::HostCallResult to ring buffer types.
        │  Add spin-wait helper in evaluator. Wire a synthetic ping() builtin to test it.
        │
      Step 2: Spawn with ComponentId reply  [Phase 6]
        │  emit(ce) → HostCall::Spawn → host processes SpawnComponentTree → HostCallResult
        │  Evaluator resumes with Value::ComponentObject(id).
        │
      Step 3: Dot-callee call dispatch  [Phase 7 syntax]
        │  Parser shape already exists via `CallExpression { callee: Box<Expression> }`.
        │  Evaluator dispatches on dot-callee receiver type.
        │
      Step 4: Fire-and-forget mutation methods  [Phase 7 semantics]
        │  set_rgba, set_position, etc. → emit Intent, return Null. No HostCall needed.
        │
      Step 5: query / query_all builtins via HostCall
        │  Wire free-function query() and component.query() to HostCall round-trip.
        │
      Step 6: MeowMeowSession  [session model]
        │  Refactor runner: don't join after init script. Add FireEvent / EventDone.
        │  Integrate with cat-engine tick drain points.
        │
      Step 7: wait() builtin + spawn statement  [background processes]
             wait() → HostCall::Wait → host timer → HostCallResult::Unit.
             spawn → create new task context (initially: new OS thread; later: coroutine).
             every(ms, fn) as stdlib sugar over spawn + wait.
```

---

## Open questions

1. **Spawn granularity for ComponentIds** — every `let x = T { }` round-trips for an ID.
   Bare `T { }` statements (no binding) can remain fire-and-forget. This means: only
   bound CEs get IDs; unbound CEs are anonymous spawns.

2. **Who handles HostCalls on the host side** — `MeowMeowRunner` becomes active during
   eval: intercepts `HostCall`, processes it (spawn/query/wait), pushes `HostCallResult`.
   Document this clearly — the runner is no longer just a passive collector.

3. **Timer accuracy for `wait`** — frame-aligned timers (simplest) vs dedicated timer
   thread (accurate). Frame-aligned means `wait(2000)` fires at the next frame after 2
   seconds. Probably fine for v1. Accurate timers needed for audio/animation sync.

4. **Coroutine implementation** — Option B (coroutines in session thread) requires the
   evaluator to be a generator internally. The tree-walking `eval_block_stmts` would need
   to support suspend/resume at HostCall points. Non-trivial refactor. Defer until `spawn`
   usage reveals whether the overhead of Option A (thread-per-task) is acceptable.

5. **Task error handling** — if a spawned task panics or errors: log and kill that task,
   keep the session running. The host is notified via `EvalResponse::TaskError { task_id, message }`.

6. **`every` vs `cron` priority** — `every(ms, fn)` covers 90% of use cases. Full cron
   expressions add complexity for rare cases. Ship `every` first; add cron later if needed.
