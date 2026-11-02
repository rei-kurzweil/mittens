# Task: Hierarchical BVH for mesh intersection, GLTF, and `CombineMesh`

## Why

The existing spatial index answers the inexpensive question, “which
renderable bounds might this ray hit?”  It is a scene-level BVH over ECS
`RenderableComponent` AABBs.  It is deliberately not a triangle-intersection
acceleration structure.

That distinction was easy to overlook while scenes contained mostly simple
primitives.  It becomes necessary to make explicit for two cases now in
scope:

- a GLTF anime/avatar model can contain roughly 50,000–100,000 triangles
  across its primitives (and substantially more in more detailed assets);
- `CombineMesh {}` creates a new, potentially large, baked mesh and the
  default mode removes its source renderables, so the result no longer has a
  per-source entry in the current ECS BVH.

Testing every triangle in every candidate model after a scene-BVH hit would
turn selection and ray casts into an unacceptable linear scan.  An aggregate
`CombineMesh` bounds box alone fixes neither accurate picking nor selection
ownership.  The engine needs two levels of acceleration rather than one
recursive BVH that conflates scene instances and mesh triangles.

## Existing seams

- `BvhSystem` stores `RenderableAabb` shapes keyed by `ComponentId`, builds a
  BVH with the `bvh` crate, and exposes
  `raycast_renderables_candidates(...)`.  Its unit of indexing is an ECS
  renderable's world-space AABB.
- `RayCastSystem` asks that BVH for candidates and then runs a narrow-phase
  policy.  Its current generic fallback is bounds-oriented; it does not own a
  reusable arbitrary-triangle acceleration structure.
- `CpuMesh` (`src/engine/graphics/mesh.rs`) retains positions, normals, and
  `indices_u32`, providing the local-space triangles needed to build an
  immutable mesh BVH.
- GLTF import produces CPU meshes from primitives.  Skinning is a separate
  concern: the rendered vertices may be in a current animated pose rather
  than the static CPU mesh pose.
- `CombineMeshSystem` bakes descendant `CpuMesh` data into one output mesh,
  uses the first material for phase 1, and registers that output directly in
  `VisualWorld`.  In default mode it removes the source subtree.  The output
  presently has no `RenderableComponent`/`BoundsComponent` registration, so
  it is absent from `BvhSystem`, ray selection, and editor show-bounds.

## Target model: two-level traversal

Call this a *hierarchical BVH* in engine-facing documentation, but preserve
the ownership boundary between the two levels:

```text
ray in world space
  -> scene/instance BVH (dynamic world AABBs)
       -> renderable, GLTF primitive, or CombineMesh aggregate instance
            -> transform ray into that mesh's local space
                 -> cached mesh triangle BVH (local triangles)
                      -> triangle hit + barycentrics/normal
  -> resolve hit to authored owner and editor selection target
```

### Scene / instance BVH

Keep a dynamic top-level BVH over *pickable instances*.  An instance has a
world AABB, a world/local transform, an optional material/visibility policy,
and a stable selection owner.  It should support refitting when only its
world transform changes and rebuilding when instances are added or removed.

The current `BvhSystem` is the natural starting seam, but its API and data
model must no longer assume “instance equals `RenderableComponent`.”  Define
an explicit instance record/handle so a collapsed `CombineMesh` output and a
normal ECS renderable participate equally without fabricating ghost authored
renderables.

### Mesh triangle BVH

Build one immutable local-space triangle BVH per CPU mesh revision, cached by
the mesh asset/handle plus a content revision.  It contains triangle index
ranges and local AABBs, not world transforms.  Every instance of the same
mesh shares it.

After the broad phase, transform the ray once by the candidate's inverse
world matrix and traverse this local tree.  Convert the winning distance and
normal back consistently to world space.  This avoids rebuilding a triangle
tree for each duplicate/instance and avoids transforming tens of thousands of
triangles per ray.

For large GLTF primitives this is the critical scaling boundary: a ray should
visit a small hierarchy of local node bounds and only a small number of leaf
triangles, rather than linearly inspecting 50k–100k triangles.

## `CombineMesh` policy

### Phase 1 output registration

Give each successful combined output one top-level pickable instance with:

- aggregate local bounds from the baked `CpuMesh`;
- the outer `CombineMesh` transform as its world transform;
- the baked mesh's cached triangle BVH;
- selection ownership resolving to the `CombineMesh` component or its
  authored outer transform, not to deleted source nodes.

This makes default-collapse trusses clickable and gives Editor Settings “show
bounds” exactly one meaningful aggregate box.  The registration should be
owned by `CombineMeshSystem`/the spatial system and removed or replaced in
the same lifecycle transaction as the VisualWorld output.  It must not depend
on a dummy `RenderableComponent` solely to enter the BVH.

### `keep_transforms()` provenance

The baked mesh needs a triangle-range provenance table in addition to its
triangle BVH:

```text
baked triangle range -> source component / source mesh handle / source triangle range
```

With this mode, select the aggregate by default but retain enough information
to offer deliberate source-level selection, inspection, or debugging later.
The policy must be explicit: source transform editing causes rebake, and any
stale output spatial entry is replaced atomically after the new bake.

Default collapsed mode may discard that table after choosing a stable owner,
unless future tooling requires source attribution after collapse.

## Animated and skinned GLTF policy

The static local mesh BVH above is correct for rigid meshes.  It is not by
itself exact for skinned vertices: a triangle's current pose can move relative
to the bind-pose tree every animation frame.

Before implementing skinned `CombineMesh`, choose and document one of these
policies (or a staged combination):

1. **Current-pose CPU snapshot:** evaluate/sketch the current skinned
   vertices, build or refit a per-pose triangle BVH, and use that snapshot for
   exact CPU ray hits.  This is the most direct compatibility path for phase
   2 CombineMesh baking, but needs memory, revision, and update-budget limits.
2. **Coarse skinned bounds then exact snapshot on demand:** maintain
   conservative broad-phase bounds per skinned primitive; only snapshot and
   traverse detailed geometry for candidates.  This may be the practical
   default for avatars.
3. **Renderer-assisted picking:** use an ID/depth pass for rendered-surface
   picking, while retaining CPU BVHs for physics, pointers, and non-rendered
   ray queries.  Define differences in occlusion and latency clearly.

Do not treat a bind-pose mesh BVH as exact animated intersection.  If it is
temporarily used as a broad phase, label the resulting picking approximation
and provide a conservative/current-pose fallback.

### Proposed default: lazy current-pose triangle snapshots

An alternative that limits detailed work to bone-associated candidate regions is
specified in [bone-oriented skinned mesh raycasting](../spec/skinned-mesh-raycasting.md).
It precomputes overlapping triangle lists from all retained skin influences and
uses conservative moving boxes before on-demand triangle evaluation. Compare
that approach with the whole-primitive snapshot below before selecting the
implementation; neither proposal is implemented for GLTF yet.

Discussion checkpoint, 2026-11-01: prefer the second policy for precise GLTF
ray hits. Joint/armature changes invalidate the detailed snapshot, but do not
immediately deform every mesh or refit every triangle tree. The first query
that needs that primitive's detailed geometry brings its snapshot up to date;
subsequent queries reuse it until a relevant input changes. This is a proposal,
not an implemented GLTF raycasting feature. The existing triangle BVHs for
damaged static cells do not automatically provide this behavior for GLTF.

The intended query path is:

```text
ray -> current conservative scene bounds -> candidate GLTF primitive
    -> compare deformation revision with cached snapshot revision
    -> if stale/missing: evaluate posed vertices and build/refit local BVH
    -> traverse current triangle BVH -> exact hit or miss
```

An AABB candidate is not yet a confirmed surface hit. If its triangles miss,
continue to other candidates; preserve nearest-hit ordering, distance limits,
and existing pointer/editor eligibility rules. A ray rejected by the outer
bounds must not cause detailed snapshot construction.

#### Invalidation and ownership

Keep immutable mesh topology and initial BVH connectivity shareable by mesh
asset revision. Own deformed vertices, refitted bounds, and snapshot validity
per independently posed instance/primitive. Two avatars using the same GLB
must not accidentally share a mutable posed tree. Identical-pose sharing is a
possible later optimization, not an initial assumption.

The snapshot's validity depends on:

- source mesh/topology and skin data, including inverse bind matrices;
- effective mesh-local joint palette, after animation, IK, tracking, spring
  motion, parenting, and other transform writers;
- effective morph weights and morph-target data, even if no joint moved;
- the instance binding identity and any replacement of the imported asset.

Use monotonic dirty/input revisions rather than hashing all vertices or
comparing every joint for each ray. Multiple writes before a query coalesce
into one refresh; multiple rays at the same revision reuse one result. The
existing `SkinnedMeshSystem` reverse indexes and dirty-binding notifications
are integration seams, not an existing deformation revision API. Invalidation
must survive renderer dirty flags being consumed. Avoid marking a snapshot
stale merely because an unchanged pose was submitted again.

Store the detailed geometry in mesh-local space. A rigid transform applied
equally to mesh and skeleton changes the world-space instance bounds and ray
transform, but not the local deformed geometry. It should reuse the snapshot.
Relative mesh/joint motion does invalidate it. Use the final effective palette
to distinguish those cases; a world-transform notification alone is not proof
that the local skin shape changed.

#### Conservative bounds must stay current

The broad phase cannot depend solely on the last detailed snapshot's bounds.
A limb can move outside those bounds before any ray requests an update, which
would prevent the exact query from ever running.

Choose and validate a cheaper conservative bounds path independently of the
lazy triangle cache: for example, precomputed per-joint influence bounds
transformed by the current palette and unioned per primitive. Account for
morph displacement, normalized skin weights, and the renderer's deformation
rules. An animation-wide envelope is another option only where a valid
envelope exists; authored animation bounds alone do not cover arbitrary IK or
tracking. Conservative bounds may include extra empty space, but must not
exclude possible current vertices. Unknown bounds require a conservative
fallback, not silently retaining an obsolete tight box.

Joint changes may therefore still do inexpensive palette/bounds work. The
expensive per-vertex deformation and triangle-BVH maintenance remain lazy.

#### Query timing, refitting, and cost

Exact means the current committed simulation pose at a defined query barrier,
not a promise to match a later render pose or XR late update. Consume all pose
changes up to that barrier before checking snapshot validity. CPU evaluation
must match renderer morph/skin ordering, weights, and matrix conventions.
Avoid GPU readback for this initial CPU query path.

With unchanged topology, update posed positions, recompute leaf bounds, and
refit parents bottom-up. Rebuild connectivity on topology changes, and consider
a measured quality threshold for badly degraded trees after large pose changes.
Do not rebuild connectivity on every joint change by default. A straightforward
full refit still costs work proportional to the primitive's triangles once per
queried deformation revision; laziness does not remove that cost. A continuous
pointer over a continuously animated avatar may refresh it every frame.

For an initial exact API, perform a required refresh synchronously when queried
and measure the first-query latency. A budgeted/deferred API may instead expose
a pending/approximate result explicitly; never label a stale-pose hit as exact
or silently report a miss because its refresh budget was exhausted. Introducing
asynchronous pose jobs is a separate implementation choice, not a requirement
to reuse the CSG worker.

Bound cache memory with inactive-entry eviction and release it on instance/asset
removal. An evicted entry rebuilds on its next precise query; dirty but unqueried
instances must not accumulate a history of pose snapshots. GPU/CPU visibility
alone cannot decide eligibility, since gameplay rays can target offscreen models.

#### Focused validation before implementation acceptance

- No detailed build/refit for unqueried instances or rays missing their outer
  bounds; repeated hits and misses at one pose reuse the same snapshot.
- Joint, ancestor, IK/spring, morph-only, and asset changes refresh the correct
  instance on its next query; several writes coalesce into one refresh.
- Rigid whole-avatar movement reuses local geometry; independent avatar poses
  never overwrite one another's cached geometry.
- A moved limb outside its old snapshot bounds is still discoverable. Test
  non-uniform transforms, morph extremes, and multiple skin influences.
- Compare BVH hits with a brute-force current-pose triangle reference, including
  holes/misses inside outer bounds and candidates behind a rejected near box.
- Track cache hits, invalidations, posed vertices, build/refit time, broad-phase
  bounds cost, memory, and eviction. Benchmark idle avatars, many animated but
  unqueried avatars, and a continuous pointer on a moving 50k–100k triangle avatar.

## Work plan

### A. Define common spatial instance and hit types

- Introduce `SpatialInstance` (or equivalent) independent of ECS
  `RenderableComponent`, with bounds, transform, mesh reference, visibility,
  and owner/selection routing.
- Extend the ray result beyond `(ComponentId, distance)` to carry instance
  identity, triangle index, barycentric coordinates, world normal, and the
  resolved authored owner as appropriate.
- Preserve existing UI raycast priority and `RaycastableComponent` rules;
  mesh intersection belongs only to the relevant 3D branch.

### B. Implement and cache local triangle BVHs

- Choose a robust triangle BVH builder/traverser (the existing crate if it
  supports this cleanly, otherwise a mesh-specialized implementation).
- Cache by immutable `CpuMesh` asset revision; invalidate when a generated
  combined mesh is replaced or an editable mesh changes.
- Define degenerate-triangle, missing-index, non-invertible-transform,
  back-face, and max-distance behavior.
- Add cache counters and timing seams so the future Engine Stats panel can
  report mesh-BVH count, triangle count, cache memory, build/refit work, and
  ray traversal candidates.

### C. Register normal, GLTF, and CombineMesh instances

- Migrate normal renderable registration into the common spatial-instance
  path without breaking current bounds/refit behavior.
- Register each collapsed `CombineMesh` output as one aggregate instance;
  update/remove it with output lifecycle and expose its aggregate bounds to
  the editor.
- For `keep_transforms()`, define whether only the aggregate is raycastable
  (recommended initially) or source instances are separately selectable;
  never permit duplicate hits accidentally.
- Use GLTF primitive boundaries where useful for culling/material ownership,
  while still sharing a mesh BVH for repeated mesh assets.

### D. Add skinned snapshot policy and budgets

- Start with the lazy current-pose proposal above: independent conservative
  broad-phase bounds, deformation invalidation, and query-triggered snapshots.
- Establish snapshot ownership/lifetime, update cadence, maximum triangles,
  and what happens when a budget is exceeded.
- Ensure a CombineMesh bake of skinned descendants explicitly consumes a
  current-pose snapshot, records its pose/revision, and does not silently bake
  bind-pose geometry.
- Benchmark a representative 50k–100k triangle animated avatar as both a
  normal GLTF scene and a CombineMesh source.

## Acceptance criteria

- A ray can pick a collapsed truss through one aggregate spatial instance;
  the hit resolves to its CombineMesh/outer authored owner and show-bounds
  displays the same aggregate bounds.
- Repeated rigid mesh instances share one local triangle BVH, while their
  transforms remain independent top-level instances.
- A large static GLTF primitive is triangle-tested through a local BVH, not a
  full triangle scan per candidate ray.
- Replacing/rebaking a CombineMesh neither leaks its old VisualWorld/spatial
  entry nor creates duplicate hits.
- Skinned mesh intersection has a documented current-pose accuracy policy;
  bind-pose BVHs are never represented as exact animated hits.
- Tests cover nearest-hit ordering, transformed/non-uniformly scaled meshes,
  aggregate bounds, owner resolution, stale-output removal, and shared-cache
  reuse.  Add benchmark coverage for the representative avatar and combined
  truss scenes.
