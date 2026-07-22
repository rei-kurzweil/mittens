# Secondary-Motion Signals Review

## Summary

The retained secondary-motion runtime no longer rediscovers its ECS topology every frame. That
means changes which used to become visible through polling must now arrive as lifecycle signals.

The signals have two jobs:

- keep the retained ownership indexes synchronized with the ECS graph;
- retry expensive binding only when a dependency or authored configuration actually changed.

They are internal engine lifecycle contracts, not signals that ordinary MMS scenes should emit.
The steady-state frame tick consumes the retained result and does not emit or process lifecycle
signals.

## The event-to-intent pattern

Secondary motion uses events and intents for different meanings:

- an **event** records a fact that already happened, such as `ParentChanged` or
  `GltfInitialized`;
- an **intent** asks the mutation executor to update retained secondary-motion state in response to
  that fact.

```text
canonical graph or GLTF mutation
  -> EventSignal::ParentChanged / EventSignal::GltfInitialized
  -> lightweight global RX handler
  -> targeted secondary-motion intent
  -> mutation executor
  -> SecondaryMotionSystem lifecycle entry point
  -> retained indexes and one affected binding are updated
```

The global handlers deliberately do not own or mutate `SecondaryMotionSystem`. RX handlers receive
the ECS world and a signal emitter, while the mutation executor has coordinated mutable access to
the engine systems. Emitting an intent is the bridge between those two responsibilities.

This also preserves an important semantic distinction: `ParentChanged` says that topology has
already changed; `SecondaryMotionTopologyChanged` requests the derived runtime work caused by that
change.

## Signal roles

| Signal | Normal producer | Target | Runtime effect |
| --- | --- | --- | --- |
| `RegisterSecondaryMotion` | `SecondaryMotion`, `SpringBone`, and `SpringJoint` component initialization | The initialized component | Idempotently creates retained entries, determines ownership from the already-built graph, and attempts the relevant chain binding. |
| `SecondaryMotionTopologyChanged` | Global `ParentChanged` handler | The child whose parent changed | Updates root/chain/joint ownership, transfers GLTF ownership when needed, releases stale resolved-transform claims, and rebinds only affected chains. |
| `SecondaryMotionGltfInitialized` | Global `GltfInitialized` handler | The initialized GLTF | Uses the GLTF-to-root reverse index to retry only roots waiting on that GLTF or affected by its respawn. |
| `SecondaryMotionConfigurationChanged` | Editor or engine mutation path after an in-place field edit | The changed `SpringBone` or `SpringJoint` | Uses the joint/child reverse index to rebind exactly the owning chain. |
| `UnregisterSecondaryMotion` | Component teardown compatibility path | The component being removed | Removes retained ownership and dependency records. The current subtree coordinator invokes the same cleanup entry point directly. |
| `ResetSecondaryMotion` | Explicit engine reset path | A chain, secondary-motion root, or GLTF | Rebinds the retained simulations in that target scope without enabling frame polling. |

## `RegisterSecondaryMotion` is intentionally broad

The name predates child and joint lifecycle registration, but its recipient is no longer limited to
`SecondaryMotionComponent`.

All three authored component types emit it from `Component::init`:

```text
SecondaryMotion -> register retained root and owning GLTF
SpringBone       -> register supported simulation child and root ownership
SpringJoint      -> register configuration ownership and rebind its chain
```

Registration is idempotent because initialization intents can arrive in any order. If a
`SpringJoint` intent arrives first, the system follows its already-built parent links and creates
the chain and root entries. Later chain/root intents become no-ops rather than duplicate bindings.

Registration is a lifecycle operation, so graph enumeration and selector resolution are allowed
there. The performance rule is that none of that work moves into `tick`.

## Topology changes

Canonical attach, detach, reparent, and removal paths publish `ParentChanged`. The global handler
turns it into:

```rust
IntentValue::SecondaryMotionTopologyChanged {
    component_ids: vec![changed_child],
}
```

The changed child is the most useful target because reverse indexes answer the remaining questions:

- Is it a retained root whose nearest GLTF may have changed?
- Is it a `SpringBone` moving between roots?
- Is it a `SpringJoint` being added, removed, or reordered?
- Is it an imported transform used by one or more resolved chains?

The handler does not scan for secondary-motion components. For unrelated topology changes, the
mutation executor performs constant-time retained-map misses and stops.

Code that mutates the graph with raw `World::add_child` or `detach_from_parent` does not publish an
event by itself. Runtime code should use the canonical mutation intents. Low-level tests or legacy
engine code that performs a raw mutation must publish `ParentChanged` or explicitly invoke the
secondary-motion topology contract.

## GLTF readiness and respawn

A registered chain can exist before its GLTF has spawned imported node transforms. Its state is
then `WaitingForDependencies`; the frame tick skips it and does not retry selectors.

Once GLTF spawning finishes:

```text
GltfInitialized { gltf }
  -> SecondaryMotionGltfInitialized { component_ids: [gltf] }
  -> gltf_roots[gltf]
  -> bind only those retained chains
```

The same path handles a GLTF respawn notification. Bindings release their previous imported
transform dependencies before resolving against the new spawned-node set.

## In-place configuration edits

Changing public Rust fields does not change ECS topology and therefore cannot produce
`ParentChanged`. A mutation path must explicitly announce the edit after writing it:

```rust
if let Some(joint) = world.get_component_by_id_as_mut::<SpringJointComponent>(joint_id) {
    joint.stiffness = new_stiffness;
}
emit.push_intent_now(
    joint_id,
    IntentValue::SecondaryMotionConfigurationChanged {
        component_ids: vec![joint_id],
    },
);
```

The same contract applies to changes to references, center selection, virtual endpoint length,
gravity, drag, or stiffness. `enabled` is sampled cheaply from the already-known chain during the
frame tick, but mutation paths should still use the configuration notification consistently.

Builder calls made while constructing an uninitialized MMS component tree need no notification.
The final values are present before `RegisterSecondaryMotion` binds the chain.

The target should be the component that changed. Callers do not need to discover or target its
owning root; the retained reverse index exists specifically to avoid that work.

## Removal and unregistering

The current subtree-removal coordinator visits every component before deleting its ECS record and
calls `SecondaryMotionSystem::component_removed`. This cleans all applicable indexes, including:

- root-to-child and child-to-root ownership;
- joint-configuration ownership;
- GLTF-to-root ownership;
- resolved imported-transform dependencies;
- exclusive bound-joint claims.

`UnregisterSecondaryMotion` exposes the same operation as an intent for compatibility with the
planned unified component lifecycle. It is not a second cleanup implementation.

Cleanup happens before records disappear so the system can release ownership deterministically.
The cached-ID existence checks in `tick` are only defensive protection for raw graph deletion.

## Reset is explicit invalidation

`ResetSecondaryMotion` is the escape hatch for an engine operation that knows retained simulation
state must be reconstructed even though no component field or parent changed. It accepts:

- a `SpringBone`, to rebind one chain;
- a `SecondaryMotion` root, to rebind its retained children;
- a GLTF, to rebind roots in that retained GLTF scope.

Reset is deliberately an explicit intent rather than a flag checked every frame.

## What signals do not do

These lifecycle signals do not:

- run physics or write spring rotations;
- propagate dirty transform subtrees;
- refresh skinning palettes;
- perform periodic health checks;
- broadcast configuration edits to every root;
- make arbitrary unsignaled field mutation observable.

Physics remains in the post-pose frame stage. It iterates bound retained chains and cached joints,
then returns dirty imported-transform roots for synchronous propagation before skinning.

## Usage rules

1. Let component initialization emit registration; do not manually register authored MMS trees.
2. Use canonical attach/detach/removal operations so `ParentChanged` is published.
3. After a live `SpringBone` or `SpringJoint` field edit, emit
   `SecondaryMotionConfigurationChanged` for that component.
4. Let `GLTFSystem` publish `GltfInitialized`; consumers should not guess readiness from frame
   timing.
5. Use unregister only from teardown/lifecycle infrastructure.
6. Use reset for genuine explicit invalidation, not as a retry loop.
7. Never emit registration, topology, readiness, or reset intents per frame.

## Useful code entry points

- [secondary-motion component lifecycle](../../src/engine/ecs/component/secondary_motion.rs)
- [signal definitions](../../src/engine/ecs/rx/signal.rs)
- [mutation executor dispatch](../../src/engine/ecs/rx/mutation_executor.rs)
- [retained runtime and global handlers](../../src/engine/ecs/system/secondary_motion_system.rs)
- [subtree cleanup and frame ordering](../../src/engine/ecs/system/system_world.rs)
- [secondary-motion architecture specification](../spec/secondary_motion_system.md)

## Review takeaways

- Signals replace polling as the source of runtime topology and dependency changes.
- Events describe completed external facts; targeted intents request retained-system updates.
- Reverse indexes make each signal local to the affected root, chain, joint, GLTF, or imported
  transform.
- Waiting and invalid chains stay dormant until a relevant signal arrives.
- The frame tick remains a physics loop over retained data, not a lifecycle reconciliation loop.
