
# Signals: `{ actions, events }`

This doc describes the engine’s “signals-first” layer.

## Goal

Reframe engine reactivity as a single concept: **signals**.

- **Actions** are *input-ish* signals (intent / requests / commands).
- **Events** are *output-ish* signals (facts / observations / consequences).

Both are still “signals” because:
- they flow through the same scoping + dispatch machinery
- they can be recorded/replayed
- they can be bridged (Action → mutations → Event)

Implementation note: the current codebase uses `engine::ecs::rx::RxWorld` + `Signal`.

---

## Terminology

### Signal
A typed message delivered to listeners.

Properties we want:
- **Scoped**: delivered to listeners registered at an ancestor scope root.
- **Deterministic**: processed in ordered phases.
- **By value**: payloads are small, cloneable (ComponentId + small enums + numbers).

### Action signal
A signal representing intent.

- Usually produced by: animation keyframes, input, tools/REPL, gameplay logic.
- Usually consumed by: an `ActionSystem` (or “ActionExecutor”) that mutates the world.

### Event signal
A signal representing a fact.

- Usually produced by: mutation points (topology changes), systems (ray hit), etc.
- Usually consumed by: gameplay logic / scripts / debug tools / derived-index updaters.

---

## Why unify as Signals?

The current model already has these properties for events:
- scoped listeners
- deterministic drain + dispatch

But conceptually we have *two* reactive things:
- intent (actions)
- facts (events)

Putting both under “signals” makes the engine easier to reason about:
- “Where do reactive messages live?” → in one place.
- “How do I listen?” → one handler API.
- “How do I record/replay?” → one stream.

---

## Proposed structure

### Types

Today the engine represents signals roughly like this (see `engine::ecs::rx`):

```rust
pub struct Signal {
    pub scope: ComponentId,
    pub value: SignalValue,
}

pub enum SignalValue {
    Action(ActionSignal),
    Event(EventSignal),
}

pub enum ActionSignal {
    Action(Action),
}

pub enum EventSignal {
    ParentChanged {
        child: ComponentId,
        old_parent: Option<ComponentId>,
        new_parent: Option<ComponentId>,
    },
    RayIntersected {
        raycaster: ComponentId,
        renderable: ComponentId,
        t: f32,
        origin: [f32; 3],
        dir: [f32; 3],
    },
    CollisionStarted { a: ComponentId, b: ComponentId, delta: [f32; 3] },
    CollisionEnded { a: ComponentId, b: ComponentId, delta: [f32; 3] },
}

pub enum SignalKind {
    Any,
    Action,
    ParentChanged,
    RayIntersected,
    CollisionStarted,
    CollisionEnded,
}

type SignalHandler = fn(&mut World, &mut CommandQueue, &Signal);
```

Notes:
- `SignalKind` exists so the handler registry doesn’t need to do pattern matching.
- `Signal` does not store the kind; it’s derived by calling `signal.kind()`.

## Why fn pointers (not closures) for handlers?

Using `fn(&mut World, &mut CommandQueue, &Signal)` buys a few pragmatic wins:

- **Simple storage + identity**: a function pointer is `Copy` and comparable by address, so `remove_signal_handler(kind, scope, handler)` is trivial.
- **No allocation / no trait objects**: avoids `Box<dyn Fn...>` and dynamic dispatch in the hot path.
- **No lifetime/capture complexity**: closures want captured environment, which quickly forces handler registries to be generic over lifetimes or to heap-allocate captured state.
- **State lives in the ECS**: if a handler needs state, store it as components/resources under its `scope_root` and look it up from `World` when the handler runs.

When we eventually need closure ergonomics (editor UI callbacks, scripting), a common evolution is:

- `add_signal_handler(...) -> HandlerId` and `remove_signal_handler(HandlerId)`
- internally store `Box<dyn FnMut(...) + 'static>` (or `Fn`) keyed by the ID

That’s a larger surface-area change, so starting with fn pointers keeps the system small and deterministic.

### RxWorld

The bus that stores + dispatches signals:

```rust
pub struct RxWorld {
    signals: Vec<Signal>,
    handlers: HashMap<SignalKind, HashMap<ComponentId, Vec<SignalHandler>>>,
}

impl RxWorld {
    pub fn push(&mut self, scope: ComponentId, value: impl Into<SignalValue>) { ... }
    pub fn drain(&mut self) -> Vec<Signal> { ... }
    pub fn add_handler(&mut self, kind: SignalKind, scope_root: ComponentId, h: SignalHandler) { ... }
    pub fn dispatch_handlers(&mut self, world: &mut World, queue: &mut CommandQueue, signal: &Signal) { ... }
}
```

Dispatch semantics:
- handlers are keyed by `(SignalKind, scope_root)`
- when dispatching a signal scoped at `S`, handlers attached to `S` and any ancestor of `S` are invoked

---

## Where phases live today

There is no separate `ReactiveWorld` type right now. In practice:
- systems push signals into `SystemWorld::rx` (`RxWorld`)
- command flushing + signal dispatch is centralized in `SystemWorld::process_commands`

### Frame phases (sketch)

A deterministic frame looks like:

1. **Systems tick** (may queue commands and/or push signals)
2. **Flush commands** (`CommandQueue::flush`)
3. **Drain + dispatch signals** (`RxWorld::drain` then `dispatch_handlers`)
4. **Flush commands again** (handlers may have queued commands)

The key is to avoid re-entrancy:
- handlers shouldn’t directly mutate the world; they should enqueue commands (or future “intent” signals)
- mutations happen via the command queue flush

---

## Where does this live?

### Option A: `engine::rx`
Pros:
- clean separation: ECS stays “storage + systems”, RX stays “reactivity + signals”.
- makes it easy to later plug other worlds into rx (UI graph, audio graph, editor graph).
- clearer naming: `SignalWorld` doesn’t feel ECS-specific.

Cons:
- more cross-module wiring (`engine::rx` needs to know about `ecs::World`, `CommandQueue`, `ComponentId`).
- might feel weird if rx becomes “just for ECS anyway”.

### Option B: `engine::ecs::rx` (current)
Pros:
- tighter cohesion with `World`/`ComponentId` scoping.
- fewer dependency edges.

Cons:
- naming mismatch: “rx” becomes ECS-only.
- future non-ECS reactive streams will either copy or depend on ECS.

### Practical recommendation
Keep it in `engine::ecs::rx` while scoping semantics are ECS-specific. If you later want reactive streams for non-ECS graphs (UI/editor/audio), extract a generic layer.

---

## Should Action execution live in RX?

### Option 1: keep `ActionSystem` and “fact producers” in ECS systems, and keep RX as just the bus
- RX owns: `RxWorld` and dispatch semantics
- ECS owns: actual producers/consumers

Pros:
- minimal refactor
- keeps systems self-contained

Cons:
- mental split: “reactivity” partly in systems, partly in rx

### Option 2: make action execution an explicit RX stage
- RX owns: `ActionExecutor`
- systems produce `SignalValue::Action(ActionSignal::Action(_))`; RX executes actions in a dedicated phase

Pros:
- extremely clear pipeline: produce signals → execute → emit facts → dispatch
- easier to record/replay actions deterministically

Cons:
- bigger refactor
- risk of over-centralizing logic (RX becomes a god-object)

### Middle ground
Keep `ActionSystem` where it is, but treat it as an “executor” invoked by RX:

- a helper that drains action signals, executes them, then proceeds to “fact” dispatch

This makes the pipeline explicit without relocating all code.

---

## How does this map to the current code?

Current reality:
- `ecs::RxWorld` already provides scoped queue + dispatch.
- `ActionSystem` delegates the core execution logic to `ecs::rx::ActionExecutor`.
- Some systems emit fact signals (e.g. raycast hits, collisions).
- `SystemWorld::process_commands` flushes → drain+dispatch → flush.

Possible next steps (optional):
1. Make “intent vs fact” phases explicit (even if both still use `RxWorld` underneath).
2. Decide whether action signals should be dispatched to handlers, or executed directly by an executor.
3. Add tooling: tracing/logging, record/replay, and debugging UI.

---

## Open questions / design constraints

- Do we want handlers to be `fn` pointers only, or allow closures?
  - `fn` pointers are simple + stable.
  - closures would be nicer for gameplay code but require lifetimes + storage management.

- Do action handlers get access to a safe API (emit more action signals), not direct world mutation?

- Do we want “Any” kinds for both Action and Event?
  - Useful for logging / debugging.
  - Might be too spammy in practice; could be behind a feature flag.

---

## Notes on evolution

If you want to push the architecture further, preserve:
- scoping behavior
- deterministic ordering
- the extra command flush after dispatch (handlers often enqueue visual/system work)
