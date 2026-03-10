# RefreshTransform: topology refresh without value mutation

## Summary
`RefreshTransform` is an internal intent used to recompute *transform-derived caches* (world matrices, renderable instance models, skinning dirtiness, BVH refits, etc.) **without changing any `TransformComponent` values**.

This exists because the previous implementation used `UpdateTransform` as a “refresh” mechanism after topology changes (e.g. `Attach`). Once intent routing was introduced for `update_transform`, that refresh path could be routed onto *different components*, accidentally overwriting joint transforms just from clicking a viz proxy.

## The bug this fixes (old behavior)
### Scenario
- GLTF transform-visualization mode spawns proxy transforms `viz:*` under `viz_overlay:*`.
- Each `viz:*` transform has a child `SignalRouteUpwardComponent` operator configured for `intent_kind = "update_transform"` and `parent_type = "transform"`.
- The editor selects by clicking the renderable `viz_box:*` (the proxy mesh), then reparents the gizmo under the clicked transform.

### What happened on click
1. Editor selection emits an `Attach { parents: [viz_transform], child: gizmo }` intent.
2. `Attach` execution performs `world.add_child(parent, child)` and then calls `emit_topology_transform_refresh()` for both:
   - the moved `child` (gizmo subtree)
   - the `parent` (the clicked `viz:*` transform)
3. **Old implementation of `emit_topology_transform_refresh()` emitted an `UpdateTransform` intent**, using the nearest `TransformComponent`’s current `translation/rotation/scale` as payload.
4. Because `UpdateTransform` is a routable intent (its `component_ids` are rewritten by `SignalPipelineProcessor`), the refresh for `viz:*` was rewritten by the route-up pipeline to target the ancestor “real joint” transform.
5. Result: the engine applied `systems.update_transform(real_joint, viz_transform_values)`.

### Symptom
A plain click on `viz_box:J_Bip_L_LowerArm` could instantly “nuke” the lower-arm chain (skinned mesh collapses / limb disappears) without any drag movement.

### What was expected to happen before routing existed
Before routing existed, emitting `UpdateTransform` with the same values for the same component was used as a cheap “poke” to:
- recompute `matrix_world` for the transform subtree
- push `VisualWorld` model updates for renderable descendants
- mark skinning rigs dirty and queue BVH refits

That expectation breaks once recipient routing can change *which* component receives the `UpdateTransform`.

## New behavior (fixed)
### `RefreshTransform` intent semantics
- Recompute all transform-derived caches for the specified `component_ids`.
- **Do not modify** `TransformComponent.transform.translation/rotation/scale/model`.
- **Must not be routed** by the pipeline processor.

### Implementation notes
- `emit_topology_transform_refresh()` now emits `RefreshTransform` instead of `UpdateTransform`.
- The mutation executor handles `RefreshTransform` by calling `systems.transform_changed(world, visuals, component)`.
- The pipeline processor treats `RefreshTransform` as non-routable (it does not expose `component_ids` for rewriting).

## Where `RefreshTransform` is used / emitted
### Emitted
- [src/engine/ecs/rx/intent_executor.rs](../../src/engine/ecs/rx/intent_executor.rs)
  - `emit_topology_transform_refresh(...)` emits `IntentValue::RefreshTransform { component_ids: vec![...] }`
  - Called after topology-changing operations:
    - `Attach`
    - `AttachClone`
    - `Detach`
    - child removals (`RemoveChild`, `RemoveChildren`) where refresh is triggered (refreshes the parent)

### Executed
- [src/engine/ecs/rx/mutation_executor.rs](../../src/engine/ecs/rx/mutation_executor.rs)
  - `IntentValue::RefreshTransform { component_ids }` => `systems.transform_changed(...)`

### Defined / named
- [src/engine/ecs/rx/signal.rs](../../src/engine/ecs/rx/signal.rs)
  - `IntentValue::RefreshTransform { component_ids }`
  - `kind_name()` maps it to `"refresh_transform"`

### Explicitly non-routable
- [src/engine/ecs/rx/signal_pipeline_processor.rs](../../src/engine/ecs/rx/signal_pipeline_processor.rs)
  - `recipient_component_ids(_mut)` returns `None` for `RefreshTransform` so routing never rewrites it.

## What counts as “transform-derived caches”
In this engine, calling `SystemWorld::transform_changed(...)` triggers:
- `TransformSystem::transform_changed(...)`
  - recomputes cached `matrix_world` down the transform subtree
  - updates `VisualWorld` instance model matrices for descendant renderables
  - updates cameras/lights/collision data that depend on transforms
- `SkinnedMeshSystem::transform_subtree_changed(...)`
  - marks affected skin bindings dirty so joint palettes are recomputed lazily
- `BVH` system queues a subtree refit (used by raycast/collision acceleration)

None of that requires changing the transform’s stored local values.

## Relationship to `SetTransform` / `UpdateTransform` traversal
There are three different “shapes” here:

1. **`SetTransform` (high-level intent)**
   - Executed by the intent executor.
   - Uses `collect_transform_targets(...)` to find transform components:
     - If the target is itself a `TransformComponent`, it is the target.
     - Otherwise, it walks the subtree and collects the **first** `TransformComponent` encountered per branch.
   - Mutates `TransformComponent.transform.*` and then emits `UpdateTransform` per resolved transform.

2. **`UpdateTransform` (mutation intent)**
   - Executed by the mutation executor.
   - Does **not** traverse descendants.
   - Directly calls `systems.update_transform(world, visuals, component, t)` for each `component_id`.
   - This *updates transform values* (sets `transform_comp.transform = t`) and then calls `transform_changed`.

3. **`RefreshTransform` (mutation intent)**
   - Executed by the mutation executor.
   - Does **not** traverse descendants.
   - Calls `systems.transform_changed(world, visuals, component)`.
   - This recomputes caches but does not alter the transform value.

The key difference: topology refresh needs (3), not (2).

## Why not just keep using `UpdateTransform` with identical values?
Because intent routing can rewrite recipients. A refresh intent must be semantically “about this component’s caches” and must not be redirected to another component.

Even in non-routed cases, `RefreshTransform` is also clearer: it communicates intent (recompute derived state) without implying a value mutation.
