# Meshes in cat-engine: `CpuMesh`, `Renderable`, and `Instance` (data flow)

This doc explains how **CPU-side mesh data** (`CpuMesh`) moves through the engine into **renderer-ready instances** (`VisualWorld` / `InstanceHandle`), and why `Renderable.base_mesh` exists (raycasting + collision inference use it).

If you’re specifically interested in picking, see also `docs/bvh-and-raycast.md`.

## Glossary

- **`CpuMesh`** (CPU geometry)
  - A CPU-side vertex + index buffer representation (`vertices`, `indices_u32`) in `src/engine/graphics/mesh.rs`.
  - Used as authoring/staging data; the renderer uploads it to GPU buffers later.

- **`CpuMeshHandle`** (CPU geometry identity)
  - A stable-ish integer handle (`CpuMeshHandle(u32)`) used throughout ECS/gameplay code.
  - Handles are owned by `RenderAssets` and index into its `cpu_meshes` array.

- **`MeshHandle`** (GPU geometry identity)
  - A renderer-owned handle (`MeshHandle(u32)`), returned after upload.
  - Stored in `GpuRenderable` inside `VisualWorld`.

- **`Renderable`** (what to draw)
  - A small struct containing `mesh: CpuMeshHandle`, `base_mesh: CpuMeshHandle`, and `material`.
  - Think: “this entity wants to draw this geometry with this material”.

- **`RenderableComponent`** (ECS-side renderable)
  - ECS component containing a `Renderable` plus an `Option<InstanceHandle>`.
  - It is the bridge between gameplay topology and the renderer snapshot.

- **`InstanceHandle`** (a registered draw instance)
  - The ID returned by `VisualWorld::register(...)`.
  - Represents an entry in `VisualWorld.instances: Vec<VisualInstance>`.

- **`VisualInstance`** (renderer snapshot)
  - The renderer-facing struct stored in `VisualWorld` containing:
    - `renderable: GpuRenderable` (GPU mesh + material)
    - transform/model matrix data
    - color/opacity/background flags/etc

## The short version

1. You create or import a `CpuMesh` (built-in primitive, generated, or glTF-imported).
2. You register it in `RenderAssets` and get a `CpuMeshHandle`.
3. You attach a `RenderableComponent` to the ECS world that references that `CpuMeshHandle`.
4. `RenderableSystem` resolves that CPU mesh to a GPU `MeshHandle` (uploading once, cached), then registers an instance in `VisualWorld` and writes back the `InstanceHandle` onto the `RenderableComponent`.

After that, the renderer draws the `VisualWorld` snapshot.

## Where `CpuMesh` comes from

### Built-in primitives

`RenderAssets::new()` pre-registers a set of built-in meshes (triangle, quad, cube, …) using `MeshFactory`.

Those meshes correspond to stable `CpuMeshHandle` constants in `src/engine/graphics/primitives.rs` (e.g. `CpuMeshHandle::QUAD_2D`). The stability matters because scenes serialize mesh ids.

### Imported meshes (glTF)

Imported meshes are registered into `RenderAssets` and keyed by a stable string (so systems can look them up again after import).

### Dynamically registered meshes

Some systems generate a fresh `CpuMesh` at runtime and register it via `RenderAssets::register_mesh(...)`.

## `RenderAssets` is the bridge: CPU handle -> GPU handle

`RenderAssets` owns:

- `cpu_meshes: Vec<CpuMesh>`
- `gpu_meshes: HashMap<CpuMeshHandle, MeshHandle>` (upload cache)

The upload path is:

- `RenderAssets::gpu_mesh_handle(uploader, cpu_mesh_handle)`
  - If already uploaded: return cached `MeshHandle`.
  - Else: fetch the `CpuMesh` and call `uploader.upload_mesh(mesh)`.

The key point: gameplay/ECS generally only needs `CpuMeshHandle`, not GPU buffers.

## `Renderable` vs `RenderableComponent`

### `Renderable` (graphics primitive request)

Defined in `src/engine/graphics/primitives.rs`:

- `mesh: CpuMeshHandle` — the CPU mesh identity used for *rendering*
- `material: MaterialHandle` — shader/pipeline selection
- `base_mesh: CpuMeshHandle` — the semantic “shape identity” this renderable came from

### `RenderableComponent` (ECS + runtime handle)

Defined in `src/engine/ecs/component/renderable.rs`:

- `renderable: Renderable` — what to draw
- `handle: Option<InstanceHandle>` — the VisualWorld instance once registered

`RenderableComponent::init()` doesn’t immediately create a renderer instance; it queues a registration command. The registration happens during command flush / render preparation.

## `Instance` / `VisualWorld`: what the renderer consumes

`VisualWorld` is the renderer snapshot.

- When `RenderableSystem` decides a renderable is ready, it calls `VisualWorld::register(...)`.
- This stores a `VisualInstance` containing a `GpuRenderable { mesh: MeshHandle, material }` plus per-instance attributes.
- `VisualWorld::register(...)` returns an `InstanceHandle`, which `RenderableSystem` stores back into the ECS `RenderableComponent.handle`.

This split is intentional:

- ECS owns *topology* and *authoring intent*.
- `VisualWorld` owns a compact, renderer-friendly list of instances.

(See `docs/render-phases.md` for how instances become draw batches.)

## Why `base_mesh` exists

Sometimes the mesh used for rendering is a **derived clone** of a simpler mesh. Today, the main case is UV overrides:

- The engine can “bake” per-instance UV overrides by cloning a base `CpuMesh` and writing new UVs.
- That cloned mesh gets a new `CpuMeshHandle` for rendering.
- But many non-render systems want a stable *semantic* shape (“this is still a quad”).

So we keep:

- `Renderable.mesh` = the actual render-time CPU mesh handle (possibly a cloned mesh)
- `Renderable.base_mesh` = the original “shape identity” handle

`RenderableSystem` enforces this:

- when it bakes UVs, it sets `renderable.mesh = new_mesh` and `renderable.base_mesh = uv_base_mesh`.

### Raycasting BVH uses `base_mesh`

Broadphase AABB computation and fallback ray tests use `RenderableComponent.renderable.base_mesh`, not `mesh`.

Rationale:

- UV-baked variants should still be pickable as the original primitive.
- AABB computation is only implemented for a few primitive mesh handles, so using the base handle keeps behavior stable.

Where it happens:

- BVH AABB build/refit: `BvhSystem::compute_aabb_for_renderable` uses `r.renderable.base_mesh`.
- Brute-force raycast fallback: `RayCastSystem` computes AABBs using `r.renderable.base_mesh`.

### Collision inference uses `base_mesh`

Collision shape resolution can fall back to the sibling renderable’s `base_mesh` when no explicit `CollisionShapeComponent` is present.

Current behavior:

- If a `CollisionComponent` has a sibling `RenderableComponent` and `base_mesh` is:
  - `CpuMeshHandle::CUBE` => inferred `CollisionShape::CUBE()`
  - `CpuMeshHandle::SPHERE` => inferred `CollisionShape::SPHERE()`

This keeps collision authoring light for simple primitives, and avoids coupling collisions to UV-baked mesh clones.

## Practical patterns

### “Normal” mesh instance

- `Renderable.mesh == Renderable.base_mesh`
- Both point to a built-in or imported `CpuMeshHandle`.
- Raycast/collision inference sees the same shape identity you render.

### Text / glyph-like rendering (UV-baked)

- Render-time mesh may be a UV-baked clone (new `CpuMeshHandle`).
- `Renderable.base_mesh` remains `CpuMeshHandle::QUAD_2D`.
- Picking/collision treat it as a quad.

## Current limitations (worth knowing)

- BVH-backed picking is AABB-based and only has real bounds for a small subset of primitive `CpuMeshHandle`s.
- For many imported meshes, raycasting won’t hit because the AABB code path doesn’t compute bounds from arbitrary mesh vertex data yet.
- Collision inference from `base_mesh` only covers a couple shapes; anything more complex should attach a `CollisionShapeComponent`.

## Related docs

- `docs/bvh-and-raycast.md` (raycast BVH + cursor ray details)
- `docs/render-phases.md` (how `VisualWorld` instances get batched and drawn)
- `docs/spec/skinned-mesh-system.md` (skinning pipeline; touches `CpuMesh.joints0/weights0` and instance flow)
