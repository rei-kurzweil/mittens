# Commands vs Signals (and where “actions” belong)

This doc is an exploration of whether we’d gain anything by **merging actions with commands** (instead of treating actions and facts as one unified signal stream), and how that interacts with:

- mutation barriers / “when can the world change?”
- recursion / feedback loops
- per-frame propagation
- push vs pull overhead

It’s grounded in the current code:

- Signals: `src/engine/ecs/rx/*` (especially `RxWorld` and `SignalValue`)
- Command queue: `src/engine/ecs/command_queue.rs`
- Frame phases: `src/engine/ecs/system/system_world.rs`

---

## Current architecture (today)

### Two transports

The engine currently has *two* message-like mechanisms:

1) **`CommandQueue`**: a concrete queue of imperative operations (“do X in a system / visual world”).

- It’s used heavily for registrations + incremental updates: transforms, renderables, textures, text rebuild, etc.
- `CommandQueue::flush(...)` drains commands until empty (multi-pass), with a hard cap (1000 passes) to avoid infinite loops.

2) **`RxWorld` signals**: a scoped signal stream with handler dispatch.

- `SignalValue` is a single enum that contains both:
  - “action-ish” variants (side effects) which map to `SignalKind::Action`
  - “fact-ish” variants (ParentChanged, RayIntersected, DragMove, …)
- `RxWorld::dispatch_new_signals(...)` is immediate-mode: it dispatches signals pushed since the last dispatch, using an internal cursor.
- Handlers can emit follow-up signals via a restricted `SignalEmitter` (see `docs/signal-emitter.md`).

### Frame phases / explicit propagation points

`SystemWorld::tick(...)` uses explicit “barrier” points where it flushes commands and/or dispatches newly pushed signals. Example pattern:

- run something that enqueues updates
- `queue.flush(...)` so caches/visuals are updated
- run something that pushes signals (raycast, gesture, animation)
- `rx.dispatch_new_signals(...)` so downstream consumers react *this frame*

Separately, `SystemWorld::process_commands(...)` does an end-of-frame style cleanup:

- flush any remaining commands
- dispatch any remaining undispatched signals
- drain signals and reset per-frame dispatch cursor
- flush again (handlers may have queued commands)

### What “actions” are right now

Actions are *not* a separate type. They’re just `SignalValue` variants that return `SignalKind::Action`.

`ActionSystem` installs a **global handler** for `SignalKind::Action` that:

- mutates `World` directly in some cases
- enqueues commands to `CommandQueue` (e.g. to re-register a Color component)
- emits fact-ish signals as follow-ups (e.g. `ParentChanged` after `Attach`)

This means:

- “actions” and “facts” use the same dispatch machinery (scoping, ordering, recording)
- but “actions” *also* cross the mutation boundary (they mutate `World`)

---

## What problem are we trying to solve?

The motivation for “merge actions with commands” is usually one (or more) of:

1) **Stronger mutation barriers**

> Ideally, the world only mutates at well-defined points.

Right now, signal handlers (including action handlers) receive `&mut World`, so the world can mutate during signal dispatch. That’s still deterministic, but it dilutes the mental model of “commands are the mutation barrier.”

2) **Taming recursion / feedback loops**

Both systems can loop:

- `RxWorld`: handler emits signals; dispatch can keep consuming appended signals in the same `dispatch_new_signals(...)` call.
- `CommandQueue`: flushing can enqueue more commands; `flush(...)` keeps draining until empty.

Both have caps (`max_signals` per dispatch call; 1000 passes for command flush), but the semantics differ.

3) **Reducing per-frame propagation overhead**

Signals are push-based dispatch with per-signal scope-chain walk.

Sometimes it’s tempting to replace “push signals to listeners” with “pull by scanning” (e.g. systems scan for changed components), but that can trade dispatch overhead for O(N) scanning.

---

## Option A: keep unified signals (status quo), but clarify the rules

This is the smallest-change option: **keep actions as signals**, but adopt stricter conventions.

### A.1 Convention: “facts don’t mutate”

- Fact-ish signal handlers should ideally be read-only w.r.t. `World`.
- If they need to cause mutation, they should:
  - push an action signal, or
  - enqueue commands (depending on which model we want long-term)

Today the type signature doesn’t enforce that; it’s a social / code-review rule.

### A.2 Make action execution feel like a phase

Even without changing types, we can *treat* action dispatch as a phase:

- dispatch actions
- flush commands
- dispatch facts

The engine already does something like this around animation (dispatch then flush), but not as a universal pattern.

### Pros

- Minimal refactor; preserves scoping + replayability of “actions.”
- Keeps the “signals-first” model: everything reactive is a signal.

### Cons

- Mutation barrier remains soft: handlers can mutate.
- It’s easy to accidentally create world mutation during “fact reaction,” which can be surprising.

---

## Option B: merge Actions + Commands (“commands are intent; signals are facts”)

In this model:

- **Commands** are the *only* thing allowed to mutate the world.
- **Signals** are facts/observations derived from mutations and systems.

### What this implies structurally

- Replace (or de-emphasize) `SignalValue` action variants with command variants.
- `ActionSystem` becomes either:
  - a compatibility layer that converts “action requests” into commands, or
  - goes away entirely if all producers enqueue commands directly.

### How to preserve scoping

Signals have a `scope` and scoped handlers are a major win.

If commands replace actions, you likely still want *some* notion of scope, for at least:

- provenance (what subtree/tool caused the command)
- selective logging / recording
- local tool behaviors

Two plausible shapes:

- **Scoped commands**: store `{ scope, command }` in the command queue.
- **Unscoped commands + scoped facts**: commands execute globally, but they emit scoped fact signals for observers.

The latter is simpler but loses “subtree-local command handling.”

### Recursion / feedback loops

This option can make recursion easier to reason about:

- Commands mutate in a known phase.
- They emit facts.
- Facts can enqueue more commands, but those commands won’t execute until the next command phase / flush point.

That’s a more explicit loop boundary than the current “signals can dispatch immediately and mutate now.”

### Pros

- Strong mutation barrier: mutations happen in `CommandQueue::flush` (or equivalent).
- Easier to reason about ordering and “what changed when.”
- Better foundation for future parallelism (if we ever want it): observe → decide → mutate.

### Cons

- Larger refactor: many places that currently `rx.push(SignalValue::SetTransform { ... })` would need to enqueue commands.
- Command queue is currently not scoped; adding scope changes the API.
- Some uses of “actions as signals” are ergonomic (record/replay, tool scripting).

### Implementation sketch (if we ever try it)

- Introduce a new command enum that covers the current action surface area.
- Move the mutation logic out of `ActionSystem` and into command execution.
- Have command execution emit fact signals (needs access to `RxWorld` / `SignalEmitter`).
- Keep the existing `SignalEmitter` pattern to avoid handler-map mutation hazards.

---

## Option C: merge Commands into Signals (“signals are everything”)

This means eliminating (or mostly eliminating) `CommandQueue` and representing what it does today (register/update/remove/apply work) as **command-like signal variants**.

In your suggested shape, the old command-queue responsibilities become “mutating signals” handled automatically by `ActionSystem` (or an ActionSystem-like global executor). Initially this is likely **global handling** (no meaningful scoping behavior yet), matching how other mutating signals work today.

### What “command-signals” would look like

Conceptually:

- Add `SignalValue` variants for the things `CommandQueue` currently represents:
  - register/remove renderable
  - register/update/remove transform
  - register camera, UV, texture, filtering, emissive, etc.
  - text rebuild / set text

Then producers stop doing `queue.queue_register_*` and instead do something like:

- `rx.push(scope, SignalValue::RegisterRenderable { component_id })`
- `rx.push(scope, SignalValue::UpdateTransform { component_id, transform })`

The execution model becomes:

1) producers push command-signals
2) a global executor drains/executes them
3) execution may emit fact signals

### Where the work actually runs (important constraint)

Today, `CommandQueue::flush(world, systems, visuals)` is the thing that has access to:

- `&mut SystemWorld`
- `&mut VisualWorld`

But signal handlers currently run with:

- `&mut World`
- `&mut CommandQueue`
- `&mut dyn SignalEmitter`

So if we literally “move commands into signals” and ask `ActionSystem` to execute them, we hit an immediate practical issue:

- many of the old commands *must* touch `SystemWorld` and/or `VisualWorld` (registering renderables, updating GPU-visible state, refitting BVH bookkeeping, etc.)

There are a few ways to resolve that, depending on how pure we want the signals layer to be:

**C1) Expand the handler execution context**

- Change handler signature to receive something like `WorldContext` (or add `&mut SystemWorld` + `&mut VisualWorld`).
- Then the global executor (ActionSystem) can directly call the same methods that `CommandQueue::flush` calls today.

This is the most direct path to “signals-only,” but it grows the handler API and increases the chance that random handlers reach into systems/visuals.

**C2) Add a dedicated “CommandSignalExecutor” system**

- Keep signal handlers as they are.
- Introduce a system (owned by `SystemWorld`) that drains/executes command-signals at explicit barrier points, because systems already have access to visuals.
- `ActionSystem` can remain the executor for *world mutations* (topology edits, component field edits), while command-signals cover the “apply/register” half.

This preserves the current separation in practice (world mutation vs. applying to visuals), but the transport becomes one thing (signals).

**C3) Keep a tiny internal queue (signals as API, queue as implementation)**

- Public API becomes signals.
- Internally, the executor translates command-signals into a compact internal queue for efficiency.

This is effectively “Option C in API terms” while keeping the performance characteristics of the command queue.

### “No scope yet” in a world where signals always have `scope`

Even if we don’t *use* scoping initially, each signal still needs a `scope: ComponentId`.

Two pragmatic conventions:

- For component-local command-signals (register/update/remove), set `scope = component_id`.
- For multi-target mutations, set `scope = first_target` or the subtree root that conceptually owns the operation.

Then install handlers globally (like `ActionSystem` does today for `SignalKind::Action`). Scoping becomes “latent”: available later for tooling/recording without affecting behavior yet.

### Pros

- Truly one transport and one dispatch mechanism.
- Scoped registrations/updates become possible.

### Cons

- Many commands are *hot path* and very frequent (transform updates, registrations). Routing them through handler dispatch would likely be slower than today’s direct system calls.
- Command flushing is currently a tight loop with a clear “drain until empty” semantics; signals are intentionally more general.

Additional cons specific to the “ActionSystem executes old commands” framing:

- Without changing the handler execution context, signal handlers cannot perform the `SystemWorld`/`VisualWorld` work that command flush does today.
- If we *do* expand handler context, we lose some of the nice separation that `SignalEmitter` was protecting (handlers become more powerful, and more likely to entangle systems/visuals in arbitrary callbacks).

In practice this tends to collapse back into “signals for coarse events, queue for fine-grained work.”

---

## Push vs pull overhead

### Push (today)

- Commands are push-based dirtying: callers enqueue explicit “something changed” operations.
- Signals are push-based delivery: producers push messages; consumers run immediately at dispatch points.

Costs:

- Signals: each dispatched signal walks its ancestor chain to find scoped handlers.
- Commands: repeated flush passes can do a lot of work if commands generate more commands.

Benefits:

- Avoids O(N) scans.
- Natural for immediate-mode reactivity (raycast → gesture → gizmo in one frame).

### Pull (hypothetical)

“Pull” typically means: systems scan world state each tick and detect deltas.

Costs:

- Often O(N) per frame, unless backed by indexes/dirty sets (which are… push again).

Benefits:

- Fewer message types.
- Can be simpler to debug in tiny prototypes.

Given this engine’s component graph + desire for immediate-mode propagation, push-based + explicit barriers is usually the right default.

---

## Two dispatch modes for Signals: execute vs observe

One additional axis that’s orthogonal to “commands vs signals” is **dispatch mode**.

Right now signals are:

- queued via `RxWorld::push(...)`
- delivered when the engine calls `dispatch_new_signals(...)` at explicit barrier points

That gives deterministic ordering and lets the engine choose *when* to run handlers.

You proposed a model with two kinds of dispatch:

1) **Handler-based dispatch** (existing): signals are delivered to handlers by kind + scope chain.
2) **Direct dispatch** (new): for action/command-like signals, call a function immediately (like a direct “execute this action now” call), rather than waiting for a later dispatch pass.

The interesting question is whether we can have (2) for responsiveness/ergonomics, while still preserving (1) so that observers can listen — even if they receive the message later.

### Model: “execute now, notify later”

The cleanest conceptual split is:

- **Execution path**: immediate function call that performs mutation / queues apply-work.
- **Observation path**: handler-based propagation that notifies listeners.

In that model, an action/command signal is effectively treated like:

- an RPC/command that must run now (direct)
- plus a log entry that can be replayed/observed (handler dispatch)

This is very close to an event-sourcing intuition: *commands execute*, and *events are the record*.

### What “handlers receive it later” means (semantics)

Yes, handlers can still receive the message later — but we should be explicit about what guarantees they get.

If notification is delayed, then a handler that runs later will observe the world after potentially more mutations have occurred.

That leads to two possible semantics:

**D1) Payload-first semantics (recommended for delayed delivery)**

- Handlers should treat signals as the source of truth.
- The signal payload must contain enough information to react meaningfully without relying on the world still being in the intermediate state.

Examples already in this direction:

- `ParentChanged { child, old_parent, new_parent }` is a complete fact even if the child is reparented again later in the frame.

**D2) World-read semantics (fragile with delayed delivery)**

- Handlers read `World` and expect it to reflect “the state right after this signal.”

This only works if we enforce a strong ordering rule like: “notify immediately after execution and before any other mutations.” If we don’t, handlers become order-dependent in surprising ways.

### Ordering knobs

If we add direct execution, we still get to choose when observation happens. Common options:

- **Notify immediately after execution**: direct call executes, then immediately runs handler dispatch for the corresponding notification signal(s).
  - Most intuitive, but increases re-entrancy risk (handlers can emit more signals; now you’re nested inside a call stack).
- **Notify at next barrier point** (end of phase / end of frame): direct call executes now, but observers run later.
  - Safer against re-entrancy, but requires payload-first semantics.
- **Two-phase within the same frame**: execute now; enqueue notifications; dispatch notifications at the end of the current “mutation phase.”
  - Often a good compromise.

### How this interacts with “signals-only commands” (Option C)

If we go toward Option C (command-signals), the direct dispatch idea becomes especially useful:

- Tool code / REPL could “execute command-signal now” to avoid waiting for a later engine dispatch pass.
- Observers (debug UI, derived indexes) can still subscribe via handler dispatch.

But we still need to resolve the earlier constraint: executing what used to be `CommandQueue` work often needs `SystemWorld`/`VisualWorld` access.

So “direct dispatch” almost implies there is a *non-handler* execution entry point living in `SystemWorld` (or a context bundle), e.g.:

- `systems.execute_command_signal(world, visuals, rx, signal)`

…which can be called either immediately by the producer, or later by a barrier-phase executor.

### Tradeoffs / risks

- **Re-entrancy & recursion**: direct execution makes it easier to accidentally create deep call stacks (action executes → emits → executes → …). The current architecture contains this by batching into explicit dispatch calls with caps.
- **Determinism**: if producers can execute actions immediately from arbitrary places, you can lose the “mutations happen here” mental model unless you enforce “direct execution only allowed during specific phases.”
- **Observer confusion**: delayed notification means handlers should not assume “world state is exactly post-action.” This pushes you toward payload-first facts.

### Practical framing

If we ever implement this, it helps to name the two roles explicitly:

- **Executor**: immediate, function-call style, does the mutation/apply work.
- **Bus**: handler-based, scoped, for observation/derivation.

Even if both are implemented using `SignalValue` as the shared message format, keeping the two responsibilities separate tends to keep the architecture understandable.

---

## Recommendation

If the goal is primarily clarity and determinism:

- Keep the current unified signal model, but adopt a stronger convention:
  - fact-ish handlers should not directly mutate the world
  - mutations should be expressed as action signals (today) or commands (if we migrate)

If the goal is a strict mutation barrier (and potentially future parallelism):

- Merging actions into a command queue is promising, **but only if** we also decide how to preserve scoping (either scoped commands or scoped facts) and how to keep record/replay ergonomics.

A good low-risk experiment would be:

- pick one action family (e.g. transform edits / topology edits)
- represent it as a command path
- keep emitting the same fact-ish signals
- measure how much it simplifies reasoning and how often “immediate mutation” was actually required
