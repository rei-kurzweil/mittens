# Bone-oriented bounds for skinned mesh raycasting

Status: proposed, 2026-11-02. No runtime implementation is implied. This refines
the [hierarchical mesh BVH plan](../task/hierarchical-bvh-mesh-intersection.md)
with an alternative to refreshing an entire posed primitive on its first ray.
The [testable implementation slices](../task/shared-mesh-queries-and-posed-bvh.md)
separate shared static queries, posed avatar caches, and later physics threading.

## Goal

Use a small set of bone-oriented boxes to reject most of an avatar before
evaluating its detailed geometry. Only candidate triangle regions need current
posed vertices and exact intersection tests. Initially this is a raycasting
design, not a change to physical collision shapes.

## Precomputed triangle membership

At import, retain each triangle's primitive-local index and its vertices' joint
indices and weights. The current importer retains four influence slots per
vertex; these are four weighted joints, not four kinds of vertex. Include every
nonzero influence retained by the rendering path, not just the largest weight.
If rendering later supports additional influence sets, extend this mapping too.

For each triangle, take the union of influencing joints across all three
vertices. Add the triangle once to each corresponding bone box's candidate
list. With four slots per vertex, a triangle can reference up to twelve distinct
joints. Lists overlap deliberately: an elbow triangle can be discoverable from
both upper-arm and forearm boxes. Preserve primitive/material ownership.

Precompute an influence score for optional ordering, such as the average weight
of that bone across the triangle's three vertices, treating missing weights as
zero. Strongly influenced triangles may be tested first. This score is a search
hint, not a reason to omit lower-weight membership or accept a farther hit.
Triangle membership stays fixed while topology and skin weights stay fixed;
ordinary joint motion only changes transforms, bounds, and posed geometry.

## Bounds and hierarchy

Each bone box follows a bone-local coordinate frame. Rigidly attached geometry
can retain fixed local bounds while its frame moves. Blended geometry requires
bounds that enclose the current positions of every triangle in its candidate
list, including all three vertices and their other joint influences.

Assigning a triangle to every influencing bone does not by itself guarantee
coverage. A blended vertex can lie between separated bone boxes, outside both.
Do not assume that rotating bind-pose boxes or choosing generous-looking fixed
padding proves conservative coverage.

A possible conservative construction precomputes source-position envelopes for
each candidate group and each influencing joint. At query time, transform those
envelopes by the current skin palette into the owning box's frame and bound
their union. For nonnegative, normalized linear skinning weights, that enclosing
box also contains their weighted blends. Include morph displacement envelopes
and match renderer weight/fallback rules. Validate this construction against
posed vertices before relying on it; it may produce loose boxes.

These conservative boxes can be updated using joint transforms without skinning
every detailed vertex. Their limits may change as well as their orientation.
Ancestor bounds must enclose moving descendant boxes. The skeleton's anatomical
parent links alone do not form a valid spatial containment hierarchy. Start with
a small flat box set if sufficient; add a BVH over it when measurements justify
the extra maintenance. Frames with scale/shear need consistent affine-space ray
tests or a conservative fallback, rather than silently treating them as rigid.

The outer scene bound must conservatively contain all candidate geometry even
before any detailed query. Stale tight bounds must not hide a moved limb.

## Ray query and lazy geometry

1. Query the scene bounds, then the current bone-oriented boxes.
2. Prefer candidate boxes by ray entry distance. Influence scores may order
   triangles within a candidate list; distance bounds take priority for pruning.
3. Skin/morph candidate vertices on demand using the current committed pose.
   Cache posed vertices by instance and deformation revision; evaluate shared
   vertices once and test duplicate triangles once per query.
4. Run exact ray/triangle tests and keep the nearest eligible hit. A box hit is
   only a candidate. Continue after triangle misses. Stop only when remaining
   candidates are provably unable to beat the best hit, not at the first hit in
   an influence-prioritized list.

Joint/ancestor motion, IK, tracking, springs, morph changes, and binding/asset
replacement invalidate affected geometry. A candidate group's dependencies
include every influencing joint, not just the bone naming its box. Whole-avatar
rigid motion can reuse local posed geometry. Repeated rays at an unchanged pose
reuse cached vertices; unqueried groups do no detailed deformation work.

Begin with precomputed lists and exact scans of candidate triangles. Optional
triangle BVHs within large groups can be added later with lazy refits; they are
not required for the first version. Cache scope, timing, eviction, and exact vs
approximate results follow the linked hierarchical BVH proposal.

## Acceptance and measurement

Compare nearest hits against a brute-force current-pose triangle reference.
Cover blended elbows/shoulders/hips, vertices influenced outside their primary
bone, triangles spanning bones, extreme joint poses, morphs, duplicate candidate
lists, nonuniform transforms, and independent instances of one avatar asset.
Demonstrate that conservative bounds never reject a reference hit.

Measure box maintenance, candidate triangles, unique deformed vertices, cache
reuse, memory, and query latency against whole-primitive lazy refits. Test both
occasional queries and continuous pointers over animated avatars. Influence
ordering is optional and should remain only if it improves measured queries.
