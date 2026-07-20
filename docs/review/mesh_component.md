# `MeshComponent` and `RenderAssets` Review

## Mental model

Mittens has three different mesh identities at different stages:

- `CpuMesh` is vertex and index data owned on the CPU.
- `CpuMeshHandle` identifies a `CpuMesh` inside `RenderAssets`.
- `MeshHandle` identifies uploaded geometry owned by the renderer/GPU side.

An ECS `RenderableComponent` describes what should be drawn. A renderer-facing
instance does not exist until `RenderableSystem` resolves its CPU geometry, uploads
it if necessary, and registers it in `VisualWorld`.

```text
CpuMesh
  -> RenderAssets::register_mesh
  -> CpuMeshHandle
  -> RenderableComponent
  -> RenderAssets::gpu_mesh_handle
  -> MeshHandle
  -> VisualWorld instance
  -> InstanceHandle stored back on RenderableComponent
```

## What `RenderAssets` owns

`RenderAssets` is the bridge between CPU geometry and renderer geometry. It owns:

- all registered `CpuMesh` values
- the mapping from stable imported-mesh keys to `CpuMeshHandle`s
- the cache from `CpuMeshHandle` to uploaded `MeshHandle`
- stable built-in handles for triangle, quad, cube, sphere, and other primitives
- cached procedural meshes such as wireframe boxes and capsules

Built-in handles have intentionally stable numeric values because existing scene
serialization and shape inference rely on them. Imported and dynamically generated
handles are runtime allocations; their string keys or owning components provide the
meaningful identity.

CPU geometry is useful outside rendering. Bounds, collision diagnostics, inspection,
and other simulation systems can read it without waiting for GPU upload.

## `Renderable` versus `MeshComponent`

`RenderableComponent` contains a `Renderable` with:

- `mesh`: the CPU mesh currently used for rendering
- `base_mesh`: the semantic/original mesh before derived variants such as UV clones
- `material`: the renderer material/pipeline choice

For ordinary built-in and procedural renderables, `Renderable.mesh` directly names
the geometry and no `MeshComponent` is needed.

`MeshComponent` is a string-key override attached as an immediate child of a
`RenderableComponent`:

```text
RenderableComponent
└── MeshComponent { key: "bisket:Body:prim0" }
```

In the current engine, `RenderableSystem` resolves that key through
`RenderAssets::imported_mesh`. This is how GLTF primitives refer to imported CPU
geometry before a runtime numeric handle has been written onto the renderable.

The rule for systems inspecting geometry is:

> If a renderable has a `MeshComponent`, the key is authoritative. Do not interpret
> `Renderable.mesh` as the imported geometry until resolution has replaced it.

## Why GLTF renderables start as triangles

`Renderable` currently requires a concrete `CpuMeshHandle`; it has no unresolved or
optional mesh state. While building a GLTF component tree, `GLTFSystem` therefore
creates each primitive with `CpuMeshHandle(0)`, the built-in `TRIANGLE_2D`, and adds
the actual imported key in a child `MeshComponent`.

The triangle is structural initialization, not model geometry. It lets GLTF create
the complete ECS topology—transforms, renderables, materials, skins, and sidecars—
before renderer instance creation. `RenderableSystem` later replaces `mesh` and
`base_mesh` with the resolved imported handle.

This placeholder caused the AVC capsule bug because bounds read
`Renderable.mesh` before resolution and measured the valid triangle. Bounds now
checks `MeshComponent` first and never falls back to the placeholder.

## Current frame ordering

The important ordering is:

```text
GLTFSystem::tick_with_queue
  -> receive completed decode work
  -> create the GLTF ECS hierarchy and MeshComponent keys

GLTFSystem::flush_mesh_imports_only
  -> register imported CpuMesh values in RenderAssets

queue.flush
  -> apply component registration and transform commands

simulation systems, including AVC
  -> measure imported CPU geometry synchronously through RenderAssets

SystemWorld::prepare_render
  -> finish texture imports
  -> resolve pending renderables
  -> upload/cache GPU meshes
  -> register VisualWorld instances
```

The ordering guarantees that AVC can measure a newly spawned GLTF tree using the
real CPU meshes. AVC does not wait on `RenderableSystem`, `VisualWorld`, or GPU
upload, and `BoundsSystem` does not keep measurement state between ticks.

## What the pending-renderable collection means

`RenderableSystem::pending` is not a bounds queue and is not the primary GLTF load
state. It contains ECS renderables that do not yet have renderer-ready visual
instances.

During `flush_pending`, the system:

1. resolves an optional `MeshComponent` key to a `CpuMeshHandle`
2. applies UV-derived mesh variants when needed
3. updates `Renderable.mesh` and `base_mesh`
4. caches a local `BoundsComponent` from the resolved vertices
5. obtains or uploads the GPU `MeshHandle`
6. registers the `VisualWorld` instance
7. stores the returned `InstanceHandle` on `RenderableComponent`

This collection exists to defer renderer work until render preparation. CPU-side
bounds consumers do not depend on it.

## Bounds paths

There are two related bounds mechanisms:

- `BoundsSystem::measure_renderable_subtree_bounds` walks a requested ECS subtree,
  reads CPU meshes from `RenderAssets`, transforms each local AABB into the root's
  coordinate frame, and returns one aggregate outer AABB.
- `BoundsComponent` is a cached local AABB attached to an individual renderable after
  its mesh is resolved by `RenderableSystem`. BVH and GLTF bounds visualization can
  consume this cache.

AVC uses the first path because its capsule only needs CPU geometry and should not
depend on GPU/render-instance readiness.

The aggregate result is deliberately simple:

- `Measured(Aabb)` when geometry was measurable
- `Unmeasurable` when it was not

There is no list of descendant measurements and no retained measurement between AVC
ticks.

## `mesh` and `base_mesh`

Normally `mesh == base_mesh`. They differ when the render mesh is derived from
another mesh, particularly UV-overridden clones:

- `mesh` identifies the exact CPU geometry sent toward rendering
- `base_mesh` preserves the original semantic shape

Primitive collision inference and some fallback geometry logic use `base_mesh` so a
derived quad remains semantically a quad. Imported meshes receive their resolved
import handle for both fields before renderer registration.

## Useful code entry points

- `src/engine/ecs/component/mesh.rs` — `MeshComponent`
- `src/engine/ecs/component/renderable.rs` — `RenderableComponent` constructors
- `src/engine/graphics/primitives.rs` — mesh and renderer handle types
- `src/engine/graphics/render_assets.rs` — CPU registry and GPU upload cache
- `src/engine/ecs/system/gltf_system.rs` — GLTF keys, placeholder creation, CPU import
- `src/engine/ecs/system/renderable_system.rs` — resolution and visual registration
- `src/engine/ecs/system/bounds_system.rs` — synchronous subtree measurement

## Review takeaways

- A `MeshComponent` overrides the mesh identity of its owning renderable.
- The GLTF triangle is a placeholder and must not be treated as imported geometry.
- CPU mesh readiness and GPU renderable readiness are separate stages.
- `RenderAssets` is available to simulation systems; `VisualWorld` is the rendered
  snapshot.
- Pending renderables concern renderer instance creation, not AVC bounds readiness.
