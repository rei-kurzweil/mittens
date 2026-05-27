# SkinnedMeshSystem: skinning data flow, transforms, gizmos, and routing

This document describes the **current end-to-end runtime pipeline** for glTF skinning in cat-engine.

It supersedes the older shader-focused notes and aims to be the single place that explains:
- how glTF armatures/skins are imported and adapted to the ECS
- how transforms feed skinning (local `model` vs cached `matrix_world`)
- which systems/signals participate in keeping the GPU skinning palette updated
- how editor gizmos and signal pipelines interact with joints and viz proxies

For mesh/instance fundamentals (non-skinning), see `docs/spec/mesh.md`.

## Glossary

- **Joint**: a glTF node referenced by a skin; provides a world transform.
- **Skin**: a glTF `skin` object containing `joints[]` and `inverseBindMatrices[]`.
- **IBM**: inverse bind matrix (mesh->joint bind-space inverse) for a joint.
- **SkinMat**: the per-joint matrix uploaded to the GPU, in **mesh-local** space.
- **Bones palette**: a global SSBO containing concatenated `SkinMat[]` arrays for all skinned instances.

## Key idea: skinning uses cached world matrices

- `TransformComponent` stores:
  - `transform.model`: local matrix (derived from translation/rotation/scale)
  - `transform.matrix_world`: **cached world** matrix
- `TransformSystem::transform_changed(...)` recomputes `matrix_world` down a subtree.
- `SkinnedMeshSystem` uses `TransformSystem::world_model(...)`, which returns `matrix_world` (or the nearest ancestor transform’s `matrix_world`).

So: skinning is driven by **world-space** transforms, but computed from local values via the transform cache.

## Data flow table (systems, data, and signals)

| Stage | Producer | Data produced | Trigger / intent(s) | Consumer(s) | Notes |
|---|---|---|---|---|---|
| Import skins | `GLTFSystem` | `Skin` definitions (joints + IBMs) | glTF import/flush | `VisualWorld` | Stored as shared `SkinId` entries; de-duplicated per asset+skin index. |
| Import meshes | `GLTFSystem` | `CpuMesh` with optional `joints0/weights0` | glTF import/flush | `RenderAssets` | Skin vertex attributes are uploaded as separate GPU bindings. |
| Spawn ECS nodes | `GLTFSystem` | Transform/renderable subtrees; `SkinnedMeshComponent` | component init | ECS World | Skinned meshes are discovered by system scan (no explicit register intent). |
| Resolve joints (per instance) | `GLTFSystem` | `instance_joints[(gltf_component, skin_id)] -> Vec<Option<ComponentId>>` | import/spawn | `SkinnedMeshSystem` | Per-instance because multiple GLTF instances can share a `SkinId`. |
| Create instances | `RenderableSystem` | `InstanceHandle` for each `RenderableComponent` | `RegisterRenderable` / flush | `VisualWorld` | Required before `set_skin_matrices(handle, ...)` can succeed. |
| Change transform value | user code / gizmo / animation | local TRS updated | `UpdateTransform` | `MutationExecutor` → `SystemWorld` | `UpdateTransform` mutates a specific transform and calls `transform_changed`. |
| Topology change refresh | `Attach`/`Detach` intents | cache recompute request | `UpdateTransformWorld` | `MutationExecutor` → `SystemWorld` | **Does not modify any TRS**; avoids routing hazards. |
| Mark skins dirty | `SystemWorld::transform_changed` | dirty bindings | direct call | `SkinnedMeshSystem` | Uses reverse indices so only affected bindings recompute. |
| Recompute skin mats | `SkinnedMeshSystem` | `SkinMat[]` per binding | on tick if dirty | `VisualWorld::set_skin_matrices` | Computes: `inv(mesh_world) * joint_world * IBM`. |
| Upload palette | renderer | SSBO writes | frame build | GPU | Palette is per-frame-slot to avoid read/write hazards. |

## Skin matrix math (mesh-local palette)

Skinned meshes are skinned in mesh-local space so the engine can still use instance transforms normally.

For each joint $j$:

$$
SkinMat_j = M_{meshWorld}^{-1} \cdot M_{jointWorld}(j) \cdot IBM(j)
$$

- $M_{meshWorld}$ comes from the mesh transform’s cached `matrix_world`.
- $M_{jointWorld}(j)$ comes from each joint transform’s cached `matrix_world`.
- $IBM(j)$ comes from the glTF skin.

## Components involved

- `GLTFComponent`: identifies a glTF instance and is the key used for per-instance joint resolution.
- `TransformComponent`: local TRS + cached `matrix_world`.
- `RenderableComponent`: becomes a `VisualWorld` instance once flushed; stores an `InstanceHandle`.
- `SkinnedMeshComponent`: marks a renderable subtree as skinned; holds `skin_id` (shared `SkinId`).

## Systems involved (high level)

### `GLTFSystem`
Responsibilities for skinning:
- imports meshes with `JOINTS_0`/`WEIGHTS_0` into `RenderAssets`
- upserts shared skins into `VisualWorld` and gets a `SkinId`
- resolves joint nodes to spawned ECS `ComponentId`s and registers them with `SkinnedMeshSystem::register_skin_instance_joints(...)`
- attaches `SkinnedMeshComponent` under renderables, setting `skin_id`

### `TransformSystem` + `SystemWorld::transform_changed`
- recomputes cached `matrix_world` down the changed transform subtree
- updates `VisualWorld` model matrices for descendant renderable instances
- notifies `SkinnedMeshSystem` that a transform subtree changed (marking relevant bindings dirty)
- queues BVH refits and other transform-derived updates (collision/camera/light)

### `SkinnedMeshSystem`
Two responsibilities:
1) Discover bindings each tick (group skinned renderables by `(mesh_transform, gltf_component, skin_id)`)
2) For dirty bindings, compute skin matrices from cached world transforms and call `VisualWorld::set_skin_matrices(handle, ...)`

Important behavior:
- If instance joints are not registered yet, or renderables don’t have handles yet, it retries next tick.

### `VisualWorld`
Owns the renderer-facing skin state:
- shared skin registry (`SkinId` → joints + IBMs)
- bones palette allocator and CPU-side palette storage
- per-instance `(bones_base, bones_count)` range used by shaders

### Renderer
- uploads the bones palette SSBO (typically per swapchain/XR slot)
- binds the skinning descriptors and vertex buffers

## Signals and intent shapes (and why routing matters)

There are two different concepts that look similar but behave differently:

1) **`UpdateTransform` (mutation intent)**
   - executed by the mutation executor
   - applies directly to its explicit `component_ids`
   - mutates transform values and calls `transform_changed`
   - **routable**: `SignalPipelineProcessor` may rewrite `component_ids`

2) **`UpdateTransformWorld` (mutation intent)**
   - executed by the mutation executor
   - applies directly to its explicit `component_ids`
   - recomputes derived caches via `transform_changed` **without changing transform values**
   - **non-routable**: pipeline processor does not rewrite it

### Why `UpdateTransformWorld` exists
Topology operations (`Attach`, `Detach`, etc.) need to recompute cached world matrices after parent/child relations change.

Historically this used `UpdateTransform` as a “poke”, but once routing existed for `update_transform` (e.g. `viz:*` route-up), that poke could be redirected to a different component and overwrite joint values.

`UpdateTransformWorld` encodes the real intent: “recompute caches for *this* transform”, not “set this transform’s TRS again”.

## Editor gizmos + viz proxies (transform visualization mode)

### Viz proxies
When transform visualization is enabled for imported glTF nodes, the engine spawns:
- `viz_overlay:*` (grouping)
- `viz:*` (a transform used to position the viz box)
- `viz_box:*` (renderable)

Viz transforms can be configured with a route-up operator so editing the viz target affects the real joint/transform.

### Gizmo targeting
- The editor selects by clicking `viz_box:*` and reparents a gizmo under the nearest transform.
- `TransformGizmoSystem` mutates `TransformComponent` directly during drags (which emits `UpdateTransform`).
- To support viz proxies, gizmo targeting can be rerouted by reading `SignalRouteUpwardComponent` children under the selected `viz:*` transform.

### Routing hazards to avoid
- **Do not** use routable intents as “refresh” operations during topology changes.
- If you need to recompute caches after selection/reparenting, use `UpdateTransformWorld`.

## How glTF armatures are adapted to cat-engine

### glTF concepts
In glTF:
- A **skeleton/armature** is a node hierarchy.
- A **skin** references a list of joint node indices and a parallel array of inverse bind matrices.
- A skinned primitive references a skin.

### cat-engine adaptation
At runtime cat-engine maps glTF to ECS like this:
- glTF node transforms become `TransformComponent` nodes in the ECS topology.
- Skinned primitives become `RenderableComponent` instances, with a `SkinnedMeshComponent` somewhere under their subtree.
- Shared skin definitions (joint order + IBMs) live in `VisualWorld` and are referenced by `SkinId`.
- Per-instance joint resolution is stored in `SkinnedMeshSystem.instance_joints[(gltf_component, skin_id)]`:
  - a `Vec<Option<ComponentId>>` aligned to the skin’s joint order
  - `None` is allowed (joint node not spawned) so indices remain aligned

Two important nuances:
1) The “owning” `GLTFComponent` for a skin may not be a direct ancestor of the skinned primitive due to how the subtree is anchored; the system resolves the nearest applicable `GLTFComponent`.
2) Transform updates must reach `TransformSystem::transform_changed` so `matrix_world` stays correct; skinning reads world matrices, not local matrices.

## Debugging knobs

Common env vars:
- `CAT_DEBUG_SKIN_APPLY=1`: SkinnedMeshSystem binding/apply behavior
- `CAT_DEBUG_GIZMO_TARGET=1`: gizmo target rerouting logs
- `CAT_DEBUG_GIZMO_APPLY=1`: per-drag transform writes by gizmo
- `CAT_DEBUG_GIZMO_SANITY=1`: warns on NaN/Inf/huge transform values

## Related files

- ECS:
  - `src/engine/ecs/system/gltf_system.rs`
  - `src/engine/ecs/system/transform_system.rs`
  - `src/engine/ecs/system/skinned_mesh_system.rs`
  - `src/engine/ecs/system/gizmo_system.rs`
  - `src/engine/ecs/system/editor_system.rs`
  - `src/engine/ecs/rx/signal_pipeline_processor.rs`
  - `src/engine/ecs/rx/intent_executor.rs`
  - `src/engine/ecs/rx/mutation_executor.rs`
- Graphics:
  - `src/engine/graphics/visual_world.rs`
  - `src/engine/graphics/skin.rs`
  - `src/engine/graphics/mesh.rs`
- Shaders:
  - `assets/shaders/skinned-toon-mesh.vert`
