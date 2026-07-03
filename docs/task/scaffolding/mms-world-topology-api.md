# World Topology API — Intents, Queries, and the MMS Runtime

This document defines the principle: **all topology mutations go through
intents**. It inventories the current state, identifies what's missing, and
distinguishes mutation intents from query access — which are different problems
solved differently.

---

## Principle: all topology mutations are intents

A topology mutation is any operation that changes the structure of the
component tree or brings a new component into existence. These should all be
expressed as `IntentValue` variants so there is one pipeline for all world
mutations. This makes the world's mutation surface auditable, scriptable,
schedulable (`AtBeat`), and routable.

---

## Current mutation inventory

### Already intents ✓

| Operation | IntentValue variant |
|---|---|
| Attach child to parent | `Attach { parents, child }` |
| Detach from parent | `Detach { component_ids }` |
| Remove child at index | `RemoveChild { parents, index }` |
| Remove all children | `RemoveChildren { parents }` |
| Remove subtree | `RemoveSubtree { component_ids }` |
| Clone prefab and attach | `AttachClone { parents, prefab_root }` |

### Missing — not yet an intent

| Operation | Current mechanism | What to add |
|---|---|---|
| Spawn a new component tree | Direct world mutation in `ComponentCodec::decode_subtree`, `Universe::add()`, Rust example code | `IntentValue::SpawnComponentTree` (see `mms-runtime-and-intents.md`) |

That's it. Component creation is the only topology mutation not yet going
through the intent pipeline.

---

## The AttachClone problem

`IntentValue::AttachClone` is already an intent, but its implementation in
`RxIntentExecutor` calls `ComponentCodec::encode_subtree_node` and
`ComponentCodec::decode_subtree_node_with_new_guids` directly inside the
executor. This is the one place where ComponentCodec is entangled inside the
intent pipeline.

When ComponentCodec is removed (as part of the MMS migration), `AttachClone`'s
executor branch needs to be rewritten. The replacement is:

1. Walk the prefab subtree and call `encode_mms()` on each node → build a
   `ComponentExpression` tree.
2. Execute that tree as if it were a `SpawnComponentTree` intent (same logic), with
   fresh GUIDs.

This can be modeled as: `AttachClone` emits a `SpawnComponentTree` intent with the
encoded prefab expression as its payload, and the `SpawnComponentTree` executor
handles the rest. `AttachClone` then becomes a one-liner in the intent executor.

---

## Queries are not mutations — different solution

A topology *query* is a read: "what is the parent of X?" or "what are the
children of Y?". Queries don't fit the mutation-intent model because intents
are fire-and-forget side effects. There is no return value pathway in
`IntentValue` today, and adding one would require a fundamentally different
mechanism.

The right answer depends on *who is asking* and *from where*.

### Case 1: Main-thread code (handlers, executors, Universe helpers)

Code running on the main thread has direct access to `&World` (or
`&mut World`). Queries are synchronous:

```rust
world.parent_of(id)          // → Option<ComponentId>
world.children_of(id)        // → &[ComponentId]
world.get_component_node(id) // → Option<&ComponentNode>
```

`Universe` already exposes these as helpers:

```rust
universe.parent_of(id)
universe.children_of(id)
universe.get_component_by_id_as::<T>(id)
```

No intent needed. Direct synchronous access is correct here and should stay
that way. Making these into intents on the main thread would add indirection
with no benefit.

### Case 2: Off-thread scripting (future — MMS worker thread)

When MMS scripts eventually run on a worker thread (compiler + VM path, phase
2+), they won't have direct access to `&World`. Queries need to cross the
thread boundary.

The right mechanism is **query intents with a reply channel** — the same
executor pipeline as mutation intents, just with a response attached:

```rust
// New intent variants (phase 2+, not phase 1):
IntentValue::QueryParent {
    component_id: ComponentId,
    reply: oneshot::Sender<Option<ComponentId>>,
}
IntentValue::QueryChildren {
    component_id: ComponentId,
    reply: oneshot::Sender<Vec<ComponentId>>,
}
```

These are not slow. Like all intents they go directly to the executor, not
through event broadcasting. The executor reads from `&World` and sends the
answer back through the `reply` channel immediately when it processes the
intent. There's no broadcast, no handler routing, no subtree traversal.

The one cost is timing: the response arrives at the next drain point, not
instantaneously. This is inherent to cross-thread communication — the worker
thread must wait one round-trip (one drain point's worth of latency) for the
answer. For a VM that can yield and resume, this is fine. For a blocking
synchronous call, it means a brief stall of at most one tick.

This is not needed for phase 1 because `SpawnComponentTree` executes on the
main thread and has direct world access. It is worth designing for from the
start so that the future VM path has a clear query story.

---

## Summary: topology API contract

| Category | Operation | Mechanism |
|---|---|---|
| Mutation | Attach / Detach / Remove | `IntentValue` (already) |
| Mutation | Spawn component tree | `IntentValue::SpawnComponentTree` (to add in phase 1) |
| Mutation | Clone prefab | `IntentValue::AttachClone` → rewrite to use `SpawnComponentTree` when ComponentCodec is removed |
| Query (main thread) | parent / children / component data | Direct `&World` / `Universe` helpers (synchronous, no intent) |
| Query (off-thread) | parent / children | `IntentValue::QueryParent` / `QueryChildren` with oneshot reply channel — intent goes to executor (fast), response arrives at next drain point (phase 2+) |

---

## Phase 1 implications

Phase 1 only needs to add `SpawnComponentTree`. Everything else in this document is
either already in place or deferred to phase 2+.

The `AttachClone` refactor (replacing its internal `ComponentCodec` calls)
should happen as part of the ComponentCodec removal in the MMS migration, not
necessarily in phase 1 — but it should be tracked so it isn't forgotten. Add
it to the phase 1 checklist as a note under step 6.
