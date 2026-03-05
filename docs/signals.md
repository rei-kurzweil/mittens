
# Signals: one stream, drain points

This is the canonical doc for the engine’s “signals-first” layer.

If you were previously looking for the old events doc: it has been folded into this file.

## Practical examples (start here)

### Example 1: listen for topology changes in a subtree

One built-in structural event signal is `ParentChanged`.

Register a listener on some subtree root (e.g. a character rig root), and you’ll be notified when
any component in that subtree is reparented (attach/detach).

```rust
use cat_engine::engine::ecs;

fn on_topology_change(
    _world: &mut ecs::World,
    _emit: &mut dyn ecs::SignalEmitter,
    signal: &ecs::Signal,
) {
    match &signal.value {
        ecs::SignalValue::ParentChanged {
            child,
            old_parent,
            new_parent,
        } => {
            println!(
                "parent changed: child={child:?} old={old_parent:?} new={new_parent:?} (scope={:?})",
                signal.scope
            );
        }
        _ => {}
    }
}

fn setup(universe: &mut cat_engine::engine::Universe, scope_root: ecs::ComponentId) {
    universe.add_signal_handler(ecs::SignalKind::ParentChanged, scope_root, on_topology_change);
}
```

Key idea: listeners are subtree-scoped by default. You don’t subscribe globally and then filter.

### Example 2: actions are intent, events are facts

When you run an action (from animation keyframes, input, tools, etc), the action is queued as a
signal and executes when the engine drains signals at an explicit drain point.

If that action performs a meaningful state transition (like a topology move), it emits an **event
signal** (a fact).

That means you can:

- use actions for “do something now”
- use events for “something happened; react/derive/cache/log”

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

### Signal handler

A function registered to run when a signal is dispatched.

In the current implementation handlers are plain function pointers:

```rust
type SignalHandler = fn(&mut World, &mut dyn SignalEmitter, &Signal);
```

Note: the public `Universe::add_signal_handler(...)` API takes a `SignalHandler` function pointer.
Internally, `RxWorld` also supports closure-based handlers for engine systems that need state.

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

    /// Whether the engine should execute this signal via the default executor.
    ///
    /// Handlers still run after execution.
    pub immediate: bool,
}

pub enum SignalValue {
    // Intent-ish “actions” (requests).
    SetColor { target: Vec<ComponentId>, rgba: [f32; 4] },
    Attach { parents: Vec<ComponentId>, child: ComponentId },

    // Command/mutation signals (formerly CommandQueue commands).
    UpdateTransform { component: ComponentId, translation: [f32; 3], rotation_quat_xyzw: [f32; 4], scale: [f32; 3] },
    RegisterRenderable { component: ComponentId },

    // Facts.
    ParentChanged { child: ComponentId, old_parent: Option<ComponentId>, new_parent: Option<ComponentId> },
    RayIntersected { raycaster: ComponentId, renderable: ComponentId, t: f32, origin: [f32; 3], dir: [f32; 3] },
}

pub enum SignalKind {
    Any,
    Action,
    ParentChanged,
    RayIntersected,
    CollisionStarted,
    CollisionEnded,
}

type SignalHandler = fn(&mut World, &mut dyn SignalEmitter, &Signal);
```

Notes:
- `SignalKind` exists so the handler registry doesn’t need to do pattern matching.
- `Signal` does not store the kind; it’s derived by calling `signal.kind()`.

`SignalValue` includes both intent-ish requests and fact-ish observations, but the engine also
uses `Signal.immediate` to decide whether a given signal should run through the default executor.

## Why fn pointers (not closures) for handlers?

Using `fn(&mut World, &mut dyn SignalEmitter, &Signal)` buys a few pragmatic wins:

- **Simple storage + identity**: a function pointer is `Copy` and comparable by address, so `remove_signal_handler(kind, scope, handler)` is trivial.
- **No allocation / no trait objects**: avoids `Box<dyn Fn...>` and dynamic dispatch in the hot path.
- **No lifetime/capture complexity**: closures want captured environment, which quickly forces handler registries to be generic over lifetimes or to heap-allocate captured state.
- **State lives in the ECS**: if a handler needs state, store it as components/resources under its `scope_root` and look it up from `World` when the handler runs.

If we want closure ergonomics in the public API (editor UI callbacks, scripting), a common
evolution is:

- `add_signal_handler(...) -> HandlerId` and `remove_signal_handler(HandlerId)`
- internally store `Box<dyn FnMut(...) + 'static>` (or `Fn`) keyed by the ID

That’s a larger surface-area change, so starting with fn pointers keeps the system small and deterministic.

## How scoped dispatch works

Listeners are registered by `(SignalKind, scope_root)`.

Each signal also has a `scope` (the component where the signal is most relevant). To dispatch a
signal scoped at `S`, the engine walks the ancestor chain:

- `S, parent(S), parent(parent(S)), ...`

Any handler registered at any of those nodes will fire.

So:

- a handler on a root sees everything beneath
- a handler on a nested node sees only that subtree

This avoids global scanning: there’s no “iterate all listeners and test predicates” step.

## When handlers run (important)

Signals are processed at explicit drain points driven by `SystemWorld::process_signals(...)`.

Implications:

- execution happens at drain points, not at emission time
- handlers observe during the drain (after the engine’s execution stage)
- handlers can emit follow-up signals via `SignalEmitter`; those signals join the same stream and can be executed/observed in order

Terminology note:

- The current code uses `Signal.immediate`.
- The intended name is **direct mode** (not “immediate”), because it does not mean “run sooner”, it means “execute via a direct-call executor at drain time”.
- v1 goal: have no per-signal direct/immediate flag at all; drain points run a fixed execution stage.

### RxWorld

The bus that stores + dispatches signals:

```rust
pub struct RxWorld {
    signals: Vec<Signal>,
    dispatched_cursor: usize,
    global_handlers: HashMap<SignalKind, Vec<Handler>>,
    scoped_handlers: HashMap<SignalKind, HashMap<ComponentId, Vec<Handler>>>,
}

impl RxWorld {
    pub fn push(&mut self, scope: ComponentId, value: impl Into<SignalValue>) { ... }
    pub fn drain(&mut self) -> Vec<Signal> { ... }
    pub fn add_handler(&mut self, kind: SignalKind, scope_root: ComponentId, h: SignalHandler) { ... }
    pub fn dispatch_handlers(&mut self, world: &mut World, signal: &Signal) { ... }
}
```

Dispatch semantics:
- handlers are keyed by `(SignalKind, scope_root)`
- when dispatching a signal scoped at `S`, handlers attached to `S` and any ancestor of `S` are invoked

---

## Where phases live today

There is no separate “reactive runtime” type beyond `SystemWorld` + `RxWorld`.

In practice:

- Systems and helpers push signals into `SystemWorld::rx` (`RxWorld`).
- The engine processes signals at explicit drain points via `SystemWorld::process_signals(...)`.

If you want a current end-to-end spec of the drain/execution model, see:

- `docs/analysis/unified-signal-graph.md`

### Frame phases (sketch)

A deterministic frame looks like:

1. **Systems tick** (push signals)
2. **Drain signals at explicit points** (execute action/command stage, then run handlers)
3. **End-of-frame cleanup** (drain remaining signals; reset per-frame cursor)

The key is to keep ordering deterministic:

- action/command execution happens at drain time (executor stage)
- handlers observe after execution
- follow-up signals are appended and processed in order

This is why “queueing up world mutations” is fine: as long as you drain between dependent
systems in `tick()`, you get deterministic ordering and up-to-date caches where needed.

---

## Where does this live?

### Option A: `engine::rx`
Pros:
- clean separation: ECS stays “storage + systems”, RX stays “reactivity + signals”.
- makes it easy to later plug other worlds into rx (UI graph, audio graph, editor graph).
- clearer naming: `SignalWorld` doesn’t feel ECS-specific.

Cons:
- more cross-module wiring (`engine::rx` needs to know about `ecs::World`, `SignalEmitter`, `ComponentId`).
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
- systems produce intent-ish `SignalValue` variants; RX executes them in a dedicated phase

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

- `ecs::RxWorld` provides the scoped signal stream and handler dispatch.
- `SystemWorld::process_signals(...)` implements “execute (executor stage) then observe (handlers)”.
- The default executor in `SystemWorld::execute_immediate_signal(...)` applies typed mutation signals.

Goal direction:

- Rename “immediate” to “direct mode” in code/docs.
- Eventually remove per-signal direct/immediate entirely and treat execution as a drain-stage pipeline.
- Remove the `CommandQueue` facade and thread `&mut dyn SignalEmitter` (or `&mut RxWorld`) directly.
- Emitting signals should not require being inside a handler; engine code should be able to grab an emitter from context (e.g. `SystemWorld.rx`).
- `ActionSystem` installs a global handler for `SignalKind::Action` and handles higher-level intent-ish signals.

Possible next steps (optional):
1. Make a clearer split between intent vs mutation vs fact.
2. Remove per-signal direct/immediate and run execution as a drain stage.
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
