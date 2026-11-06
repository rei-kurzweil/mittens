# Task: Geometry-aware runtime destruction

Date: 2026-11-05
Status: started. Geometry-only rigid-mesh import and worker cuts (slice 4a) implemented;
live Destroyable dispatch, implicit craters and appearance preservation remain pending.

## Outcome

Keep one `Destroyable` opt-in and damage request model. Choose a supported damage
representation from the source geometry: existing box solids, implicit-field
subtraction, or validated imported solids. Damage must update visuals, ray queries
and collision together. Preserve batching where possible after damage.

The ship's orchard hills and rigid pickup body are the first integration targets.
Posed/skinned destruction is a later project, not a prerequisite for either.
This tracker plans engine extensions; it does not authorize new renderer features
or a separate public MMS destruction API for every geometry type.

Responsibility boundary: `DestroyableSystem` owns damage thresholds, instance state,
source selection and publication. It should request implicit-field regeneration for
implicit sources or delegate solid subtraction to `CsgSystem`. CSG accepts resolved
mesh geometry and cutters; it does not load GLBs or decide gameplay damage rules.

Related trackers:

- [Current box runtime implementation](sakura-runtime-destruction.md)
- [Original damage and fragment proposal](damageable-surfaces-csg-and-fragments.md)
- [Implicit surface roadmap](implicit-3d-surfaces-and-csg.md)
- [General mesh CSG roadmap](mesh-csg-operations.md) (historical backend proposal;
  the runtime destruction path currently uses `manifold-rust` through `csg_mesh`).

## Current state

- [x] `DestroyableComponent` supplies settings; `DamageComponent` supplies impact volumes.
- [x] `DestroyableSystem` accumulates local damage and manages derived geometry.
- [x] `DestroyableGroups` unpacks affected aggregate batches after a successful cut.
- [x] `CsgSystem` performs bounded worker-side solid subtraction and fragment generation.
- [x] Box cuts update visible geometry and precise triangle collision/raycast data.
- [x] Surviving chunks are batched into one render mesh per damaged source surface.
- [x] Unpacking resolves imported mesh overrides rather than publishing GLTF placeholders;
  regression: `unpacking_damage_group_preserves_imported_mesh_overrides`.
- [ ] Destroyable automatically cuts imported GLB geometry. Runtime opt-in still only
  cuts cube sources with cube zones; the CSG mesh-input path is now tested independently.
- [ ] Implicit terrain accepts subtractive damage fields.
- [ ] An unpacked aggregate is recombined. Intact sources currently remain separate draws.
- [ ] General cuts preserve imported UVs/material assignments. Current CSG output uses
  generated flat normals and planar UVs.

## Slice 1: Source selection and ownership

- [ ] Establish an internal source classification shared by discovery and damage handling:
  supported box, implicit field, supported imported solid, or unsupported geometry.
- [ ] Retain existing `Destroyable` inheritance and explicit child overrides.
- [ ] Keep damage state and derived geometry per instance; never mutate a shared GLB asset.
- [ ] Establish one owner for each published surface. An implicit rebake or CombineMesh
  rebuild must not resurrect intact geometry over a damaged replacement.
- [ ] Report unsupported geometry with an actionable reason; never cut its bounding box
  as a silent substitute for its actual shape.

Acceptance: existing cube fixtures behave identically; two instances sharing a mesh can
be classified independently; unsupported sources remain visible and unchanged.
Preserve direct/batched hot paths rather than adding per-draw dynamic dispatch.

## Slice 2: Precise static imported-mesh queries

Depends on slice 1's source ownership decisions. This enables accurate shot targeting;
blast-volume terrain damage in slice 3 need not wait for it.

- [ ] Resolve imported CPU geometry and retain a triangle acceleration structure per
  immutable source mesh, with instance transforms and primitive identity kept separate.
- [ ] Transform rays into mesh-local space for rigid translation/rotation. Rotating a
  truck must not require rebuilding its local triangle hierarchy.
- [ ] Preserve world hit distances under supported nonuniform scales and reject singular
  transforms explicitly. Keep broad-phase bounds current when instances move.
- [ ] Return enough provenance to identify the hit primitive and triangle; retain a
  conservative bounds test before precise triangle tests.

Acceptance: rays through empty cab/window space miss; rays on rotated body panels hit
the correct surface and primitive. Shared-mesh instances remain independent. Record
query cost and confirm idle rigid motion does not rebuild triangle topology.

## Slice 3: Local implicit-terrain damage

Depends on slice 1; can proceed independently of imported-solid cutting.

- [ ] Retain subtractive blast volumes in terrain-local space, applying them to the
  same field that generates the visible hills. Keep their runtime lifetime explicit.
- [ ] Apply the existing local damage threshold policy before adding a subtraction.
- [ ] Dirty only intersecting chunks, including neighboring samples required for seams.
- [ ] Rebuild affected mesh and collision off the frame-critical path with bounded work.
- [ ] Publish matching visual/query/collision revisions only after successful preparation;
  failed jobs retain the last valid surface.
- [ ] Define support at the deck: a crater in a hill must not accidentally delete the
  independent station floor. Runtime placement/support queries must see damaged ground,
  rather than relying on the original `park_ground_height` formula.

Acceptance: repeated impacts carve a local crater, including across a chunk seam;
walking/driving and shot queries follow its surface; untouched chunks remain retained.
Record affected chunks, triangles, worker time and upload cost. No terrain rebakes at idle.
Debris from implicit cuts is a separately evaluated follow-up, not required for the crater.

## Slice 4: Import validated closed meshes into the solid backend

Depends on slice 1. Use small untextured fixtures before integrating the truck.

- [x] Add CPU-mesh-to-solid conversion at the `csg_mesh` boundary.
- [x] Validate finite coordinates, topology, winding, closed volume and degeneracy;
  distinguish render seams/duplicated vertices from genuinely open geometry.
- [ ] Partition actual source solids with stable local bounds, rather than assuming
  `[-0.5, 0.5]` box geometry. Keep local damage addressing stable after cuts.
- [x] Establish handling for disconnected closed shells and unsupported open panels.
- [ ] Reuse worker queues, revision checks and transactional publication. Bound the
  initial partitioning cost as well as subsequent cuts.

Acceptance: cut an irregular closed fixture at rest and after rigid rotation; holes
match its actual shape, collision and rays. Failed/open-mesh conversion leaves the
original intact with a diagnostic. Repeat cuts and shared instances do not corrupt assets.

### Slice 4a implementation and evidence (2026-11-05)

Following the user's priority clarification, start at the CSG geometry boundary rather
than implementing Destroyable dispatch first. `Solid::from_mesh` accepts rigid CPU
geometry, canonicalizes exact coincident positions at render seams, checks boundary
edges/winding/degeneracy and imports through the strict solid kernel. It does not
approximately weld gaps or turn panels into bounding boxes. Closed self-intersecting
folds are normalized to their positive material region on the CSG worker.
Open/non-manifold/inverted inputs, skinning and morph data are rejected explicitly.
Closed disconnected shells are supported. Source imports are limited to 16,384 triangles
and 49,152 vertices; larger sources must be partitioned before this entry point.

`CsgSystem::submit_mesh(MeshCutJob)` accepts an immutable shared mesh and a cutter in
the same local coordinate frame. Import, validation, subtraction, fragment generation
and replacement CPU-mesh construction run on the existing bounded CSG worker. Results
retain owner/revision metadata and a `Solid` for subsequent cell jobs. Failed jobs
return an error without publishing visuals or collision. Existing cell-job callers
retain their API and behavior.

Validation commands:

```sh
cargo test --lib csg_ -- --nocapture
cargo test --lib destroyable
cargo test --test csg_pickup_glb -- --nocapture
```

To audit a temporary Blender export without replacing the checked-in ship model:

```sh
MITTENS_CSG_AUDIT_GLB=/tmp/mitten_pickup_truck-v7-audit.glb cargo test --test csg_pickup_glb -- --nocapture
```

The 2026-11-05 `.7.blend` inspection confirmed closed, positive-volume roof and
wheel-well shells, parented to `truck`; each wheel is parented to its spring.
The temporary GLB's roof and wheel wells pass actual worker cuts. In that export,
11 primitives pass and 9 are rejected: the doors, hood, headlights and main body
have open boundaries, and the four tire primitives have inward winding. The
front-wheel hub primitives pass separately. The main body has three disconnected
pieces, including one isolated triangle. This newer file has not replaced the
checked-in model; its new `hood`, `roof` and `wheel wells` object names need to
be considered when updating the prefab's static batch list.

The subsequent `.8.blend` export replaced the checked-in GLB. Its doors and
four tire primitives now pass actual CSG cuts: 17 of 20 primitives pass. The
three rejected sources are the hood (14 inconsistent winding edges), headlights
(16 open boundary edges) and main body (191 open boundary edges). Parenting is
retained. The prefab now batches the renamed hood plus roof and `wheel_wells`
(the runtime-sanitized name), for 10 sources / 2,074 triangles; the suspension
and hydraulic regression passes against this export.

A disposable in-memory hood trial (recalculate consistent normals, then flip
the resulting inward shell) fixes its source topology. However, cutting that
trial produces a degenerate triangle in a generated debris mesh when checked
through the strict re-import validator. Track generated-output numerical cleanup
in the CSG path separately from the saved hood's source-winding failure; do not
claim the repaired hood is fully validated or require extra thickness solely
to hide that output issue. The user's blend was not modified by the trial.

The `.9.blend` / subsequent checked-in GLB inspection reports 20 of 23 primitives
passing actual cuts. Headlights are now closed and pass. Remaining rejections:
`hood` still has 14 inconsistent winding edges; the separated `truck_bed` has
three open boundary edges; lower-cabin `truck` has 130 open boundary edges.
Seats are now named `car_seat` and `car_seat.passenger_seat`. The prefab batches
these names and the separate bed, for 11 sources / 2,286 triangles. This remains
a geometry audit, not automatic live imported-mesh destruction.

The `.9` compatibility run passes the updated body-batch checks, then fails the
existing suspension fixture's authored hub-height expectation: FL is 0.3407216 m
versus the previous 0.34669146 m at rest. The export changes root/spring/hub
coordinates. Do not report full suspension validation as passing; refresh the
model calibration and check hydraulic wheel support against the final asset.

Fixtures cover an irregular L-shaped volume (including a cutter inside its empty
bounding-box region), rigid rotation/nonuniform scale, disconnected shells, repeated
cuts, shared-source independence, invalid topology and worker recovery after rejection.
The pickup audit reads actual GLB primitive data and submits it to the worker. Current
asset: 7 primitives accepted/cut, 5 rejected for open boundary edges and 4 for
non-positive signed volume/winding. Small 58-triangle parts took about 4 ms per
import/cut; 2,316-triangle springs took about 51–58 ms on this development machine.
These are worker timings including conversion, fragments and render-mesh generation,
not XR frame times or performance guarantees. Avoid importing the full asset repeatedly
on impacts; retain converted solids and add bounded partitioning in the next slice.

Not complete: automatic `Destroyable` delegation, chunking arbitrary large sources,
GLB material/UV preservation, implicit terrain subtraction, live publication and
aggregate rebatching. The ship's truck body is not automatically destructible yet.

## Slice 5: Appearance-preserving imported cuts

Depends on slice 4; required before shipping textured GLB destruction.

- [ ] Preserve source material/primitive identity, UV seams and appropriate normals
  on surviving exterior surfaces. Define attribute transfer for new edge vertices.
- [ ] Give newly exposed cut faces an explicit interior material/UV policy.
- [ ] Preserve the source material path without adding legacy shader fallbacks.

Acceptance: a multi-material textured fixture retains its exterior appearance after
repeated cuts. Cut interiors are distinguishable and deterministic; unaffected material
regions are not flattened into the first material in a combined batch.

## Slice 6: Retain or restore aggregate batching after damage

Build first against existing box destruction; reuse for imported solids later.

- [ ] Track source provenance and geometry revisions through CombineMesh output.
- [ ] Select one owner for rebuilding damaged aggregate batches. Evaluate reusing
  CombineMesh versus a shared batching utility; avoid competing duplicate outputs.
- [ ] Include intact sources and damaged replacements, respecting material compatibility,
  transforms, destroyed/empty parts and independently moving wheel/spring branches.
- [ ] Rebuild affected batches on committed damage revisions, not every frame.
- [ ] Keep the previous batch visible until replacement upload succeeds; update query
  ownership separately so combined drawing does not discard per-part hit identity.

Acceptance: repeated cuts and failed uploads produce no disappearing body, duplicate
geometry or resurrected parts. Measure draw count, allocations, mesh uploads and frame
time before/after damage against the existing unpacked behavior. Untouched groups stay cached.

## Slice 7: Ship pickup integration

Depends on slices 2, 4 and 5; use slice 6 to retain useful batching.

- [ ] Audit `mitten_pickup.glb` per primitive for closed solids versus open/thin panels.
- [ ] Opt supported body parts into real mesh cutting. Decide explicitly whether thin
  parts need authored thickness or authored breakable sections; do not fabricate box bodies.
- [ ] Keep wheels/springs independently movable and preserve mounting, driving, gravity
  and hydraulic transitions after body damage.
- [ ] Replace affected collision proxies where required so a visible hole has a defined
  collision outcome, rather than leaving an invisible intact body collider across it.

Acceptance: dashboard damage retains intact body geometry; a supported body-panel cut
removes actual material; a second truck sharing the GLB stays intact. Validate mounting,
driving, transitions and query/collision behavior on the damaged instance.

## Later: Skinned models

- [ ] Investigate bone-attached conservative hit regions for approximate body-part queries.
- [ ] Account for blended bone influences; one vertex group is not an exact rigid part.
- [ ] Evaluate posed triangle testing/refitting independently of oriented BVH nodes.
- [ ] Define cut-space ownership, new-vertex skin weights, morph targets and ongoing
  animation before enabling actual destructive cuts on skinned geometry.

Rigid-model destruction does not depend on this investigation. Rotated BVH nodes alone
would not solve deformation or skin-weight propagation.

## Suggested work order and validation

Start with slice 1, then terrain damage (3) and rigid-mesh queries (2). Build imported
solid conversion (4), appearance preservation (5), and the truck integration (7).
Aggregate batching (6) can start using box fixtures while those paths develop.

For each slice, record focused correctness tests and measured performance in this
tracker. Use small deterministic fixtures before the full ship scene. Keep native
modules focused and within repository size guidelines; avoid per-draw allocations,
resource recreation or unbounded main-thread remeshing. Do not enable an editor or
add a rendering feature as part of these slices.

## Current pickup checkpoint (2026-11-06)

The repaired `.10.blend` export is now checked in. Its 22 closed primitives pass
the real mesh-cut audit, including closed output and volume conservation; the
open lower cabin remains excluded. Initial solid import normalizes intersecting
closed folds on the worker. Float output stitches collapsed collinear faces and
exports standalone fragments with outward winding. Authored positions are unchanged.

All three ship pickups opt their supported parts into native imported-mesh
destruction. Panel colliders follow replacement triangle geometry, precise hit
queries include the excluded cabin, and the first cut unpacks only the affected
body batch. The immutable imported geometry is shared between truck instances.
Material scope is retained; exterior UV preservation and aggregate rebatching
remain future work.

Focused tests cover cutting a door and wheel, moving damaged replacements,
keeping the second truck and excluded cabin intact, and suspension/hydraulic
transitions with the updated spring origins. Temporary layout/editor controls
are removed and the hydraulics panel is raised 20 cm.
