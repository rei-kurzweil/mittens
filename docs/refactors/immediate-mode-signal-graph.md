# Immediate-mode signal graph (proposal)

Goal: allow systems to react to signals **immediately after they are emitted** (within the same frame), without “peeking” into `RxWorld` via `rx.signals()`.

This doc is intentionally *pre-refactor*: it proposes architecture changes and constraints, but makes **no code changes**.


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
- Signals are normally drained and dispatched in `SystemWorld::process_commands` (end-of-frame, after `tick`).
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

### Today

- `GestureSystem::tick_with_rx(...)` scans `rx.signals()` for `RayIntersected`.
- It picks the closest hit and decides whether to emit `DragStart`/`DragMove`/`DragEnd`.

### Target direction

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


## Proposed next step (still no code)

After agreeing on the model (Option 1 vs Option 2, and whether we keep signals buffered until end-of-frame), we can:

1. Add a doc-level spec for `RxWorld::dispatch_new_signals(...)` semantics.
2. Update `SystemWorld::tick` documentation to show the explicit dispatch points.
3. Only then implement the minimal refactor:
   - Gesture consumes `RayIntersected` via handler state, not scanning `rx.signals()`.

