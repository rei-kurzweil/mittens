# Actions + Signals in Cat Engine (current model)

## Practical examples (start here)

### Example 1: listen for topology changes in a subtree

One built-in structural signal is `ParentChanged`.

Register a listener on some subtree root (e.g. a character rig root), and you’ll be notified when
any component in that subtree is reparented (attach/detach).

```rust
use cat_engine::engine::ecs;

fn on_topology_change(
    _world: &mut ecs::World,
    _queue: &mut ecs::CommandQueue,
    signal: &ecs::Signal,
) {
    match &signal.value {
        ecs::SignalValue::Event(ecs::EventSignal::ParentChanged {
            child,
            old_parent,
            new_parent,
        }) => {
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

### Example 2: actions are *intent*, signals are *facts*

When you run an action (from animation keyframes, input, tools, etc), the action executes
immediately.

If the action performs a meaningful state transition (like a topology move), it emits a
**signal** (a fact).

That means you can:

- use actions for “do something now”
    - use signals for “something happened; react/derive/cache/log”

---

## What is an Action?

An **Action** is a reusable “verb + targets + params” payload interpreted by `ActionSystem`.

- Actions are *not* events.
- Actions are *intent*: a request to call engine APIs.

In practice today:

- `AnimationSystem` executes actions at keyframes.
- `InputSystem` and tools/REPL can also execute actions.
- `ActionSystem` may queue deferred work through `CommandQueue`.

### When do actions emit signals?

Actions emit events only when it’s useful to observe or maintain derived state.

Currently emitted:

- `EventSignal::ParentChanged`

Emitted from:

- `ActionSystem` when an action attaches/detaches/removes children
- `Universe` helper methods (`attach`, `remove_child`, `remove_children`, `attach_clone`)

(So you get consistent topology notifications whether the move was caused by actions or direct
Universe-level helpers.)

---

## What is a Signal?

An **Signal** in Cat Engine means a fact about the world.

Signals:

- are scoped to a component subtree (`scope: ComponentId`)
- are dispatched after command flushing for the frame

---

## How `Universe::add_signal_handler` works

`Universe::add_signal_handler` is a thin API over `RxWorld::add_handler`.

### Registration structure

Internally, listeners are organized as:

- `(SignalKind, scope_root) -> Vec<SignalHandler>`

Conceptually:

```rust
HashMap<SignalKind, HashMap<ComponentId, Vec<fn(&mut World, &mut CommandQueue, &Signal)>>> 
```

(In code today this is `HashMap<SignalKind, HashMap<ComponentId, Vec<SignalHandler>>>`.)

This avoids global scanning: there’s no “iterate all listeners and test predicates” step.

### Scope dispatch semantics

Each signal has a `scope` (the component where the signal is most relevant).

To dispatch, the engine walks the scope’s ancestor chain:

- `scope, parent(scope), parent(parent(scope)), ...`

Any listener registered on any of those nodes will fire.

So:

- a listener on the root sees everything beneath
- a listener on a nested node sees only changes in that nested subtree

### When listeners run (important)

Events are dispatched in `SystemWorld::process_commands`, immediately after the
`CommandQueue` flush.

Implications:

- listeners observe a stable “post-mutation result"
- listeners can queue new commands; the engine flushes again after dispatch so effects can be
    visible in the same frame

---

## Why this model scales (UI / game / simulation)

With these primitives:

- **Components**: a graph of typed nodes (no entity layer required)
- **Systems**: maintain derived state (VisualWorld, BVH, audio graph, etc)
- **Actions**: portable intent payloads
- **Change events**: scoped facts for reaction + derived-index maintenance

…you can build:

- UI: scope listeners to widget subtrees; actions drive immediate interactions; changes drive caches
- Game logic: keyframes execute actions; change events trigger follow-up (effects, recompute, logging)
- Simulation: systems do heavy lifting; change events keep derived indexes in sync without O(N) scans
