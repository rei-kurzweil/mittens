# Collidable BVH and static contact

StaticContactSystem owns a persistent BVH over enabled static Collidable children
of enabled Zones. It runs on the simulation thread. The renderable/raycast BVH
remains separate; both reuse bounds traversal/refitting utilities. Standalone
zone-pair queries and ZoneObservationSystem keep their existing semantics.

## Maintenance

World retains typed collidable IDs as components are inserted and removed,
including offline assembly and runtime-generated AVC capsules. Successful mutable
access to a Transform, Zone, or Collidable marks it changed. Raw record access
also invalidates reference/topology resolution conservatively. This does not
require a per-pass scan of unrelated scene components.

Before each contact pass, the index consumes changes, resolves affected bindings,
and refreshes static geometry. Dependencies include the owning Zone and its
resolved effective frame, including Zone.at references. Transform propagation
marks the effective frame when ancestors or operators move it. Structural or
selector-metadata revisions re-resolve registered collidables, including disabled
or unresolved bindings that may become ready later.

Membership changes rebuild the BVH in a batch. Pose/shape changes refit affected
ancestor bounds without rotating nodes. Unchanged entries retain their geometry.
The bounds are conservative; exact contact rejects unsupported frames as before.
Main-thread callers must propagate changed poses before querying contact.

## Queries and response

Internal AABB queries return possible neighboring surfaces in stable collidable
ID order using reusable traversal and result buffers. Each mover queries its
previous/proposed swept bounds, then runs the existing downward capsule/box floor
sweep and exact separation math on candidates. Corrections refresh candidates so
new neighbors cannot be missed. The existing six-pass limit, friction, Velocity
grounding, movement-target routing, mounted exclusions, and thin-floor landing
behavior remain. The final non-convergence check also queries the BVH.

The index caches world frames and primitive contact geometry. Sweep and
separation math are shared with synchronous zone geometry; it does not infer
physical contact from a world AABB alone. Ordinary contact includes floors,
walls, and ceilings. Native crouch support now shares this index through
`StaticContactSystem::support_index` after pose propagation. It queries a padded
capsule/sole AABB, then checks centers below the capsule center, exact yawed-box
footprints and top heights near the capsule bottom for each foot. Own-avatar
zones and effective frames are excluded, including redirected `Zone.at` frames.
No floor flag, duplicate BVH or per-avatar Y index is required. Per-foot support
does not alter contact response or publish a new grounded state.

## Legacy migration

CollisionComponent, CollisionShapeComponent, CollisionMode, CollisionSystem,
CollisionStarted/CollisionEnded, their intents and MMS constructors, COL/COLS,
and the old collision visualization are removed. These authoring APIs are no
longer accepted. Use Zone.cube/sphere/capsule_y with Collidable.static() for
surfaces or Collidable.slide() for pose-driven movers. Existing Zone observation
events and Zone visualization remain available. CollisionShape is still the
shared Rust geometry value, and AVC's mesh-based capsule inference is retained.

Zone dimensions are local. A unit cube scaled by its parent uses
Zone.cube([0.5, 0.5, 0.5]); do not repeat the parent's scale in half extents.
The shared studio floor retains its existing contact zone and drops its redundant
legacy collider.

## Profiling and verification

Set CAT_PROFILE_SPATIAL=1 to emit contact summaries every 120 passes. Summaries
include indexed static/mover counts, average maintenance/query/exact/total time,
node visits, candidates, exact tests, and cumulative rebuild/refit counts.
Per-pass counters include the final non-convergence query. Multiple contact
passes may occur in one presentation frame, including fixed physics substeps.

The ignored profile_original_scan_against_retained_bvh unit test compares the
retained index with the actual pre-change scan algorithm on the same 4,097-static,
one-mover fixture. It prints initial and steady-state times. Run with:

```sh
CAT_PROFILE_SPATIAL=1 cargo test --lib profile_original_scan_against_retained_bvh -- --ignored --nocapture
```

Differential tests compare neighborhoods with a brute-force bounds oracle and
contact outcomes with the original scan; lifecycle, moved referenced frames,
large floors, correction-discovered neighbors, multiple movers, and distant
geometry are covered. Timing results are workload/build-specific, not a promise
for full XR scenes. Threading remains a profiling-driven follow-up and must
preserve current-step contact before dependent cameras and avatar solving.


### Controlled measurement (2026-10-21)

On the debug test fixture above, averaged over 120 warmed passes:

| Path | First pass | Steady contact pass |
| --- | ---: | ---: |
| Original full scan | 35.685 ms | 35.7710 ms |
| Retained BVH | 60.109 ms | 0.0618 ms |

The indexed first pass includes initial BVH construction. This fixture has
4,097 static collidables and one capsule; it is not a full-scene or XR timing.
Distant geometry leaves exact-test counts unchanged in the dense regression.
Initial builds remain synchronous, and the existing bvh 0.7 backend rebuilds
when membership changes. Refit-only motion does not rebuild the tree.
