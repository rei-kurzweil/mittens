# Intent migration audit (actions → intents + handlers)

This doc audits the remaining work to make **all world/system mutations** flow through the signal pipeline:

- **events** → dispatched to **handlers** (observers)
- handlers emit **intents**
- intents are executed by an **intent executor** at explicit drain points

The goal is to remove remaining “action/command” style mutation entrypoints and ensure we don’t have side effects happening in arbitrary places.

## Current architecture (as of 2026-03-07)

### Signal plumbing

- `CommandQueue` is now a per-frame staging buffer that implements `SignalEmitter` and drains into `RxWorld` at explicit drain points.
  - Source: `src/engine/ecs/command_queue.rs`
- `RxWorld` stores ready/deferred events and ready/timed intents, plus scoped/global handler tables.
  - Source: `src/engine/ecs/rx/rx_world.rs`
- `SystemWorld::process_signals` is the drain loop:
  1. drain + dispatch all **ready events** (handlers)
  2. drain + execute all **ready intents**
  3. loop until queues are empty or max count reached
  - Source: `src/engine/ecs/system/system_world.rs`

### Intent execution is split in two

There are currently two intent executors:

1) **RxIntentExecutor** (high-level intent interpreter)
- Chooses a conservative set of “user intent” values (`IntentValue`) to interpret.
- Delegates to `action_system::handle_intent_signal`.
- Source: `src/engine/ecs/rx/intent_executor.rs` and `src/engine/ecs/system/action_system.rs`

2) **Default executor** (low-level mutation executor)
- Everything not handled by `RxIntentExecutor` is executed by `SystemWorld::execute_intent_signal`.
- This covers the internal “register/remove/update” style intents and some user intents (notably `SetText`).
- Source: `src/engine/ecs/system/system_world.rs`.

### Important current type-level reality

- `IntentValue` still contains both:
  - “user intent” (SetColor/Attach/RemoveSubtree/etc)
  - “internal mutation ops” (RegisterRenderable/UpdateTransform/RegisterTexture/…)
  - Source: `src/engine/ecs/rx/signal.rs`

This mixing is the root reason we still have migration debt: it’s unclear which values are intended to be public API vs internal engine plumbing.

## What’s already done

- No legacy `SignalKind::Action` exists (explicitly documented in `SignalKind`).
- Most user-facing mutations already exist as intents (`Attach`, `RemoveSubtree`, `SetTransform`, etc).
- Scoped handler lifecycle cleanup exists when deleting a subtree.

## Inventory: remaining “action-like” mutation pathways

### A) `action_system` still performs direct mutations

`action_system::handle_intent_signal` currently does **real mutations** in addition to emitting follow-up intents/events.

Examples:

- `IntentValue::SetColor` mutates `ColorComponent.rgba` directly, then emits `IntentValue::RegisterColor`.
- `IntentValue::SetTransform` mutates `TransformComponent` directly, then emits `IntentValue::UpdateTransform`.
- `IntentValue::Attach` / `Detach` mutate topology via `World::add_child` / `World::detach_from_parent`, then emit `EventSignal::ParentChanged` and other follow-ups.

This is “action-system style” behavior: interpretation + mutation are mixed.

### B) Public API (`Universe`) still bypasses intent execution

`Universe` provides convenience APIs like `attach`, `remove_child`, `remove_children`, `attach_clone`.

At the moment, some of these APIs **directly mutate topology** (e.g. `world.add_child`) and then push events/intents manually.

This bypasses the goal of a single mutation path (intents executed only at drain points).

Source: `src/engine/universe.rs`.

### C) Some follow-up work is performed “inline” instead of via handlers

Example: after `Attach`, `action_system` performs/queues topology refresh and audio dirtiness directly.

A cleaner end-state is:

- `Attach` causes a `ParentChanged` **event**
- a `ParentChanged` **handler** emits any required follow-up intents:
  - transform propagation refresh
  - audio graph dirty
  - gizmo target retargeting (already handler-driven at the gizmo scope)

That makes “what reacts to topology changes” centralized and composable.

## Proposed target architecture

### Split “intent interpretation” from “mutation execution”

- **Intent interpreter**: expands high-level user intent into low-level mutation intents.
  - Should be *read-heavy*, and ideally avoid mutating components directly.
- **Mutation executor**: performs all actual world/component/system state changes.

This implies introducing a clearer boundary than we currently have.

### Move reactive follow-ups into handlers

- For stable events that represent facts (`ParentChanged`, collisions, drag gestures), move cross-cutting side effects into scoped/global handlers.
- Keep intent executor focused on “do the mutation requested”.

## Concrete audit checklist (what to change)

### 1) Classify intent values (public vs internal)

**Task:** Decide and document which `IntentValue` variants are:

- **User intent (API)**: requested by gameplay or tools, should be stable.
- **Engine mutation (internal)**: purely plumbing used by systems/components.

**Suggested direction:** split the enum.

- `UserIntent` (public-ish)
- `EcsMutation` (internal, exhaustive, allowed to be noisy)

This removes ambiguity and makes it obvious what belongs in which executor.

### 2) Make `action_system` either pure interpreter *or* delete it

Right now `action_system` mutates world/components.

Two possible end states:

A) Keep it as **IntentInterpreter**
- Rename module to avoid the old “Action” vocabulary.
- Stop directly mutating components where possible; emit low-level mutations instead.

B) Remove it
- Move logic into a dedicated `RxIntentExecutor` implementation that lives near `rx/`.

**Note:** repo policy says don’t keep old names “for compatibility” after renames. If we rename `action_system`, we should fully migrate call sites.

### 3) Reduce duplicated mutations (important correctness + perf)

Example hotspot:

- `SetTransform` currently mutates `TransformComponent`, then emits `UpdateTransform`, whose executor also mutates the component and triggers transform propagation.

**Task:** For each high-level intent, ensure we don’t mutate the same state twice.

### 4) Migrate `Universe` convenience APIs to emit intents

Goal: public helpers should just **emit** intents and let drain points execute them.

- `Universe::attach` should emit `IntentValue::Attach { parents: vec![parent], child }`.
- `Universe::remove_child` should emit `IntentValue::RemoveChild { parents: vec![parent], index }`.
- `Universe::remove_children` should emit `IntentValue::RemoveChildren { parents: vec![parent] }`.
- `Universe::attach_clone` should emit `IntentValue::AttachClone { ... }`.

This makes Universe behavior consistent with the new model and makes ordering semantics explicit (drain-point driven).

### 5) Add core event handlers for topology changes

Add a small set of global/scoped handlers that react to `ParentChanged`:

- Transform propagation invalidation/refresh
- Audio graph dirtying
- (Any other topology-derived indexes)

Then remove those concerns from the intent interpreter.

### 6) Decide what *must* remain “direct executor” work

Some operations don’t need a handler because they are direct mutations:

- Register/unregister component types with systems
- Update system caches (BVH, renderable registries)

These should remain in the low-level executor as `EcsMutation`-style intents.

## Suggested implementation order

1. Document classification (public vs internal) and add a new enum if we go that route.
2. Migrate `Universe` APIs to emit intents only.
3. Add `ParentChanged` handlers for cross-cutting follow-ups.
4. Simplify `action_system` into a pure interpreter (or rename/remove).
5. Remove duplicated mutation paths and tighten executor responsibilities.

## Notes / questions to resolve

- Should `SetText` be treated as a high-level user intent (handled by interpreter) or a low-level mutation (handled by default executor)? Today it’s the latter.
- Do we want a first-class *event* for “transform changed” (as a fact) separate from the “update transform” intent? That can make reactive systems handler-driven without inventing intent variants.
- Which parts of topology refresh are required immediately in the same tick vs can be deferred?
