# ᓚᘏᗢ MMS Async Model — Design Draft

> **Status: draft / design exploration.**
> This doc explores the design space for persistent, event-driven, and resumable MMS
> execution. Nothing here is implemented. Use this to inform decisions about session
> design, HostCall protocol, and whether/how to add `yield` or `async/await`.

---

## The problem with one-shot evaluation

The current model is a batch job:

```
MeowMeowRunner::eval(source)
  parse → transform → eval → collect intents → EvalOutput
  thread dies
```

This is fine for scene construction — run a script, get a list of spawns, apply them,
done. But it cannot express:

- **Reactive behavior** — run some code when a button is clicked, when a physics collision
  happens, when a timer fires
- **Stateful logic** — a button that cycles through three states; a counter that
  accumulates across clicks
- **Async queries** — pause, ask the engine for a live value (world position, query
  result), resume with the answer
- **Timed sequences** — turn red, wait 2 seconds, turn green (without a callback pyramid)

All of these require the script to **outlive its first evaluation** and **keep state**
across invocations.

---

## Reference: the JavaScript event loop

JS is the most widely deployed single-threaded, event-driven, async script runtime. Its
execution model is worth examining before designing MMS's own.

### What JS does

```
JS event loop:

  ┌─────────────────────────────────────────────────────────┐
  │  current task (call stack)                              │
  │    runs to completion — no preemption                   │
  └─────────────────────────────────────────────────────────┘
        ↓  task ends
  ┌─────────────────────────────────────────────────────────┐
  │  microtask queue (drained fully before next macrotask)  │
  │    Promise resolutions, queueMicrotask()                │
  └─────────────────────────────────────────────────────────┘
        ↓  microtask queue empty
  ┌─────────────────────────────────────────────────────────┐
  │  render step (requestAnimationFrame callbacks)          │
  └─────────────────────────────────────────────────────────┘
        ↓
  ┌─────────────────────────────────────────────────────────┐
  │  next macrotask (setTimeout, I/O, user events)          │
  └─────────────────────────────────────────────────────────┘
```

Key properties:
- **Run to completion** — a task runs without interruption; nothing else executes
  concurrently
- **No preemption** — the event loop never stops a task mid-execution
- **Microtasks flush eagerly** — all Promise resolutions run before the next user event
  or timer fires
- **async/await** — syntactic sugar over Promises; pauses the async function at each
  `await`, resumes when the awaited value resolves

### What maps well to MMS

| JS concept | MMS equivalent | Notes |
|---|---|---|
| Run to completion | Handler runs fully before next event | Already how the engine's drain points work |
| Macrotask = user event | Engine tick / event handler invocation | Natural boundary |
| Event callback registration | `on_clicked = fn(e) { }` in component body | See event-handlers.md |
| No preemption | Single-threaded evaluator | Already the case |
| Persistent JS heap | Persistent MMS env + heap | The key missing piece |

### What doesn't map cleanly

| JS concept | Problem for MMS |
|---|---|
| Microtask queue | Probably overkill — the engine's "follow-up intents applied immediately, follow-up events deferred" already covers this use case |
| Promise as a first-class value | Requires a heap-allocated future type and `then()` method — significant complexity |
| Full async/await | Requires the evaluator to be a continuation/state-machine, not a tree-walker — heavy redesign |
| Multiple concurrent async chains | JS is single-threaded but multiple Promise chains can be "in flight" — tracking this in MMS env would be complex |

**Conclusion on JS model:** the scheduling model (run to completion, event queue) maps
well and MMS should follow it. The Promise/async/await machinery is overengineered for
the use cases MMS needs in the near term.

---

## The session / actor model

A cleaner framing than "JS event loop" for MMS is the **actor model**:

```
MMS Session = an actor with:
  ├─ private state: env + heap (persists across all messages)
  ├─ inbox: events / queries from the host
  └─ outbox: intents / query responses / print output to the host
```

The session processes one message at a time (run to completion). No shared mutable state
between sessions. The host sends messages; the script handles them and sends back
responses.

This is very close to how Erlang processes work, and how web servers work conceptually:
the server has application state, handles requests one at a time (or concurrently, but
for MMS: one at a time), sends responses.

### Session vs Runner

| | `MeowMeowRunner` (current) | `MeowMeowSession` (future) |
|---|---|---|
| Lifetime | One script evaluation | Lives for the duration of a scene |
| State | Discarded after eval | Persistent env + heap |
| Event handling | None | Registered callbacks, fired on demand |
| HostCall | Not supported | Bidirectional: query → wait → resume |
| Thread | Spawns and joins | Long-lived thread (or task) |
| Use case | Scene construction | Interactive, reactive, stateful behavior |

`MeowMeowRunner` stays as-is for scene construction scripts. `MeowMeowSession` is a new
type that wraps a long-lived evaluator thread.

---

## Bidirectional communication

The current ring-buffer protocol is **unidirectional during evaluation**: MMS produces
intents, host consumes them. There is no path for the host to send data back while the
script is mid-execution.

For a session model with HostCalls, communication must be bidirectional:

```
MMS thread                          Host thread
─────────                          ───────────
eval_call("query", ...)
  send EvalResponse::HostCall       →  (ring buffer)
  spin-wait for EvalRequest...
                                    process query
                                    ← EvalRequest::HostCallResult(value)
  resume with value
  continue evaluation
```

This is the "spin-wait" model already sketched in `function-dispatch.md`. It works for
simple cases (one outstanding query at a time) without requiring coroutines.

### One outstanding call at a time

The simplest implementation: the evaluator blocks on a HostCall until the result arrives.
The host is responsible for replying before the evaluator times out.

```
eval        →  request     →  host
            ←  reply       ←
eval resumes
```

This works well for queries (world position, component lookup). It serializes the
interaction — no two HostCalls can be in flight simultaneously. This is fine for most
game scripting use cases.

### Multiple concurrent calls (future)

If the script wants to issue multiple queries and wait for all results:

```mms
// hypothetical — requires concurrent HostCall support
let [pos, color] = await_all(world_position(hero), get_color(hero))
```

This requires tracking multiple in-flight requests and collecting results. This is where
Promise-like structures would be useful. **Defer this** — one-at-a-time is sufficient
for v1 session behavior.

---

## Resumable execution: options

For the "keep state across async interactions" goal — specifically, writing sequential-
looking code that implicitly waits at async boundaries — there are several options:

### Option A: Callbacks (no coroutines)

```mms
query("#hero", fn(hero) {
    hero.set_rgba(0, 1, 0, 1)
    wait(2.0, fn() {
        hero.set_rgba(1, 0, 0, 1)
    })
})
```

**Pros:** no language changes; works with current evaluator architecture.
**Cons:** callback pyramid for sequences; state must be captured in closures, not local vars.

This is the minimum viable approach. Already almost expressible with existing syntax.

### Option B: Generator functions (`yield`)

```mms
fn* hero_sequence() {
    let hero = yield query("#hero")
    hero.set_rgba(0, 1, 0, 1)
    yield wait(2.0)
    hero.set_rgba(1, 0, 0, 1)
}
```

A generator function (`fn*`) returns a generator object. The host drives it:

```
session.spawn_generator(hero_sequence())

each tick:
  gen.resume(event_or_null)
  → runs until next yield
  → yields a "request" (query, wait, etc.)
  host satisfies the request
  → next tick: gen.resume(result)
```

**Evaluator implementation:** `StmtEffect::Yield(value)` bubbles up through
`eval_block_stmts`, the session runner catches it, saves the paused generator state
(env + "position in body"), resumes it next tick with the host-provided result.

The "position in body" is the index into `body.statements` — for generators that only
`yield` at the top level (not inside helper functions), this is a simple integer. For
nested yields (inside loops, etc.), a full continuation stack is needed.

**This is the same model Lua coroutines use** — widely proven in game scripting. Very
ergonomic for timed sequences and async queries.

**Pros:** sequential-looking code; no callback nesting; state lives in local vars naturally.
**Cons:** requires a new AST node (`fn*`), new `Value::Generator`, and yield-point tracking
in the evaluator. Non-trivial but not a full redesign.

### Option C: `async/await`

```mms
async fn setup() {
    let hero = await query("#hero")
    await wait(2.0)
    hero.set_rgba(1, 0, 0, 1)
}
```

Syntactically cleaner than generators. Semantically: `async fn` returns a future-like
value; `await` suspends the function until the future resolves.

**Evaluator implementation:** requires a continuation-passing transform or a full
coroutine stack. The tree-walking evaluator cannot do this without major redesign —
`await` inside a nested helper function means the entire call stack must be serializable.

**Pros:** most ergonomic; familiar to JS/Rust/Python developers.
**Cons:** the implementation cost is high. Full CPS transform or coroutine support
(basically reimplementing the evaluator as a state machine).

**Recommendation:** target this eventually, implement generators first as a stepping stone.
`async fn` could desugar to `fn*` + a scheduler, making generators the primitive.

### Option D: implicit yield at every HostCall

The script looks fully sequential, but HostCalls implicitly suspend:

```mms
// no special syntax
let hero = query("#hero")        // implicitly suspends, resumes with result
hero.set_rgba(0, 1, 0, 1)
wait(2.0)                        // implicitly suspends for 2 ticks
hero.set_rgba(1, 0, 0, 1)
```

This is the most ergonomic. Implementation: the evaluator is a coroutine that yields to
the host at each `Value::HostFn` dispatch. Requires the evaluator to be a proper Rust
async task or green thread (not just a Rust call stack).

**Pros:** no special syntax at all.
**Cons:** requires rearchitecting the evaluator — it can no longer be a plain recursive
tree-walker. Significant implementation investment.

---

## Practical path forward

In order of implementation effort, smallest first:

### Shared state across requests

Session-level bindings in the env persist across all handler invocations — this is the
primary mechanism for shared mutable state between requests:

```mms
let score = 0        // lives in session env
let lives = 3

on("enemy_killed", fn(e) { score = score + e.points })
on("player_died",  fn(e) { lives = lives - 1 })
```

For structured state, a struct with public fields works the same way:

```mms
let state = AppState { score: 0, lives: 3 }

on("enemy_killed", fn(e) { state.score = state.score + e.points })
```

Field visibility (`pub` / `private`) determines whether other modules can read or write
session state — see [structs.md](structs.md) for the field visibility design.

---

### Step 1 — Persistent session (no async yet)

Add `MeowMeowSession` that wraps a long-lived evaluator thread. The session can:
- Run a setup script (scene construction, handler registration)
- Accept `fire_event(name, args)` calls from the host — the session dispatches to
  registered MMS handlers and returns intents
- Maintain persistent env + heap across all event firings

Event handlers run synchronously and return immediately (no suspension). HostCalls still
block with spin-wait. This is already almost expressible with the current ring-buffer
architecture — just don't shut down the evaluator thread after the first script.

### Step 2 — Generator functions (`fn*` / `yield`)

Add generator support to the evaluator. Generators are the right primitive for:
- Timed sequences (`yield wait(n)`)
- Async queries (`let x = yield query("#id")`)
- State machines (character behaviors, cutscene logic)

The host session runner drives generators by calling `resume(value)` each tick.

### Step 3 — `async/await` sugar over generators

Once generators exist, `async fn` / `await` can desugar to `fn*` / `yield`. The scheduler
that drives generators is already in place — async functions are just generators with a
different surface syntax.

---

## Event dispatch model for sessions

When the host fires an event into a session:

```
host: session.fire("clicked", { target: component_id })

session:
  1. look up registered handlers for "clicked" in env
  2. for each handler (in registration order):
     a. call handler with event args (run to completion)
     b. collect any emitted intents
  3. return collected intents to host
  4. host applies intents (process_commands)
```

Run to completion means: one handler fully executes before the next starts. No
re-entrancy. If a handler fires an event internally (`emit(Clicked { ... })`), that
event is queued and dispatched after the current handler finishes — not recursively
within it. This mirrors the engine's own "follow-up events deferred" rule.

### Tick integration

The most natural integration with cat-engine:

```
engine tick:
  1. collect input events
  2. fire_event(session, "pre_tick", { delta })
  3. process engine systems (physics, animation, etc.)
  4. fire_event(session, "tick", { delta })
  5. apply intents from session handlers
  6. render
```

This gives MMS scripts two hooks per tick: before and after engine systems. Additional
hooks (collision, gesture, timer) fire at the appropriate drain points.

---

## Open questions

1. **Session per scene or per component?** One global session, or one session per
   component tree? Per-component sessions would isolate state but complicate
   cross-component communication.

2. **Thread model for sessions** — long-lived thread? Rust `async` task? Green thread?
   The choice affects how HostCall suspension is implemented.

3. **Error handling in event handlers** — if a handler errors, does it: (a) log and
   continue to next handler, (b) abort all handlers for this event, (c) crash the session?
   Probably (a) for robustness.

4. **Generator scheduling** — who drives generators that aren't tied to a specific event?
   Background generators (timed sequences, behavior loops) need a scheduler that resumes
   them at the right tick.

5. **Cross-session communication** — can two MMS sessions communicate? Probably via
   the engine's event system (fire an engine event from one session, another handles it).
   Direct session-to-session messaging seems like overengineering.

6. **`async fn` as a transpiler target** — if the transpiler emits Rust `async fn`, the
   evaluator's generator model maps cleanly. If it emits synchronous Rust, `await` becomes
   a compile error for baked targets. This is the same "deployment determines the host
   interface" principle from function-dispatch.md.
