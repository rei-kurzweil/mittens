# RxWorld component lifecycle (automatic handler cleanup)

## Goal

When a component id used as a *handler scope root* is removed from the `World`, `RxWorld` should automatically remove any handlers rooted at that scope.

Why:
- Prevent unbounded growth of `RxWorld.scoped_handlers` in long-running sessions.
- Avoid confusing behavior where handlers remain registered for ids that can never be dispatched meaningfully again.

Non-goals:
- Automatically removing **global** handlers. Global handlers have no scope root, and may intentionally observe many unrelated entities.
- Preserving/rehydrating handlers across save/load. (This is separate from the JSON schema policy.)

## Current handler storage

`RxWorld` stores handlers as:

- `scoped_handlers: HashMap<SignalKind, HashMap<ComponentId, Vec<Handler>>>`
- `global_handlers: HashMap<SignalKind, Vec<Handler>>`

Dispatch walks the scope chain (scope and its ancestors) and runs handlers rooted at any element in that chain.

## The lifecycle problem

A scoped handler is keyed by `ComponentId`. If that component is deleted:

- The handler will never be invoked via scope chain traversal (because the node isn’t reachable).
- The entry stays resident in the map forever unless explicitly removed.

Slotmap generations make this *safe* (deleted ids won’t alias new ids), but it’s still a memory/perf leak.

## Where removal should happen

The important observation is:

- `RxWorld` does not own the component graph.
- The only authoritative place that knows which ids are being removed is the `World` removal path.

Therefore, the simplest and most reliable integration point is **where we already delete subtrees**.

### Implemented approach (today)

When `SystemWorld` deletes a subtree (in `remove_subtree_immediate`), it already enumerates the subtree nodes to remove system-side state (renderables, collisions, transforms, etc).

We can reuse that same list to prune `RxWorld` scoped handlers:

- For each deleted id `n`, call `rx.remove_all_scoped_handlers_for_scope(n)`.

This is implemented via:
- `RxWorld::remove_all_scoped_handlers_for_scope`
- `RxWorld::remove_all_scoped_handlers_for_scopes`

and invoked from:
- `SystemWorld::remove_subtree_immediate`

This yields:
- No changes to `World::remove_component_subtree` signature.
- No need for a separate “component removed” event type.

### Why “known ahead of time” matters

To delete handlers efficiently, we need to know which scopes are going away.

Subtree removal naturally provides this, because we can walk the subtree before deletion (while parent/children links still exist) and produce the exact set of ids that will be invalid after deletion.

## Alternative designs (future)

### 1) World emits lifecycle events

`World` could emit something like `EventSignal::ComponentRemoved { id }`.

Downsides:
- Those are still events that need dispatch; order/deferral rules complicate “cleanup now”.
- Cleanup should not itself depend on handlers (bootstrap problem).

### 2) RxWorld lazily prunes during dispatch

During dispatch, `RxWorld` could periodically prune scoped handler entries that refer to ids not present in `World`.

Downsides:
- Requires `RxWorld` to query `World` membership frequently.
- Doesn’t clean up handlers for removed ids that are never part of any dispatched scope chain.

### 3) Handler IDs + reverse index

Assign a stable `HandlerId` on registration and keep a reverse index:

- `by_scope: HashMap<ComponentId, Vec<(SignalKind, HandlerId)>>`

Then subtree deletion can delete entries in (amortized) O(number of handlers) rather than O(number of scopes × kinds).

This is mostly worthwhile if we start registering many per-entity handlers.

## Notes

- This cleanup only affects **scoped** handlers (registered with `add_handler*`).
- If we later move systems like GizmoSystem to truly per-gizmo *scoped* handlers, this cleanup becomes essential.
