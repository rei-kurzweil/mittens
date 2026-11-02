# Shared mesh queries, posed avatar bounds, and physics snapshots

Status: proposed, 2026-11-02. Documentation only; each slice below should be
reviewable and useful independently. This plan builds on
[hierarchical mesh intersection](hierarchical-bvh-mesh-intersection.md) and
[bone-oriented skinned raycasting](../spec/skinned-mesh-raycasting.md).

## Intended ownership

Keep scene indexes for raycastable instances and collidable instances distinct.
Share immutable mesh geometry and triangle acceleration data underneath them.
Treat mesh changes as revisions, regardless of whether destruction, import,
procedural generation, or deformation produced them. Detailed geometry is
optional: ordinary movement collision can continue using simple shapes.

A GLTF component is an ownership boundary, not necessarily one acceleration
tree. Its primitives can share static mesh assets or expose per-instance posed
clusters. Do not build duplicate triangle trees merely because two query systems
consume the same geometry. Physics threading is a later ownership change, not
a prerequisite for nested mesh acceleration.

## Slice 1: extract the existing static triangle-query facility

- [ ] Move reusable triangle geometry, BVH construction, bounds, and query code
  out of destruction-specific ownership. Keep destruction behavior unchanged.
- [ ] Give mesh-query data explicit immutable ownership and a geometry revision.
  Retain direct/batched query paths; avoid per-triangle dynamic dispatch.
- [ ] Adapt damaged cells so collision and raycasting reference the same object.

Acceptance: existing cut-surface rays and contacts still pass through holes and
hit surviving surfaces. Assert shared acceleration ownership for both consumers,
empty-cell retirement, and release after the last reference. Compare exact ray
results with a brute-force triangle reference, including transformed geometry.
No avatar deformation or new thread in this slice.

## Slice 2: precise static GLTF raycasting

- [ ] Opt static GLTF primitives into the common mesh-query path, preserving
  material/primitive identity, authored hit ownership, and raycast eligibility.
- [ ] Build static triangle BVHs lazily on the first detailed candidate query;
  cache by mesh asset/content revision and share between rigid instances.
- [ ] Keep instance transforms and world bounds separate from local mesh data.

Acceptance: a ray through empty space inside a model's outer box misses it and
can hit a farther object. Compare hits against brute force for rotation,
nonuniform scale, distance limits, and nearest-hit ordering. Two rigid instances
reuse one mesh tree; moving either does not rebuild it. Replacing a mesh makes
the next query use its new revision and eventually releases the old cache.
Skinned meshes must not silently use bind-pose triangles as exact hits.

## Slice 3: common mesh data for precise static contacts

- [ ] Allow explicitly opted-in static collidables to reference the same mesh
  query data as raycasting, using the already supported contact query shapes.
- [ ] Retain separate scene membership and existing simple collision shapes.
- [ ] Generalize geometry replacement/retirement notifications beyond damage.

Acceptance: one static imported mesh serves both consumers without a second
triangle tree. Raycast-only objects do not block movement; collision-only
objects need not be selectable. Geometry replacement updates both consumers;
removing one consumer does not invalidate the other. Run existing static-contact
regressions, including cut surfaces. Do not expand this slice into a new rigid
body solver or unsupported collision-shape algorithms.

## Slice 4: import-time avatar triangle clusters

- [ ] Precompute compact triangle clusters and bone-to-cluster lists using all
  retained nonzero joint influences. Keep one owner per triangle, with several
  bones allowed to reference its cluster.
- [ ] Record every cluster's joint dependencies, morph dependencies, source
  vertices, and primitive/material provenance.
- [ ] Optional influence ordering must preserve complete candidate membership.

Acceptance: synthetic triangles spanning several joints appear in every relevant
candidate mapping without duplicate ownership. Include four influences per
vertex, differing influences at each triangle corner, and unweighted/fallback
vertices matching renderer behavior. Asset instances share this immutable data.
This slice validates preprocessing without changing production ray results.

## Slice 5: conservative bone-oriented candidate bounds

- [ ] Implement current conservative bounds for those clusters/regions, including
  blended joints and morph envelopes, without skinning every detailed vertex.
- [ ] Feed a conservative containing bound into the outer scene index. Refit
  ancestor containment where a hierarchy is used; flat box queries are allowed
  initially if the small box count makes them competitive.
- [ ] Connect pose dependency notifications and an explicit pose/query barrier.
  Bounds validity must not rely on renderer dirty flags remaining unconsumed.

Acceptance: for sampled and adversarial poses, all brute-force posed triangles
are enclosed by their assigned candidate bounds. Exercise elbow/hip blends,
moving limbs outside old bounds, IK, spring motion, morph extremes, and affine
transforms. Verify no reference ray hit is rejected. Measure bound maintenance
cost and looseness separately from detailed geometry work.

## Slice 6: lazy exact avatar hits with a vertex cache

- [ ] Add per-instance posed-position storage and validity tracking. Deform only
  vertices needed by candidate clusters, matching renderer skin/morph rules.
- [ ] Deduplicate candidate clusters, vertices, and triangle tests across bone
  lists with reusable scratch storage. Start with direct cluster triangle scans.
- [ ] Reuse results at an unchanged deformation revision; invalidate affected
  clusters through all influencing joints and morph inputs. Root rigid motion
  should reuse local posed geometry.
- [ ] Define exact as the committed simulation pose at the query barrier. A
  pending, approximate, or stale result must not masquerade as an exact hit.

Acceptance: compare nearest hits with brute-force current-pose triangles. Rays
missing candidate regions do no detailed deformation work. Multiple rays at one
pose reuse vertices; several pose writes before querying coalesce. Morph-only
changes invalidate correctly; independent avatar instances cannot corrupt each
other. Test a near box whose triangles miss and a farther actual surface hit.

## Slice 7: measure and bound the cache

- [ ] Instrument bounds updates, candidates, unique deformed vertices, triangle
  tests, cache hits, invalidations, memory, and first/repeated-query latency.
- [ ] Benchmark static GLTF, unqueried animated crowds, intermittent rays, and
  continuous pointers over representative 50k–100k triangle avatars.
- [ ] Compare direct cluster scans with whole-primitive lazy triangle refits.
  Add internal cluster BVHs only if measured cluster sizes/query costs justify
  them. Separate refitting bounds from rebuilding topology.
- [ ] Add inactive-entry eviction and deterministic release on instance/asset
  removal, with bounded reusable scratch and no retained pose history.

Acceptance: record reproducible before/after results and hardware/build mode.
Continuous queries over continuously changing poses must be measured explicitly;
do not infer their performance from idle cache hits. Eviction changes cost, not
correctness. No arbitrary claimed speedup or fixed cluster size before evidence.

## Slice 8: publish immutable query snapshots across threads

- [ ] Before moving simulation, define ownership of scene bounds, geometry
  revisions, poses, and query scratch. Keep immutable mesh assets shared.
- [ ] Prototype snapshot publication with a producer and reader. Readers retain
  one coherent generation while the producer prepares the next; no traversal
  observes a partially refitted tree or mismatched pose/mesh revisions.
- [ ] Define whether each query uses the latest completed physics tick, requests
  work on the physics owner, or has a separate reader-owned posed cache. Keep
  mutable caches private to their owner and make latency semantics explicit.

Acceptance: deterministic producer/reader tests cover retained old snapshots,
removal/replacement, stale job completion, repeated publication, shutdown, and
resource reclamation. Readers of an old generation remain valid until release.
Record memory retained by slow readers and establish a bounded handoff policy.
This slice does not move force integration or collision solving yet.

## Slice 9: move physics ownership in a separate change

- [ ] Move the chosen simulation workload, its collidable scene index, and its
  mutable query caches to the physics owner thread. Specify fixed-step ordering
  for forces, integration, contact, and published transforms.
- [ ] Define inputs/commands, acknowledgements, structural edits, and how main
  thread ray/pointer queries consume completed state. Preserve authored gravity,
  locomotion, and collision semantics rather than coupling them to render ticks.
- [ ] Gate scope on the snapshot contract and baseline measurements above.

Acceptance: compare deterministic fixed-step outcomes against the same workload
before threading. Exercise destruction completion, teleport/reparent/removal,
gravity changes, support/contact, and shutdown under repeated queries. Measure
end-to-end query/input latency as well as simulation and render CPU time.

## Delivery constraints

Only implement an explicitly selected slice; this document authorizes no source
change. Preserve existing UI/pointer priority rules and simple-shape fast paths.
Keep modules focused and allocation behavior measurable. A diagnostic snapshot
must show which scene/mesh/pose generation it captured, rather than appearing to
be a live tree while frozen. Detailed avatar collision and generalized physics
threading must not slip into the initial precise-raycasting work.
