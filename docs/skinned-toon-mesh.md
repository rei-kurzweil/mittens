# Skinned glTF: systems, components, and data flow (current implementation)

This document replaces the older “proposed” skinning notes and describes the **current, working** end-to-end pipeline for skinned glTF files in cat-engine.

Scope:
- Importing glTF skins (`JOINTS_0` / `WEIGHTS_0`, joints list, inverse bind matrices)
- ECS components/systems involved
- How joint transform updates propagate to GPU-visible resources
- How the renderer allocates and uploads the shared bones palette

Non-goals:
- Authoring/retargeting animation assets
- Advanced GPU skinning features (dual quaternion skinning, morph targets)

## Glossary

- **Joint**: a glTF node used by a skin; provides a world transform.
- **Skin**: glTF `skin` object containing `joints[]` and `inverseBindMatrices[]`.
- **IBM**: inverse bind matrix for a joint.
- **SkinMat**: the per-joint matrix uploaded to the GPU (mesh-local).
- **Bones palette**: a single global SSBO containing concatenated SkinMat arrays for all skinned instances.

## Key files / modules

ECS (import + runtime):
- `src/engine/ecs/system/gltf_system.rs` (GLTFSystem: imports meshes/textures/skins and spawns node trees)
- `src/engine/ecs/component/skinned_mesh.rs` (SkinnedMeshComponent)
- `src/engine/ecs/system/skinned_mesh_system.rs` (SkinnedMeshSystem: computes SkinMat and updates VisualWorld)
- `src/engine/ecs/system/transform_system.rs` (TransformSystem: maintains cached world matrices; emits transform-change events)
- `src/engine/ecs/system/system_world.rs` (SystemWorld: dispatches transform changes and render preparation)
- `src/engine/ecs/system/renderable_system.rs` (RenderableSystem: creates VisualWorld instances + instance handles)

Graphics (shared state + renderer):
- `src/engine/graphics/mesh.rs` (CpuMesh stores optional `joints0`/`weights0`)
- `src/engine/graphics/skin.rs` (Skin + SkinId; shared skin definitions)
- `src/engine/graphics/visual_world.rs` (VisualWorld: owns `skins`, bones palette allocation, and per-instance `(bones_base, bones_count)`)
- `src/engine/graphics/vulkano_renderer.rs` (Vulkano renderer: uploads palette SSBO + binds skinned pipeline)

Shaders:
- `assets/shaders/skinned-toon-mesh.vert` (vertex skinning)

## What each ECS component “means”

### `GLTFComponent`

Represents a glTF instance placed in the ECS world. GLTFSystem uses it to:
- Import resources (meshes/textures/skins) by URI
- Spawn a node/renderable subtree under a transform “anchor”
- Register per-instance joint resolution for each skin into SkinnedMeshSystem

### `TransformComponent`

Holds a local model matrix and a cached world matrix (`matrix_world`). TransformSystem keeps `matrix_world` up to date.

### `RenderableComponent`

Represents a renderable primitive instance. RenderableSystem creates a VisualWorld instance for it and stores an `InstanceHandle` on the component.

### `SkinnedMeshComponent`

Attached as a descendant of a `RenderableComponent` when the underlying glTF node uses a skin.

Fields:
- `skin_index`: the glTF skin index within the source asset
- `skin_id`: runtime pointer to the shared skin definition stored in VisualWorld (`SkinId`)

## What each system emits/consumes

### GLTFSystem (import/spawn)

Primary responsibilities:
1) **Imports CPU meshes** into RenderAssets (including skin vertex attributes)
2) **Registers shared skins** in VisualWorld (`upsert_skin(uri, skin_index, joints, ibm)`)
3) **Resolves per-instance joint ComponentIds** and registers them with SkinnedMeshSystem via:
   `register_skin_instance_joints(gltf_component, skin_id, Vec<Option<ComponentId>>)`
4) Spawns node/renderable components (including `SkinnedMeshComponent`) and sets `sm.skin_id`.

Outputs:
- CPU mesh keys in RenderAssets (some meshes have `CpuMesh.joints0/weights0`)
- VisualWorld `Skin` entries (shared; de-duplicated by `(uri, skin_index)`)
- SkinnedMeshSystem `instance_joints[(gltf_component, skin_id)]`

### TransformSystem + SystemWorld::transform_changed (pose propagation)

Transform changes are **event-driven**.

When a transform is updated (from animation, REPL, user code, etc.), SystemWorld calls:
1) `TransformSystem::transform_changed(...)` which recomputes cached world matrices for the subtree and updates VisualWorld instance model matrices.
2) `SkinnedMeshSystem::transform_subtree_changed(world, root_transform)` to mark any affected skins dirty.

This is the bridge from “joint moved” → “skinning needs recompute”.

### SkinnedMeshSystem (SkinMat computation + palette updates)

SkinnedMeshSystem performs two jobs:

1) **Binding discovery**: each tick it scans for `SkinnedMeshComponent` and groups renderables into a `BindingKey`:
- `mesh_transform`: the TransformComponent that defines the mesh-local space
- `gltf_component`: the owning GLTFComponent instance (used to fetch instance_joints)
- `skin_id`: shared skin definition id

2) **SkinMat recomputation for dirty bindings**:
For each dirty binding, it computes mesh-local skin matrices using cached world transforms:

$$
SkinMat(j) = M_{meshWorld}^{-1} \cdot M_{jointWorld}(j) \cdot IBM(j)
$$

Then it pushes them into VisualWorld for each renderable instance:
- `visuals.set_skin_matrices(instance_handle, &skin_mats)`

Important behaviors:
- If prerequisite data isn’t ready yet (e.g. instance joints not registered or renderable handle missing), it **retries next tick**.
- Dirtying is incremental: only bindings whose mesh transform subtree or joint subtree changed are recomputed.

Outputs:
- Updates to VisualWorld bones palette content + per-instance `(bones_base, bones_count)`.

### RenderableSystem (instance creation)

RenderableSystem is responsible for creating `VisualWorld` instances and assigning `InstanceHandle`s.

This matters for skinning because:
- SkinnedMeshSystem can only call `VisualWorld::set_skin_matrices()` once a renderable has a valid `InstanceHandle`.
- SystemWorld’s `prepare_render()` flushes pending renderables (uploads meshes, inserts instances) before rendering.

## VisualWorld: shared skin registry + bones palette allocation

VisualWorld is the “graphics snapshot” that the renderer consumes.

### Shared `Skin` registry

`VisualWorld::upsert_skin(uri, skin_index, joint_node_indices, inverse_bind_matrices)`:
- De-duplicates skins by `(uri, skin_index)`
- Stores joint node indices and IBMs in a graphics-owned structure (`Skin`)

### Bones palette

VisualWorld owns:
- `bones_palette: Vec<mat4>` (CPU-side)
- a tiny free-list allocator so each instance keeps a stable `bones_base` when possible
- `instances[idx].bones_base` + `instances[idx].bones_count`

Contract:
- Palette element 0 is reserved as **identity** and never freed.
- Non-skinned instances have `(bones_base, bones_count) == (0, 0)`.

`VisualWorld::set_skin_matrices(handle, bones)`:
- Allocates (or reuses) a palette range sized `bones.len()`
- Copies the matrices into `bones_palette[bones_base .. bones_base + bones_count]`
- Marks `dirty_bones_palette = true`
- Marks `dirty_instance_data = true` if `(bones_base, bones_count)` changed

## GPU data layout and renderer-side flow

### Vertex data (skinning attributes)

Skinning inputs come from glTF `JOINTS_0` and `WEIGHTS_0` and are stored in `CpuMesh`:
- `CpuMesh.joints0: Option<Vec<[u16; 4]>>`
- `CpuMesh.weights0: Option<Vec<[f32; 4]>>`

Renderer upload behavior (Vulkano):
- Base vertex buffer contains position/normal/uv (`CpuVertex`)
- Skinning attributes live in a **separate vertex buffer binding** (`GpuSkinVertex`)

### Per-instance data

Instance buffer includes:
- instance model matrix columns
- material-ish overrides (color, emissive, opacity)
- **skinning range**: `i_bones_base`, `i_bones_count`

### Descriptor sets (bones palette)

The skinned vertex shader reads a shared SSBO containing all bone matrices:
- **set=2, binding=1**: bones palette SSBO (`mat4 bones[]`)

Indexing scheme:
- For a vertex with joint index `j`, the shader reads:
  `bones[i_bones_base + j]`
- Skinning is enabled only when `i_bones_count > 0`.

### Renderer synchronization: why the palette is “per-slot”

The bones palette buffer is updated by mapping and writing from the CPU. If the same GPU buffer were reused every frame, the CPU could write while the GPU is still reading, which triggers Vulkano’s `AccessConflict(DeviceRead)`.

Current solution in `src/engine/graphics/vulkano_renderer.rs`:
- The cached bones SSBO is **per frame slot**:
  - one buffer per swapchain image
  - plus extra slots for XR eyes
- Each frame/eye picks a slot and only writes that slot’s SSBO when needed.
- A `cached_bones_slot_valid[]` flag ensures each slot is initialized at least once, even if the palette stops changing.

## End-to-end timeline

### Import / spawn time

1) You add a `GLTFComponent` (with a URI) under some `TransformComponent` anchor.
2) GLTFSystem imports the glTF:
   - creates CpuMeshes (optionally with `joints0/weights0`)
   - creates ImportedSkins (joints + IBMs)
3) GLTFSystem spawns a node/renderable subtree.
4) GLTFSystem registers:
   - shared skins into VisualWorld (`SkinId`)
   - per-instance joint resolution into SkinnedMeshSystem (`instance_joints[(gltf_component, skin_id)]`)
   - `SkinnedMeshComponent.skin_id = Some(skin_id)` on each skinned primitive.

### First frame(s): handles + first palette upload

1) RenderableSystem flushes pending renderables in `SystemWorld::prepare_render()`, creating VisualWorld instances and `InstanceHandle`s.
2) SkinnedMeshSystem tick discovers bindings and will retry until the renderable has a handle.
3) Once handles exist, `VisualWorld::set_skin_matrices()` allocates palette ranges and sets each instance’s `(bones_base, bones_count)`.
4) Renderer uploads the palette SSBO for the active slot.

### When a joint transform changes

1) Some code updates a joint’s `TransformComponent` (animation, action, REPL, etc.).
2) SystemWorld calls `transform_changed(...)`:
   - TransformSystem recomputes cached world matrices for the subtree
   - SkinnedMeshSystem marks impacted bindings dirty (`transform_subtree_changed`)
3) SkinnedMeshSystem recomputes SkinMat for dirty bindings and calls `set_skin_matrices(...)`.
4) VisualWorld marks the palette/instance data dirty; the renderer sees that and updates the SSBO for the current slot.

## Debugging knobs

Useful environment variables:
- `CAT_DEBUG_SKIN_APPLY=1`: logs SkinnedMeshSystem binding/apply behavior
- `CAT_DEBUG_SKIN_SET=1`: logs VisualWorld palette range updates
- `CAT_DEBUG_SKIN_INSTANCE_RANGES=1`: logs per-instance bone ranges as seen by renderer
- `CAT_DEBUG_BONES_PALETTE=1`: logs bones palette SSBO uploads
- `CAT_DEBUG_SKIN_JOINT_ORDER=1`: logs skin joint ordering at import time

## Appendix: correctness notes

- Mesh-local skinning is intentional so the engine can keep using instancing (`M_instance`) after skinning.
- Normals are skinned in the vertex shader and should be renormalized.
- The CPU-side cached world matrices (`TransformComponent.transform.matrix_world`) are the authoritative source for `M_meshWorld` and `M_jointWorld`.
