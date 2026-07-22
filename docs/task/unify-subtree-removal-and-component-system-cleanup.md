# Unify subtree removal and component system-state cleanup

Date: 2026-07-21

Status: planned.

## Problem

Subtree removal currently has two materially different paths:

- `World::remove_component_subtree` removes graph records directly. It does not call
  `Component::cleanup` and cannot clean state owned by engine systems or `VisualWorld`.
- `SystemWorld::remove_subtree_immediate` walks the subtree and maintains a hand-written list of
  known component types whose system state must be removed before calling the raw `World` method.

Several systems call `World::remove_component_subtree` directly. Those calls bypass render,
collision, input, IK, secondary-motion, signal-routing, and other runtime cleanup when the removed
subtree happens to contain such components. Systems then need defensive stale-ID pruning, and every
new registered component type must remember to update the centralized best-effort type list.

`Component::cleanup` already exists as a lifecycle hook, and several components implement it, but
the raw removal methods never invoke it. The current API therefore makes the unsafe path easier to
call than the lifecycle-complete path.

## Desired outcome

- One authoritative runtime subtree-removal operation owns component cleanup, system
  unregistration, visual/resource cleanup, topology mutation, and scoped signal-handler cleanup.
- Cleanup runs exactly once for every component in the subtree while its component record and
  original parent/child topology are still available.
- Engine systems request removal through the authoritative operation instead of mutating the world
  graph directly.
- The low-level graph deletion primitive is inaccessible to ordinary runtime systems.
- Adding a component with registered system state requires colocated lifecycle cleanup, not another
  `SystemWorld` type check.
- Systems do not need full-world scans or permanent stale-ID recovery to compensate for bypassed
  removal lifecycle.

## Proposed direction

### Two-phase removal coordinator

Move runtime removal behind a coordinator owned by `SystemWorld`/the mutation executor:

1. Validate and snapshot the subtree in deterministic child-before-parent cleanup order.
2. Mark the subtree as removal-in-progress to prevent duplicate or recursive removal.
3. Run each component's cleanup while all records and topology are still readable.
4. Drain or synchronously apply cleanup operations that require live component data, including
   visual handles and system registrations.
5. Delete the component records through a low-level `World` graph primitive.
6. Remove scoped handlers and clear cross-system references to deleted IDs.

The implementation must define whether cleanup is expressed entirely as intents or through a
typed cleanup context. If intents remain the mechanism, cleanup intents that require component
data must be executed before structural deletion rather than left in the normal later queue.

### API boundary

- Rename/restrict the current structural primitive so it is clearly unsafe for runtime use, for
  example `World::remove_component_subtree_records` with narrow crate visibility.
- Keep `IntentValue::RemoveSubtree` as the normal deferred request from systems that only own a
  `SignalEmitter`/command queue.
- Provide one synchronous coordinator entry point only for code already operating at a safe
  mutation drain point.
- Do not let individual systems reproduce partial subtree traversal or cleanup ordering.

### Lifecycle ownership

- Audit every system registration and pair it with an unregister operation reachable from component
  cleanup: renderables, transforms/transitions, collisions and responses, avatar/IK/secondary
  motion, pointers/raycasting, XR/input, HTTP, stencil/signal routes, text focus, and future systems.
- Replace the hand-maintained downcast list in `SystemWorld::remove_subtree_immediate` with the
  unified lifecycle mechanism.
- Preserve special cleanup that is genuinely subtree-wide, but invoke it from the coordinator
  rather than from arbitrary callers.
- After all raw call sites are migrated, turn defensive stale-registration pruning into debug
  assertions or remove it where system ownership guarantees make it redundant.

## Call-site migration

Audit and migrate every direct production call to `World::remove_component_subtree`, currently
including panel, grid, object-placement preview, pose capture, editor, asset, world-panel, and
editor-paint paths. Tests that specifically exercise raw graph behavior may continue using the
low-level primitive from an appropriately restricted test API.

Each runtime caller should either:

- emit `RemoveSubtree` and let the next mutation drain perform removal, or
- call the synchronous coordinator only when same-operation removal is required and the caller is
  already inside the mutation boundary.

## Edge cases to specify

- A root requested twice, overlapping ancestor/descendant requests, and a missing root.
- Cleanup that requests removal of another subtree or of an ancestor already being removed.
- Cleanup intents that enqueue additional mutations or attach/detach operations.
- Runtime-only descendants created by systems and components referenced from outside the subtree.
- Removal during GLTF respawn/reload and removal of partially initialized component trees.
- Deterministic cleanup order and whether parent lookup exposes the original or detached topology.
- Failure handling: cleanup should be best-effort without leaving half-deleted graph records or
  persistent registrations.

## Required regression coverage

- A synthetic registered component receives init and cleanup exactly once through the unified path.
- Nested components clean up child-before-parent while their original topology remains queryable.
- Removing a subtree unregisters renderable, collision, IK, secondary-motion, XR/input, and signal
  state without waiting for a later frame.
- Direct runtime callers cannot access the raw graph-deletion API after migration.
- Duplicate and overlapping removal requests do not double-clean or panic.
- Cleanup-triggered mutations are drained in the documented order.
- GLTF respawn and editor deletion leave no stale component IDs in system-owned collections.
- Existing world graph leaf/subtree structural tests continue to cover the low-level deletion
  primitive independently of runtime lifecycle behavior.

## Completion criteria

- All production subtree deletion flows through the unified coordinator.
- `Component::cleanup` is part of the real removal lifecycle or is replaced by one documented
  equivalent mechanism.
- The best-effort component-type cleanup list is removed.
- System-owned state is empty immediately after subtree removal in focused tests.
- No subsystem relies on scanning the world to discover deleted registrations.
