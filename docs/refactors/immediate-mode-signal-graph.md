# Immediate-mode signal graph (proposal + exploration)

Goal: allow systems to react to signals **immediately after they are emitted** (within the same frame), without “peeking” into `RxWorld` via `rx.signals()`.

This doc started as a pre-refactor proposal. As of early 2026, parts of it are now implemented; the rest of this document explores how to scale immediate-mode dispatch to more complex “systems calling systems” behavior.


## Status (as of 2026-03)

Already implemented (current behavior):

- `RxWorld` supports immediate-mode incremental dispatch via a cursor (`dispatch_new_signals(...)`).
- `SystemWorld::tick` has explicit dispatch points so signals can be handled within the same frame.
- `GestureSystem` is (at least partially) handler-driven: it installs a handler for `RayIntersected` and caches the best hit instead of scanning `rx.signals()`.

This means we now have a working “phase-based immediate dispatch” model.


## Why

Today, several systems do a two-phase pattern:

- **Emit** signals into `RxWorld` (e.g. `RayCastSystem` emits `EventSignal::RayIntersected`).
- **Consume** signals by scanning `rx.signals()` later in the same frame (e.g. `GestureSystem` finds the closest `RayIntersected`, `GizmoSystem` correlates `RayIntersected` + `DragStart`).

This “read the signal buffer” pattern has a few downsides:

- It couples consumers to `RxWorld` internal storage and encourages “global scans”.
- It encourages out-of-band ordering assumptions ("RayCast ran earlier so its signals should exist").
- It makes it harder to evolve toward a proper **signal graph** where signals can trigger downstream work *as they happen*.

We want:

- **Immediate delivery**: a system can register a handler for `RayIntersected` and receive it right away.
- **Multiple dispatch passes per tick**: update the signal graph one or more times per frame without requiring every consumer to manually scan.
- **Cleaner future refactors**: e.g. removing `cast_requests` by making raycast requests purely a signal/action-driven thing.


## Current architecture (baseline)

Files:
- `src/engine/ecs/rx/rx_world.rs` (`RxWorld`: stores `signals: Vec<Signal>`, `handlers`, and dispatches on drain)
- `src/engine/ecs/rx/signal.rs` (Signal types)
- `src/engine/ecs/system/system_world.rs` (tick order + `process_commands` drain)

Key properties:

- `RxWorld::push(scope, value)` appends signals.
- Signals are drained in `SystemWorld::process_commands` (end-of-frame, after `tick`).
- With immediate-mode enabled, signals may be dispatched during `tick`, and `process_commands` only dispatches any remaining undispatched signals before draining.
- During `tick`, systems can look at the current frame signal buffer (`rx.signals()`).
- `dispatch_handlers` uses `scope` and walks ancestry; handlers can be attached at any scope root.


## Proposed change: immediate-mode dispatch phases

### Core idea

Treat signal processing as a graph that can advance in phases:

- Phase A: Producers emit signals.
- Phase B: Dispatch handlers for signals emitted so far.
- Repeat A/B multiple times if needed.

This implies a new concept:

- **Immediate dispatch** of signals *during* `SystemWorld::tick`, not just at end-of-frame.

### What we explicitly want to avoid

- Consumers manually scanning `rx.signals()` for specific event kinds.

Instead:

- Consumers register a handler function for `SignalKind::RayIntersected` (and/or `DragStart`, etc.).
- When `RayCastSystem` emits `RayIntersected`, the handler is dispatched immediately.


## GestureSystem as an example consumer

### Previously

- `GestureSystem::tick_with_rx(...)` scanned `rx.signals()` for `RayIntersected`.
- It picked the closest hit and decided whether to emit `DragStart`/`DragMove`/`DragEnd`.

### Current / target direction

- `GestureSystem` registers handlers on startup or when its pointer is registered:
  - `SignalKind::RayIntersected` → updates an internal “best hit this frame” state
  - optionally: `SignalKind::DragEnd` or pointer-lifecycle signals
- On each tick, `GestureSystem` runs a lightweight “advance gesture state” step using only its internal cached hit info + `InputState`.

This removes the need to scan the `RxWorld` buffer.


## Where to place the immediate dispatch in the frame

There are (at least) two plausible choices.

### Option 1: explicit dispatch points in SystemWorld

In `SystemWorld::tick`, add explicit “dispatch now” steps:

- `raycast.tick_with_queue(...)`
- `rx.dispatch_new_signals(world, queue)`  ← new
- `gesture.tick(...)` (now fed by handlers rather than scanning)
- `rx.dispatch_new_signals(world, queue)`  ← optional
- `gizmo.tick(...)`
- `rx.dispatch_new_signals(world, queue)`  ← optional

This preserves a clear deterministic order and is easy to reason about.

### Option 2: RxWorld dispatches inline on push

Make `RxWorld::push(...)` optionally dispatch immediately.

This is attractive, but is riskier:

- It creates “action at a distance”: pushing a signal can now mutate world/queue via handlers.
- Borrowing gets harder (a system might be holding a mutable borrow when it pushes).

Because Rust borrowing is a hard constraint, **Option 1** is likely safer.


## New question: can we avoid manual batch dispatch between systems?

The explicit-dispatch approach works, but it has a scaling problem:

- If you want a “signal graph” where many systems can trigger work in other systems, potentially *recursively* a few times in one frame, then inserting explicit `dispatch_new_signals(...)` calls “between individual systems” becomes brittle.

There are a few ways to evolve this without giving up determinism.


## Option 3: pump the signal graph to quiescence (one call site)

Instead of sprinkling dispatch calls throughout `SystemWorld::tick`, introduce a single concept:

- `rx.pump(world, queue, limits...)` runs until there are no undispatched signals left.

Conceptually:

- Signals are still emitted into a buffer.
- Dispatch is still *iterative* (not recursive), so handlers can emit more signals safely.
- The pump loop continues until the signal queue reaches a fixed point (quiescence).

### Why this helps

- You can call `pump()` once per frame (or once after “input + producers”), and it will propagate the signal graph as far as it can go.
- If a handler emits more signals, they get handled within the same pump.
- You don’t need to predict where to place “batch boundaries” between systems.

### The critical caveat

Pumping only dispatches *handlers*. If some system requires doing work in its `tick()` (rather than in a handler) to produce new signals, then pumping alone can’t “re-run” that tick.

So for “systems calling systems recursively”, you either:

1) Move more system logic into handlers (reactive systems), or
2) Add a way for handlers to schedule additional system passes (see Option 4).

### Determinism + safety

To keep behavior stable:

- Dispatch order stays FIFO by signal emission order.
- Handler order stays well-defined (e.g. global handlers first, then scoped ancestry, and within each list: insertion order).
- Add a hard cap: `max_signals_per_frame` (and/or `max_pump_iterations`) to prevent infinite loops.


## Option 4: a unified “work queue” (signals + system passes)

If you truly want “systems can call each other recursively a few times in one frame”, the engine needs a scheduler concept.

Represent *work* as a queue of items:

- `WorkItem::Signal(Signal)`
- `WorkItem::SystemPass(SystemId)` (run one incremental step for a specific system)
- (optionally) `WorkItem::WorldOp(...)` (see CommandQueue replacement section)

Then the frame becomes:

1. Seed the queue with initial work (input events, maybe `SystemPass::Raycast`, etc.)
2. `while queue not empty`:
  - pop front
  - execute it (which may push more work)
3. Stop when quiescent or when safety limits are hit

This supports recursion without actual recursion: the graph unfolds iteratively.

### How systems would participate

Systems become closer to actors:

- Their handlers react to signals.
- If they need to do multi-step work, they enqueue `SystemPass(self)` again.
- They can also enqueue passes for other systems if that’s part of the design (though ideally signals remain the coupling mechanism).

This also lets you avoid global frame ordering assumptions. Ordering becomes “who schedules what work when”, which is explicit and debuggable.


## Option 5: dispatch-on-emit (synchronous) with re-entrancy guard

This is the “tempting” model:

- `rx.push(...)` immediately dispatches handlers for that signal.

This naturally enables “recursive” propagation, but there are two big issues:

1) **Rust borrowing**: a system may be holding a borrow into the world/components when it emits.
2) **Re-entrancy**: handlers emitting signals can cause deep call stacks and confusing control flow.

If we ever do this, it should likely be implemented as:

- `rx.emit(...)` pushes onto a FIFO queue
- if not currently dispatching, enter a loop to drain the queue
- if already dispatching, just enqueue (no recursion)

So it still ends up as “pump until quiescence”, just triggered automatically.


## Choosing a direction

- If we want to keep the current architecture intact: prefer **Option 3** (explicit `pump()` call once or twice per frame).
- If we want a true “signal graph that can iterate a few times in one frame”: move toward **Option 4** (a unified work queue / scheduler).
- Avoid pure dispatch-on-emit unless we can prove borrowing + re-entrancy are controlled.


## Supporting “one or more times per frame” updates

The simplest model is a small dispatcher that can run multiple passes:

- Maintain an index `dispatched_up_to` into the `signals` vec.
- A `dispatch_new_signals(...)` method dispatches only the new suffix.

Potential semantics:

- “Dispatch pass” means: for each newly pushed signal, walk scope chain and fire handlers.
- Signals remain stored until end-of-frame drain (or until explicitly cleared).

This allows:

- Raycast emits `RayIntersected`
- Dispatch immediately
- Gesture handler runs and pushes `DragStart`
- Dispatch immediately
- Gizmo handler runs and queues transform update


## Handler registration lifecycle

Questions to answer (in a later code change):

- Who owns handler registration?
  - `SystemWorld` could register system-level handlers once at initialization.
  - Or systems can register handlers when certain components exist.

- How do we attach handlers at the right scope root?
  - For interaction, most of these should likely be attached at a stable scope like:
    - a “universe root”, or
    - the pointer/raycaster subtree root, or
    - the renderable hit scope (but those are dynamic and less stable).

- Do we need handler priorities?
  - If multiple systems listen to the same signal kind (e.g. `RayIntersected`), ordering may matter.
  - This could be avoided by splitting signals into more specific kinds or by explicit dispatch points.


## Important constraint: World mutation + CommandQueue

In the current engine, most world mutations should go through `CommandQueue`, with flush points.

Immediate dispatch implies:

- Handlers may enqueue commands earlier in the frame.
- We must decide whether to `queue.flush(...)` between dispatch phases.

A conservative approach:

- Keep queue flush points explicit (as they are today).
- Immediate signal dispatch should *enqueue* work, not apply it.
- Flush remains controlled by `SystemWorld`.


## CommandQueue: can we replace it with engine-handled action signals?

It’s plausible, but the CommandQueue exists for a reason: it centralizes world mutation at safe points, and helps avoid “mutate while iterating” hazards.

That said, there’s a compelling direction:

- Treat *commands* as just another kind of signal: “engine actions”.
- When an engine action is emitted, the engine applies it (either immediately within a pump, or at a flush point).

### A pragmatic migration path (incremental)

1) Introduce a dedicated signal/value family, conceptually:
  - `EngineAction::UpdateTransform { ... }`
  - `EngineAction::RegisterRenderable { ... }`
  - etc.

2) Install an **engine handler** (or a small system) that listens for these actions and performs the effect.

3) Initially, the handler can simply translate engine actions into existing `CommandQueue` ops (so we keep mutation safety).

4) Later, if we want to actually remove CommandQueue:
  - Replace it with a `WorldOp` queue processed by the scheduler/pump at safe times, OR
  - Evolve the World API to support safe immediate mutation patterns (this is harder and may require interior mutability or a more borrow-aware ECS storage model).

### Why this is nice

- Unifies the mental model: “everything is a signal/event.”
- Better debug tooling: you can trace both user-facing events and engine-facing actions through the same graph.
- Removes a split-brain architecture where some things are signals and other things are “secret commands.”

### The hard part

Even if commands become signals, you still need a policy for **when** they apply:

- Apply immediately during pumping (strong immediacy, but increases re-entrancy concerns).
- Apply only at explicit flush points (keeps current safety/determinism).

This decision couples tightly with the dispatch strategy:

- With Option 3 (pump signals only), you likely still want explicit world-op flush points.
- With Option 4 (unified work queue), world ops become just another work item and can be scheduled deterministically.


## Interaction with the planned raycast refactor (cast_requests)

This doc is a prerequisite for refactoring away `RayCastComponent.cast_requests`.

The direction we want:

- Raycast requests are represented as signals/actions (e.g. `ActionSignal::Action(ActionMethod::Raycast)` or an equivalent request signal).
- Raycast system listens to those requests via handlers, rather than reading component state.

Immediate-mode dispatch makes this much cleaner because:

- An animation can emit a “Raycast request” action.
- Raycast system receives it immediately and performs a cast in the same frame.
- Downstream gesture logic can receive `RayIntersected` immediately.


## Open questions / design risks

- Re-entrancy: what if a handler emits a signal of the same kind? Do we allow nested dispatch or require a pass-based loop?
- Determinism: do we guarantee a stable order of handler calls across scopes?
- Performance: immediate dispatch could increase overhead vs a single end-of-frame batch.
- Debugging: immediate mode can be harder to inspect; we likely want a debug trace mode that logs “signal → handler” edges.

Additional risks if we pursue “recursive system calling”:

- Infinite loops: two systems can bounce signals forever; we need hard caps + diagnostics.
- Fairness/starvation: if one producer floods the queue, others may never run.
- “Tick vs handler” split: decide what belongs in handlers vs scheduled passes.


## Proposed next step

With Option 1 (explicit dispatch points) implemented, the next decision is whether we want to evolve toward:

1. A single `pump()` call to reach quiescence (Option 3), or
2. A unified work-queue scheduler that can re-run system passes (Option 4).

In parallel, if we want to explore removing `CommandQueue`, start by representing commands as engine-handled action signals (an adapter over the existing queue), then iterate.

