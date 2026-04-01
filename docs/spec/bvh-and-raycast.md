# BVH + Raycast — data flow

This document describes how cat-engine’s **renderable BVH** and **raycasting** work today: what data each system owns, how it stays in sync, and what the raycast query actually tests.

For the broader pointer/trigger/gesture pipeline, see `docs/spec/pointer-input-ray-gesture.md`.

For how meshes/handles flow through rendering (and why `base_mesh` exists), see `docs/mesh.md`.

## High-level idea
If you’re looking for future picking improvements (gizmo handles like rings/cones), see:
- `docs/analysis/raycast-circles-and-cones.md`

- `BvhSystem` maintains a broadphase acceleration structure over **world-space AABBs** of *renderable* components.
- `RayCastSystem` turns the cursor + active camera into a **world-space ray**, then uses `BvhSystem` to find the closest hit.

This is intended for **picking / UI-ish hit testing** (cube/quad/triangle primitives) rather than triangle-accurate mesh picking.

Important limitation: the BVH/raycast AABB computation is only implemented for a few builtin `base_mesh` types (`Cube`, `Quad2D`, `Triangle2D`). Other meshes currently have no usable AABB in the picking path, so they effectively won’t be hit.

## Planned improvement: “best-effort AABB first”, narrow-phase later

The next practical step for picking (especially gizmos) is to make BVH eligibility less “all or nothing”:

1. Expand AABB support beyond just rectangular / rectangular-prismatic primitives.
  - Goal: *every* renderable that opts into `RaycastableComponent` gets some reasonable broad-phase AABB.
  - For known primitives (tetrahedron, cone, ring, etc): define a local-space bounding box once and transform it.
  - For arbitrary CPU meshes (imported or generated): compute/store a local-space bounds from vertex positions at registration/import time, then transform it.
  - This alone makes these renderables show up as BVH candidates (even if narrow-phase is still “just AABB”).

2. After a BVH AABB hit, optionally run a narrow-phase shape test.
  - Initially this can be stubbed out: accept the AABB hit as the “hit” so we can iterate on broad-phase coverage first.
  - Later, for non-boxy gizmo handles, add analytic tests (ring/cone/capsule) to reduce false positives.

This keeps the system incremental: first make things selectable at all, then make them selectable *accurately*.

## Key participants

### Components

- `RenderableComponent`
  - Represents a drawable instance.
  - The BVH indexes these (by `ComponentId`).

- `TransformComponent`
  - Owns `transform.model` (local) and `transform.matrix_world` (cached world matrix).
  - `TransformSystem` keeps `matrix_world` correct.


- `RaycastableComponent`
  - Explicit opt-in/opt-out for picking.
  - A renderable is only eligible for the raycast BVH when a `RaycastableComponent` is found either:
    - as an immediate child under the `RenderableComponent`, or
    - on some ancestor in the topology (nearest ancestor wins).

  Future direction (design): keep `RaycastableComponent` as “eligible yes/no”, but resolve a separate pick shape (`RaycastableShapeComponent`) to unify broad-phase AABB generation + narrow-phase tests. In general, doing this resolution only in `RaycastableComponent::init()` is fragile (init ordering, ancestry-based opt-in, base_mesh changing), so a flush-time/on-demand resolution step is usually more robust.

  **Topology note:** for “ancestor enables a subtree” to work, the `RaycastableComponent` must be in the *ancestor chain* of the renderable (i.e. the renderable must be parented under that raycastable node). Merely attaching a `RaycastableComponent` as a sibling under the same transform does not make it an ancestor and will not be seen by the eligibility walk.

  Practical guideline:
  - Use per-renderable raycastable (the common `renderable -> raycastable` child pattern) when each leaf should be independently pickable.
  - Use one raycastable per clickable subtree by inserting a `RaycastableComponent` node above the subtree (so multiple leaf renderables inherit eligibility).

- `RayCastComponent`
  - A *request/behavior* component.
  - Has a `mode` (`Continuous` or `EventDriven`) and `max_distance`.
  - It does **not** define a ray origin/direction itself.
  - In current behavior, non-desktop pointers should be treated as request-driven; desktop cursor pointers still auto-cast from desktop mouse input.

### Systems

- `CommandQueue`
  - Batches component registrations/removals and other mutations.
  - Flush is where BVH rebuild/refit work is applied.

- `TransformSystem`
  - Event-driven: `transform_changed()` recomputes cached world matrices and updates `VisualWorld` model matrices for renderables.

- `BvhSystem`
  - Owns `shapes: Vec<RenderableAabb>` and `bvh: Option<BVH>`.
  - Applies queued changes in `flush_pending()`.

- `RayCastSystem`
  - Owns `raycasters: HashSet<ComponentId>`.
  - Resolves ray source per raycaster from topology.
  - Queries `BvhSystem` to find the best (closest) hit.

### Data sources

- `VisualWorld`
  - Supplies `camera_view()`, `camera_proj()`, and `viewport()`.
  - These are used to unproject the cursor into world space.

- `InputState`
  - Supplies `cursor_pos`, `mouse_pressed`, `mouse_down`, `mouse_released`.

## Coordinate/math details (cursor → ray)

For cursor-through-camera pointers, the ray is computed in `RayCastSystem::ray_from_cursor`:

1. Read viewport `(w, h)` from `VisualWorld`.
2. Read cursor `(cx, cy)` from `InputState` (defaults to screen center if missing).
3. Convert to Vulkan NDC:
   - $x_{ndc} = 2 (cx / w) - 1$
   - $y_{ndc} = 1 - 2 (cy / h)$
   - $z \in [0,1]$
4. Build clip-space points:
   - `near_clip = (x_ndc, y_ndc, 0, 1)`
   - `far_clip  = (x_ndc, y_ndc, 1, 1)`
5. Unproject using $inv(proj \cdot view)$ to world space.
6. Ray origin = near point; direction = normalize(far - near).

## BVH data model

### Shapes

`BvhSystem` stores one shape per raycastable renderable:

- `RenderableAabb { component: ComponentId, aabb: AABB, node_index }`
- `index_by_component: HashMap<ComponentId, usize>` maps component → shape index.

### Which renderables are indexed?

A renderable is only added if `renderable_is_raycastable(world, renderable_cid)` returns true:

- Explicit opt-in only: BVH will only include a renderable if a `RaycastableComponent` is present (immediate child or ancestor).
- If multiple `RaycastableComponent`s exist in the ancestry chain, the nearest ancestor to the renderable wins.

### How AABBs are computed

`compute_aabb_for_renderable()` uses:

- `RenderableComponent.renderable.base_mesh` (important: it uses the *base mesh*, not UV-baked variants — see `docs/mesh.md`)
- The nearest cached world matrix from `TransformSystem::world_model(world, renderable_cid)`

Then it calls `aabb_from_world_matrix_for_mesh(mesh, world_model)`.

Current implementation detail / limitation:

- Only a few primitive meshes produce a real AABB:
  - `CpuMeshHandle::CUBE`
  - `CpuMeshHandle::QUAD_2D`
  - `CpuMeshHandle::TRIANGLE_2D`
- Anything else returns `None`, which becomes a **placeholder AABB** placed extremely far away so it won’t be hit.
- If a renderable is not under any `TransformComponent`, `world_model()` returns `None` and it also falls back to the placeholder.

So today, BVH-backed picking is effectively “primitive picking”.

## Keeping the BVH in sync

BVH updates are **event-driven** and applied during `CommandQueue::flush`.

### Events that queue BVH work

- Renderable added:
  - `SystemWorld::register_renderable()` calls `bvh.queue_renderable_added(renderable_cid)`.

- Renderable removed:
  - `SystemWorld::remove_renderable()` calls `bvh.queue_renderable_removed(renderable_cid)`.

- Transform subtree changed:
  - `SystemWorld::transform_changed()` calls `bvh.queue_transform_subtree(world, transform_root)`.
  - This walks the subtree and marks any `RenderableComponent` descendants for refit.

### When the BVH is actually rebuilt/refit

At the end of `CommandQueue::flush`, cat-engine calls:

- `systems.bvh.flush_pending(&*world)`

This:

- Commits pending adds (building AABBs, inserting shapes)
- Updates AABBs for pending refits
- If topology changed (add/remove), does a full `BVH::build(&mut shapes)`
- Else performs `bvh.optimize(pending_refit_shape_indices, &shapes)`

Important note: `BvhSystem::tick` is essentially a no-op; **flush_pending is the real update point**.

## RayCastSystem data flow

### Registration

- Adding a `RayCastComponent` queues `REGISTER_RAYCAST` via its `init()`.
- When processed, `RayCastSystem::register_raycast` inserts the component id into `raycasters`.

### Per-frame behavior

`SystemWorld::tick` calls:

- `raycast.tick_with_queue(world, visuals, input, queue, &bvh, dt_sec)`

`tick_with_queue` does:

1. Build cursor ray from `VisualWorld` camera view/proj and `InputState` cursor position.
2. For each registered raycaster (`RayCastComponent`):
   - Check mode:
     - `Continuous`: cast every frame
     - `EventDriven`: cast only on `MouseButton::Left` press edge
   - Cast using BVH first:
     - `bvh.raycast_renderables(origin, dir, max_distance)`
   - Fallback to brute-force AABB tests if BVH returns nothing.
   - Print hit/no-hit messages and track `last_hit` for continuous mode.
3. Optional debug side effect: on click, it can “highlight” the hit renderable by upserting a `ColorComponent` and queuing `REGISTER_COLOR`.

### What “hit testing” means

The hit test is ray-vs-AABB only:

- BVH traversal provides candidate shapes.
- For each candidate, it computes the nearest positive intersection distance `t` with the AABB (“slab test”).
- Chooses the candidate with smallest `t`.

There is no per-triangle mesh intersection here.

## Why gizmos can “block” clicking the thing they’re attached to

It’s common to expect that a larger object (cube/tetrahedron) should be easier to click than thin gizmo parts (rings/stems/cones). However, with the current AABB-only picker, gizmo parts can win hits frequently.

Key reasons:
- **World-space AABBs are axis-aligned.** When a thin or flat part is rotated, the smallest axis-aligned box that contains it can become much larger than the visual geometry.
- **The picker chooses the closest hit by ray parameter $t$.** “Biggest AABB” doesn’t matter; the first thing along the ray that intersects an AABB is the hit.
- **AABB-only implies false positives.** A ray can intersect a gizmo part’s AABB even when it does not intersect the intended clickable shape (e.g. a ring annulus).

### Narrow-phase note (important)

Adding narrow-phase analytic tests (ring annulus / cone / capsule) is necessary to reduce false positives, but it must be paired with a query strategy that can try more than one candidate.

If the BVH query returns only a *single* nearest AABB hit and narrow-phase rejects it, the raycast must then continue searching for the next-best candidate (or gather multiple candidates up front). Otherwise the result becomes “no hit” even if a valid object behind the gizmo should be clickable.

## About parenting RayCastComponent to transforms (your question)

`RayCastSystem` infers a ray source from topology:

- If the nearest ancestor `TransformComponent` also has a camera component under it (`Camera3DComponent` or `Camera2DComponent`), it casts **cursor-through-active-camera** (the classic picking ray).
- Otherwise, it casts **forward** along the nearest ancestor transform’s -Z axis (parent-local forward), using that transform’s world pose.

Practical implication:

- Attaching `RayCastComponent` under a camera rig transform will generally produce cursor picking.
- Attaching it under a non-camera transform will produce a “controller-like” forward ray.

## Common gotchas

- If you’re trying to pick a complex imported mesh, it likely won’t hit: only a few `CpuMeshHandle` primitives generate AABBs today.
- If your renderable isn’t under a transform (no ancestor `TransformComponent`), it gets a placeholder AABB and won’t be hit.
- If nothing ever hits, double-check you’ve explicitly opted in with `RaycastableComponent` (either on the renderable or an ancestor).

## Suggested next steps (if you want better picking)

- Add AABB computation for arbitrary meshes (imported vertex bounds) so BVH covers glTF geometry.
- Add per-mesh BVH (or triangle-level intersection) for accurate hits.
- Extend `RayCastComponent` with a “ray source” mode (e.g. from a `TransformComponent` or XR controller pose) in addition to cursor/camera.
