# Moved

This document has been superseded by [docs/events.md](events.md).

<!--

# Topology events and derived indexes (sketch)

This document sketches an approach to make “relationship queries” fast and systematic.

In the current engine, many systems answer questions like:

- “Does this node have an ancestor of type X?”
- “Does this transform have a descendant camera?”
- “Which renderables are eligible for picking?”

Today these are often implemented as on-demand tree walks (parent chain / subtree traversal). That is simple, but it can become a hot path when performed repeatedly in per-frame ticks.

The goal is to shift cost from **query time** to **mutation time**:

- Queries become O(1) / O(log N) map lookups.
- Updates occur only when the world changes (component add/remove, parent changes, transform propagation, etc.).

## A motivating example: raycast eligibility

Raycasting needs a fast way to decide which renderables are eligible for hit testing.

Eligibility rules (current intent):

- Renderables are eligible by default.
- A `BackgroundComponent` ancestor with `ray_casting=false` is a hard opt-out.
- A `RaycastableComponent` can explicitly enable/disable eligibility.
- A `RaycastableComponent { set_default: true }` can override the global default for nodes without an explicit setting.

### Derived index: `RayCastSystem::eligible_renderables`

Instead of scanning `world.all_components()` to brute-force test ray vs AABB, `RayCastSystem` maintains:

- `eligible_renderables: HashSet<ComponentId>`

This set is updated when renderables are added/removed (and later, when policy/topology changes). The brute-force fallback then iterates `eligible_renderables` instead of the entire component store.

## What events do we need?

There are really two *classes* of events we care about:

- **Intent / gameplay events**: “something should happen” (input, triggers, scripted signals, animation dispatch).
- **Change events**: “something did happen to the component graph / component data”.

These should be related, but they should not be conflated.

The derived-index use-case in this doc is mostly about **change events**.

The engine already has a **batched command queue** which acts like an implicit event stream:

- `REGISTER_*` commands
- `UPDATE_TRANSFORM`
- `REMOVE_*` commands
- subtree removal

`CommandQueue::flush` processes these mutations and then runs post-flush sync work (e.g. BVH flush).

To generalize “derived indexes”, it helps to make these mutations explicit in a small, synchronous event stream.

### Proposed change-event types

The following event shapes are sufficient for most derived indexes:

- `ComponentAdded { node: ComponentId, kind: ComponentKind }`
- `ComponentRemoved { node: ComponentId, kind: ComponentKind }`
- `ComponentUpdated { node: ComponentId, kind: ComponentKind, flavor: UpdateFlavor }`
- `ParentChanged { node: ComponentId, old_parent: Option<ComponentId>, new_parent: Option<ComponentId> }`
- `SubtreeRemoved { root: ComponentId }`

Where:

- `ComponentKind` is a small tagged enum of relevant component categories (not dynamic type IDs).
- `UpdateFlavor` optionally adds specificity (e.g. `TransformPoseChanged`, `RaycastablePolicyChanged`).

### Why a synchronous event stream (not a second async bus)?

A synchronous stream processed during `CommandQueue::flush` has nice properties:

- Deterministic: systems see the same mutation order the world applied.
- Batched: init-time expansion won’t cause O(N) incremental churn per glyph/child.
- No lifetime issues: the events can be “by value” (ComponentId + small enums).

The default architecture could be:

1. Commands mutate the `World`.
2. Each mutation emits a small `Signal` into a `Vec<Signal>`.
3. After drain, systems consume the event list to update their derived indexes.
4. Finally, systems run their existing `flush_pending()` steps.

## Actions and events (mapping closely)

Ideally, `Action`s and events map very closely so that:

- Animations can **emit** events at keyframes.
- Systems (or scripts) can **respond** to events by producing actions.
- Actions are executed in a deterministic phase and produce the corresponding world mutations.

The key is to keep a separation between:

- “Intent happened” (events)
- “Mutation happened” (change events)

### Proposed pipeline

Think of the frame as having a small number of deterministic phases:

1. **Collect intent events** (input, timers, collision triggers, animation dispatch).
2. **Event→Action**: translate intent events into `Action`s (optional; could be multiple producers).
3. **Execute actions**: apply mutations via the existing command queue / world APIs.
4. **Emit events** while applying mutations.
5. **Update derived indexes** by consuming those events.

In this model:

- A keyframe can dispatch an **intent event** like `"AttachRaycasterToB"`.
- The ActionSystem (or an EventSystem) consumes that event and emits `Action::attach(rot_b, raycaster)`.
- The attach action mutates topology; that mutation emits `EventSignal::ParentChanged { ... }`.
- Derived indexes (BVH membership, eligible sets, “nearest background”, etc.) update from `ParentChanged`.

### Do actions become events?

There are two reasonable options:

#### Option A: Actions are a *kind of* intent event

Define a unified intent event enum that includes a variant carrying an action payload:

- `IntentEvent::Action(Action)`

Then “emitting an action” is just “emitting an intent event”.

Pros:

- One queue for all intent signals.
- Keyframes can emit actions directly without special-casing.

Cons:

- It’s tempting to skip “domain” events and emit only actions, which loses observability.

#### Option B: Keep actions and intent events separate, but adjacent

- Keyframes emit domain intent events.
- Event handlers translate intent events to actions.

Pros:

- Better introspection/debugging: you can see *why* an action happened.
- Easier to add “respond to event” behaviors without directly mutating the world.

Cons:

- Slightly more plumbing (a translator stage).

### Recommendation

Use **Option B** as the default, but allow Option A as an escape hatch.

In practice, this means you get both:

- first-class, inspectable intent events (useful for debugging and tooling)
- the ability to schedule raw actions when you want low ceremony

### Determinism and recording

If you ever want replay/recording, this layering is helpful:

- Record the intent event stream.
- Re-run the same event→action translation deterministically.
- The resulting change events should match.

## Keyframes emitting intent events

Animations already have the concept of time/beat and keyframes. Add a notion of “dispatch keyframe”:

- Keyframe has a list of dispatches (each dispatch emits an intent event).
- When the animation time crosses the keyframe time, it emits those dispatches.

This fits nicely with the “do work at mutation time” goal:

- the trigger is generated by a deterministic time crossing
- the resulting actions and topology changes flow through the same mutation/event machinery

## Listening to intent and change events with one mechanism

A key detail is that **intent events** and **change events** should be listenable through the
same subscription mechanism.

Even though they are different *kinds* of information (requests vs facts), the ergonomics should
be identical:

- register a handler
- get called when a matching event occurs
- optionally emit more intent events (but not mutate the world directly)

### Unified event envelope (current implementation)

The engine uses a single signal type carrying a scope and a value enum:

- `ecs::Signal { scope: ComponentId, value: ecs::SignalValue }`

This is a unified stream of **facts** (e.g. topology changes and interaction events). Intent
is represented separately as `Action` (executed by `ActionSystem`).

### Universe-level handler API (sketch)

The public API can look like:

- `universe.add_signal_handler(filter, handler)`

If you want an even simpler API with **no predicate**, you can expose a two-dimensional listener key:

- event type (enum discriminant / variant)
- scope root (ComponentId)

For example:

- `universe.add_signal_handler(signal_type, scope_root, handler)`

Where `filter` can be:

- a coarse filter (intent vs change)
- plus an optional match on a specific variant/kind

For example (pseudo-Rust):

```rust
universe.add_signal_handler(ecs::SignalKind::ParentChanged, scope_root, |world, queue, env| {
  if let ecs::SignalValue::Event(ecs::EventSignal::ParentChanged { child, old_parent, new_parent }) = &env.value {
    println!("child={child:?} old={old_parent:?} new={new_parent:?}");
    let _ = (world, queue);
  }
});
```

Notes:

- Systems can update their derived caches from `Signal` without requiring any handler registration.
  (Handlers are for “gameplay logic / scripts / debug tools”.)

### Dispatch order / phases

To keep things deterministic and to avoid re-entrancy bugs, dispatch in phases:

1. Collect intent events.
2. Dispatch intent handlers (handlers may enqueue more intent events).
3. Translate intent→actions (or have handlers emit `IntentEvent::Action`).
4. Execute actions via command queue (mutations).
5. Collect `Event`s while mutating.
6. Dispatch event handlers.
7. Update derived indexes from events (or do this before step 6 if you want handlers to see
   “already-updated caches”).

This answers “intent events happen before” in a precise way: yes, they dispatch in an earlier
phase, but the crucial property is *what they are allowed to do*.

### Implementing `add_signal_handler(signal_type, scope_root, handler)`

This API has two filtering dimensions:

1. **Signal type**: only deliver signals of the requested variant/kind.
2. **Scope root**: only deliver signals whose `env.scope` lies within the subtree rooted at `scope_root`.

You can implement this without general predicates by using a nested map keyed by both dimensions.

#### Listener storage

Use a structure like:

- `listeners_by_type: HashMap<SignalKind, HashMap<ComponentId, Vec<Handler>>>`

Where:

- `EventType` is a small enum describing the variants you can subscribe to (for intent and change).
  - You can also reserve an `EventType::AnyIntent` / `AnyChange` / `Any` if you want wildcard subscriptions.
- The inner map key is the **scope root**.

#### Dispatch algorithm (ancestor walk)

Given an emitted signal with `signal_type` and `env.scope = scope_node`:

1. Deliver to listeners registered under the exact scope node.
2. Then walk up the parent chain:

- `cur = scope_node`
- while `cur` exists:
  - deliver listeners for `(event_type, cur)`
  - `cur = world.parent_of(cur)`

This works because “listener scope roots” are ancestors of any node in their subtree.

Complexity:

- O(depth(scope_node) + total_listeners_matched)

This is typically fast because depth is small and number of listeners is small.

#### Notes / extensions

- If an event affects multiple nodes (e.g. attach touches parent and child), you can emit the same
  payload with multiple scopes, or define a canonical scope (usually the mutated node) and rely on
  payload fields for secondary ids.
- If you need faster-than-ancestor-walk (very deep trees, many listeners), you can later introduce
  subtree interval indexing (Euler tour / in-out times) and store listeners by interval, but that
  adds complexity and usually isn’t needed early.

### ECS integration: EventHandlerComponent / ScriptComponent

Long-term, the same handler mechanism can be exposed through components:

- `EventHandlerComponent` could register a handler when initialized and unregister on cleanup.
- `ScriptComponent` could be “an EventHandlerComponent with a script runtime”.

The important bit is that both are just *front-ends* for the same underlying event subscription
and dispatch machinery.

### Minimal viable implementation

You don't need reactive streams to get most of the benefit. A minimal starting point is:

- An `EventBus` that owns:
  - `Vec<IntentEvent>` (per-frame intent queue)
  - `Vec<Signal>` (per-dispatch signal queue)
  - `Vec<Handler>` (handlers with filters)
- A `dispatch_*()` function that iterates handlers and calls those whose filter matches.

As the engine grows, you can add:

- handler priorities (ordering)
- “once” handlers
- scoped handlers (lifetime tied to a component)
- event recording/replay

## System-local derived indexes

Each system maintains the minimum data it needs to answer queries quickly.

Examples:

- **RayCastSystem**
  - `eligible_renderables: HashSet<ComponentId>`
  - `ray_visual_by_raycast: HashMap<RayCasterId, TransformId>`

- **BVHSystem**
  - `index_by_component: HashMap<RenderableId, ShapeIndex>`
  - (optionally) `eligible_renderables: HashSet<RenderableId>` to avoid re-checking policy

- **CameraSystem**
  - `active_camera: ComponentId`
  - `transform_has_camera_child: HashMap<TransformId, bool>`

- **RenderableSystem**
  - `nearest_background: HashMap<RenderableId, Option<BackgroundId>>`

### Update strategy

Derived indexes typically update on:

- Renderable added/removed: add/remove from sets and maps.
- Parent changed: recompute derived properties for `node` and (often) its subtree.
- Policy component added/removed/updated (e.g. Background/Raycastable): recompute derived properties for affected subtree.

The key idea: **subtree invalidation is acceptable** because topology changes are rare compared to per-frame ticks.

## Policy evaluation and caching

Any rule that depends on “nearest ancestor with component X” can be implemented with:

- a cached “nearest X” pointer per node, updated on parent changes, OR
- an eligibility boolean per renderable, updated on relevant events.

The second approach is simpler and faster for raycasting:

- Cache `eligible: bool` per renderable.
- On `ParentChanged` or relevant policy updates, recompute eligibility for the affected subtree.

## Debugging / visualization

Once systems maintain domain-specific graphs, we can export them:

- `eligible_renderables` as a list
- parent-child edges filtered to relevant kinds
- DOT output for rendering with Graphviz

This becomes a cheap, high-signal view of “what the system thinks the world is”, which helps when debugging topology-driven behaviors.

## Current limitations

- The engine does not yet emit explicit events for component updates like `RaycastableComponent` changes.
- Some systems already compute “relationship queries” on demand (tree walks), which can become hot.

## Next steps (implementation outline)

1. Extend `ecs::Signal` + `ecs::SignalKind` for additional mutation/update signals.
2. Emit `ParentChanged`, `ComponentAdded/Removed/Updated`, `SubtreeRemoved` at mutation points.
3. Give systems a `consume_signals(&[Signal])` hook (or `flush_signals` stage).
4. Implement the first full derived index using events:
   - Maintain `RayCastSystem::eligible_renderables` accurately across:
     - renderable add/remove
     - background/raycastable policy changes
     - parent changes
     - subtree removal


-->

