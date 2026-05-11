# Bounds & BVH flow (=^･ω･^=)

This doc describes how local-space AABBs move through the engine after the
`BoundsComponent` refactor — and how the BVH consumes the same source of
truth, so layout-time intrinsic sizing and raycast-time world-space culling
agree on what each mesh actually covers.

## Source of truth

`mesh_local_aabb(mesh: CpuMeshHandle) -> Option<Aabb>` in
`src/engine/graphics/bounds.rs` is the **only** place that maps a built-in
mesh handle to local-space extents. CUBE, QUAD_2D / TRIANGLE_2D / CIRCLE_2D
(thickened on z by 0.01 so flat primitives produce non-degenerate AABBs),
SPHERE, TETRAHEDRON, and CONE are tabulated; everything else (notably
GLTF-loaded meshes) returns `None`.

`Aabb` itself is a plain `{ min: [f32;3], max: [f32;3] }` with `width()`,
`height()`, `depth()`, `union(&other)`, `transformed(matrix)`, and
`inflated_z(half)` helpers.

## At renderable registration

```
SystemWorld::register_renderable(world, visuals, renderable_id)
  ├── renderable.register_renderable(...)       // VisualWorld instance
  ├── attach_bounds_for_renderable(world, id):  // ── new ──
  │     mesh = renderable.base_mesh
  │     local = mesh_local_aabb(mesh)?          // skip if None (GLTF etc.)
  │     bounds_id = world.add_component(BoundsComponent { local })
  │     world.add_child(renderable_id, bounds_id)
  ├── if raycastable: bvh.queue_renderable_added(id)
  ├── raycast.notify_renderable_added(...)
  └── clipping.register_renderable(...)
```

`BoundsComponent` lives **as a child of the renderable**, not the TC. This
mirrors `StencilClipComponent`'s placement convention (nearest-ancestor
renderable defines the shape) and means layout's "find the renderable, then
look at its child for bounds" walk is the same shape as the stencil-clip
walk.

## At raycast (BVH)

`BvhSystem::compute_aabb_for_renderable` still ends up at
`aabb_from_world_matrix_for_mesh`, but that function is now a one-liner:

```rust
fn aabb_from_world_matrix_for_mesh(mesh, m) -> Option<(min, max)> {
    let local = mesh_local_aabb(mesh)?;
    let world = local.transformed(m);
    Some((world.min, world.max))
}
```

No behavior change to raycast — same numbers, same AABBs, single source of
truth. (A future optimisation could read `BoundsComponent.local` directly
to skip the table lookup, but the table is tiny and constant; the win is
correctness consistency, not perf.)

## At layout (intrinsic sizing)

Two new hooks in `src/engine/ecs/system/layout/measure.rs`:

- `find_renderable_local_bounds(world, tc_id) -> Option<Aabb>` walks the TC's
  local-content subtree (stops at nested TCs, same rule as
  `find_text_in_local_content_subtree`), unions any `BoundsComponent.local`
  it finds among direct renderable children.
- `intrinsic_block_width(world, tc_id) -> Option<f32>` returns
  `bounds.width()` when the TC has renderable bounds and no descendant text.
  Text cells keep filling the inline budget so they can wrap.

Wired into `measure_item`:

```
width style == Auto && renderable_intrinsic_width.is_some()
    → content_width_gu = bounds.width()        // shrink-to-fit
    → is_auto_width = false                    // no second-pass remeasure

width style == Auto && no renderable bounds
    → content_width_gu = avail - margins - padding (fill remaining)
    → is_auto_width = true                     // inline ctx may remeasure
```

And `intrinsic_block_height` checks bounds between the text branch and the
child-items recursion:

```
text in subtree?    → text_intrinsic_height (line count)
renderable bounds?  → bounds.height()
child items?        → inline-flow sim or sum of block margin boxes
fallback            → descendant_layout_intrinsic_height
```

Renderable quads are centered (`-0.5..+0.5`), so a 1×1 mesh resolves to
`content_width = content_height = 1.0`. Padding wraps that footprint
naturally and bg quads end up symmetric.

## Sequence

```
register_renderable:
  Universe ──► SystemWorld.register_renderable(renderable_id)
                    │
                    ├── RenderableSystem.register_renderable
                    ├── attach_bounds_for_renderable
                    │     mesh_local_aabb(mesh) ──► Some(local)
                    │     world.add_component(BoundsComponent{local})
                    │     world.add_child(renderable_id, bounds_id)
                    ├── (maybe) bvh.queue_renderable_added
                    └── ...

layout tick (measure):
  LayoutSystem ──► measure_item(tc_id, avail_w)
                    │
                    ├── width Auto?
                    │     find_renderable_local_bounds(tc_id)
                    │       walk subtree, stop at nested TC
                    │       find RenderableComponent
                    │         find child BoundsComponent
                    │           return local.union(...)
                    │     ──► content_width = local.width()
                    │
                    └── height Auto?
                          intrinsic_block_height(tc_id, content_width)
                              text? text_intrinsic_height
                              bounds? local.height()
                              child items? inline-flow sim / block sum

raycast tick:
  RayCastSystem ──► BvhSystem.raycast_renderables(ray)
                    └── compute_aabb_for_renderable(cid)
                          aabb_from_world_matrix_for_mesh(mesh, world_mat)
                              mesh_local_aabb(mesh)?.transformed(world_mat)
```

## Future hook: CPU culling

`BoundsComponent` is a prerequisite for CPU-side culling that would
complement `StencilClipComponent`, but it isn't sufficient on its own.
Cheap "is this renderable inside the clip / frustum?" queries need a
spatial index — extending `BvhSystem` to cover non-raycastable renderables
and exposing a `query_aabb(...)` or `query_frustum(...)` API. The BVH
would seed its world AABBs from `BoundsComponent.local` (transformed by
`matrix_world`) instead of consulting `mesh_local_aabb` per call, which
gives us one path for mesh-extent data going into the index.

That's downstream work; this change just lands the bounds component and
the BVH refactor so the source-of-truth question is answered first.

🗿🍷🍷🍷
