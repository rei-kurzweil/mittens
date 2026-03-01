# Raycasting circles and cones (design notes)

This document sketches how *we could* add raycasting support for circle/ring and cone-shaped handles.

Primary motivation: gizmo picking.
- Rotation handles are effectively **rings** (circles in a plane, usually an annulus with thickness).
- Translation handles often include a **cone** tip plus a shaft.

## Current state (as of 2026-02)

The current CPU raycaster is broad-phase only:
- It tests rays against **world-space AABBs** derived from a renderable’s `base_mesh`.
- **Important limitation:** AABB construction is only implemented for a small set of builtin meshes
  (`Cube`, `Quad2D`, `Triangle2D`). For any other `base_mesh`, AABB generation returns `None`.
  - In `BvhSystem`, this means the renderable gets a “placeholder AABB” far away, so it’s
    effectively never a hit candidate.
  - In `RayCastSystem`’s non-BVH fallback, `None` means “skip this renderable”, so it’s also
    effectively unraycastable.
- The BVH (`BvhSystem`) is AABB-based and returns the nearest AABB hit among eligible shapes.

So, today, “raycasting a circle” really means one of:
- If the thing is ultimately backed by `Cube`/`Quad2D`/`Triangle2D`: “raycast the AABB of that primitive”.
- If it’s backed by any other mesh: it currently won’t be hit at all (until we add better bounds / narrow-phase).

## What we want

We want a narrow-phase hit test that answers:
- Does this ray intersect a *ring volume* (for rotation)?
- Does this ray intersect a *finite cone* (for arrow tips), or a good proxy for it?

And we want to keep performance predictable:
- Many rays per second (cursor hover / drag), but few candidates after BVH.
- Avoid per-triangle work unless we have to.

## Recommended rollout order (practical, testable)

Before we do any fancy analytic ring/cone math, we should make sure gizmo parts (and other non-boxy meshes) are even *eligible* for picking.

1. Broad-phase first: add “best-effort AABB” for more meshes.
  - Today, only `Cube` / `Quad2D` / `Triangle2D` produce real AABBs.
  - First change should be: for non-rectangular / non-rectangular-prismatic shapes (tetrahedra, cones, rings, etc) and arbitrary imported meshes, provide a reasonable AABB so they become BVH candidates.
  - This can be done by either:
    - defining local-space bounds for known primitives, or
    - computing local-space bounds from mesh vertex positions at import/registration time.

2. Narrow-phase second: special-case math once a candidate AABB is hit.
  - After BVH says “this AABB was hit”, dispatch to a shape-specific intersection test.
  - This can start as a stub: accept the AABB hit as the final hit (lets us validate BVH coverage + UX quickly).
  - Then tighten it up by implementing analytic ring/cone/capsule tests.

## High-level approach

Use the same “broad-phase then narrow-phase” architecture:

1. Broad-phase
   - BVH traversal yields a small candidate set.
   - Candidates are filtered by a cheap test (AABB slab test).

2. Narrow-phase
   - Use **analytic** intersection tests for common gizmo shapes (ring + cone).
   - Optionally fall back to per-triangle tests for arbitrary meshes.

This keeps gizmo picking robust without forcing all renderables into triangle-level ray tests.

## Why gizmos can make the underlying object “unclickable”

This is the failure mode you’re observing: a tetrahedron (or cube) is clickable until you attach a gizmo, and then the gizmo seems to “steal” all clicks.

At a high level, this happens because the current raycaster is:
- **AABB-only** (broad-phase is treated as the final answer for hit distance), and
- chooses the **nearest hit by ray parameter** $t$ (smallest positive intersection distance).

So when you attach a gizmo you add *more* raycastable renderables (rings, stems, cone tips). Even if those gizmo parts are visually thin, their AABBs:
- are **axis-aligned** in world space, and
- often become **much larger than the actual geometry**, especially when rotated.

### Yes: axis-aligned boxes get “too big” when rotated

An axis-aligned bounding box (AABB) is aligned to world X/Y/Z axes. If you rotate a thin object (a rod/stem) or a flat object (a ring/quad), the smallest axis-aligned box that contains it can grow a lot.

Common intuition:
- A thin shaft rotated 45° in the XY plane tends to need a wider AABB in both X and Y.
- A flat ring that tilts toward the camera can require a thicker AABB in Z.

This AABB inflation creates **false positives**: the ray intersects the AABB even when it does not intersect the actual gizmo geometry.

### “Biggest AABB” does not win; **closest $t$** wins

Picking isn’t “which box is biggest”; it’s “which candidate intersects first along the ray”. A thin gizmo stem that sits in front of the tetrahedron can have an AABB hit at a smaller $t$ and therefore win.

## Narrow-phase implication: you need “next-candidate” logic

Your expected logic is correct *conceptually*:

1. Broad-phase: gather candidates whose AABBs are hit.
2. Narrow-phase: for each candidate, test the real pick shape (ring annulus, cone, capsule, etc).
3. Choose the closest candidate whose narrow-phase test succeeds.

But there’s an important implementation detail:

If the BVH query returns **only the single best AABB hit**, then narrow-phase rejection can’t automatically “fall through” to the next object behind it unless the query keeps searching.

So for narrow-phase to fix the gizmo-stealing problem, the raycast query needs one of these patterns:

- **Return multiple AABB-hit candidates** (usually in increasing $t$ order), then narrow-phase them until one passes.
- Or, **incrementally continue BVH traversal** after a narrow-phase reject to find the next AABB hit.

Without that, you can still get:
- closest AABB = gizmo ring (false positive)
- narrow-phase says “not actually on the annulus”
- result = “no hit” (even though the tetra behind *should* be clickable)

So: *narrow-phase alone is not enough* unless we also change the query to consider more than one candidate.

## Option A: per-triangle raycast (baseline, worst-case)

Yes: the straightforward “generic” solution is:
- Use BVH AABB to find candidates.
- For each candidate, intersect the ray with each triangle in the mesh (Möller–Trumbore).

Tradeoffs:
- Pros: works for any mesh.
- Cons: worst-case $O(\text{triangles})$ per candidate; gizmo rings/cones are often *not* super low-poly; requires CPU mesh access + consistent transform.

If we ever do this, we likely also want a per-mesh acceleration structure:
- Build a small BVH (or kD-tree) per CPU mesh once, then transform the ray into mesh-local space.
- This makes narrow-phase roughly $O(\log n)$ instead of $O(n)$.

## Option B: analytic pick shapes (recommended for gizmos)

Instead of raycasting the *render mesh*, attach a “pick shape” that approximates it tightly:
- Ring → annulus with thickness (a “slabbed annulus” volume)
- Arrow tip → finite cone
- Arrow shaft → capsule or cylinder

This is common in editors: the visual mesh can be fancy, but picking uses simple math.

### Suggested data model

Add a component that carries the intended pick geometry:

- `RaycastableShapeComponent`
  - `shape`: enum
    - `Ring { radius, tube_radius, thickness }`
    - `Cone { radius, height }` (finite)
    - `Capsule { radius, half_height }` or `SegmentCapsule { a, b, radius }`
    - `Sphere { radius }`
    - `Aabb { min, max }` (fallback)

  ### Unifying “automatic” vs “explicit” pick shapes

  One clean way to keep raycasting logic on a single code path is:

  - `RaycastableComponent` answers only: “is this eligible for picking?”
  - `RaycastableShapeComponent` answers: “what geometry do we actually test?”

  Then we define a *shape resolution* rule:

  1. If an explicit `RaycastableShapeComponent` is present (child or nearest ancestor, same kind of lookup we use for `RaycastableComponent`), use it.
  2. Else, derive a default pick shape from `Renderable.base_mesh`.
    - Example mapping: `CUBE -> Aabb/Box`, `QUAD_2D -> QuadSlab`, `TRIANGLE_2D -> TriangleSlab`, `TETRAHEDRON -> Tetra`, `CONE -> Cone`, `RING -> Ring`.
  3. Else, fall back to “mesh bounds” (local-space AABB computed from CPU vertices) and treat it as `RaycastableShapeType::Aabb`.

  This unifies the runtime code path:

  - BVH broad-phase always indexes an AABB coming from the resolved shape.
  - Narrow-phase always dispatches from the resolved shape.

  #### Where should the resolution happen?

  You suggested doing it in `RaycastableComponent::init()` (“when it’s initialized, look at the sibling renderable and pick a shape type”). That’s a reasonable direction, but there are a few pitfalls to watch for:

  - **Init order / topology:** `RaycastableComponent` is often a child of a `RenderableComponent`, and components may be registered in any order. If the renderable isn’t present yet (or the parent relationship isn’t established yet), init-time derivation can’t reliably read `Renderable.base_mesh`.
  - **Changes over time:** if `Renderable.base_mesh` changes (asset swap, LOD, text UV baking behavior, etc), an init-time derived shape can become stale unless you also re-resolve on renderable changes.
  - **Ancestry-based opt-in:** today `RaycastableComponent` can live on an ancestor and affect many renderables. A “child init creates a shape” approach only covers the “direct child under renderable” topology.

  Because of those, a robust pattern is:

  - Treat shape resolution as an **on-demand** or **flush-time** step (e.g. when adding a renderable to BVH, compute “resolved shape” from explicit component if present, otherwise derive from base_mesh/mesh bounds).
  - Optionally materialize the resolved result into a cached component (e.g. `ResolvedRaycastableShapeComponent`) for debugging and to avoid recomputing every query.

  This still keeps the implementation unified, without depending on fragile init ordering.

  ### Gizmo topology recommendation (concrete)

  Gizmo handles often have multiple leaf renderables per logical handle (e.g. translate arrow = stem + cone tip). A practical topology is:

  - One `RaycastableComponent` node per clickable/gestureable handle subtree (so all descendant renderables are BVH-eligible).
  - One `RaycastableShapeComponent` per leaf renderable (explicit or inferred from `Renderable.base_mesh`) so stem/tip/ring can each have the right narrow-phase shape.

  This keeps eligibility coarse (one toggle per handle) while keeping shape tests precise (per visual part).

  ## Keeping track of each renderable’s pick shape (initial approach)

  For the first implementation, prefer **on-demand resolution**.

  Given a broad-phase hit renderable id, resolve its pick behavior like this:

  1. Look for an explicit `RaycastableShapeComponent` attached to that renderable (or found via the same child/ancestor convention we use elsewhere).
  2. Otherwise, infer a default shape from `Renderable.base_mesh` for that renderable.
  3. Otherwise, fall back to “AABB-only picking” (either a best-effort bounds AABB, or no narrow-phase if we truly have no bounds).

  This is easiest to reason about because there’s no cache invalidation story: the pick shape is always derived from the current world state at the time of the query.

  ## Later optimization options (if we need them)

  If profiling shows the on-demand lookups are too expensive, we can switch to caching without changing the external behavior:

  - **Cache on the renderable:** add a `ResolvedRaycastableShapeComponent` directly on the leaf renderable.
    - Easy to inspect/debug, but must be kept in sync when dependencies change.

  - **Cache in the BVH entry:** store the resolved shape alongside the BVH AABB record.
    - Fastest for the raycast loop, but requires that “pick shape changed” triggers a BVH update/refit.

Where it lives in topology:
- Usually as a child of the renderable (similar to `RaycastableComponent`).
- Or on the transform above the renderable (so one pick shape can represent a subtree).

### Broad-phase bounds for these shapes

Even with analytic narrow-phase, BVH still needs an AABB.

#### Circle/ring AABB (tight-ish)

Assume a ring centered at `c` with plane unit normal `n` and radius `R`.

The projection of the circle onto the world X/Y/Z axes has extent:

$$
\text{extent}_i = R \sqrt{1 - n_i^2}
$$

where $n_i$ is the component of the unit normal along axis $i$.

If the ring has tube radius `r` and/or thickness along its normal, expand the AABB by `r` (and half-thickness) in the appropriate directions.

This gives a much tighter AABB than “transform 4 corners of a quad” for a rotated ring.

#### Cone AABB

A practical approach:
- Define cone in local space with apex at `z=0` and base circle at `z=height`.
- Transform a small set of extreme points to world and take min/max:
  - apex
  - base circle cardinal points: (±radius, 0, height), (0, ±radius, height)

For higher accuracy under non-uniform scale, sampling more points is acceptable (still cheap vs triangles).

### Narrow-phase: ring as a “slabbed annulus”

Model a ring volume as:
- A plane with normal `n` through center `c`
- A thickness band around the plane (a slab): $|\langle (p-c), n \rangle| \le t$
- An annulus in the plane: $R_{in} \le \|p_\perp - c\| \le R_{out}$

Algorithm (robust for near-parallel rays):
1. Intersect the ray with the slab (two planes offset by ±t)
   - This yields a ray parameter interval $[t0, t1]$ (or no hit).
2. Choose a representative point in that interval (often the nearest positive `t`).
3. Project that point onto the ring plane and compute radial distance.
4. Hit if radial distance is within `[R_in, R_out]`.

Notes:
- This behaves better than pure ray-plane intersection when the ray is nearly parallel to the ring plane.
- For gizmos, it’s usually good to *inflate* `t` and the annulus thickness slightly to make picking feel easier.

### Narrow-phase: finite cone

Model a finite cone in local space.
A common parametrization:
- Cone axis is +Z
- Apex at `z=0`
- Base at `z=h` with radius `r`
- Slope $k = r / h$

Implicit surface (in local space):

$$
 x^2 + y^2 = (k z)^2
$$

Ray: $p(t) = o + t d$.
Substitute into the implicit equation → quadratic in `t`.
Then reject solutions where `z` is outside `[0, h]`.

Practical gizmo advice:
- Don’t rely on the cone alone; include a capsule/cylinder for the shaft.
- Prefer transforming the ray into shape-local space (cheap) rather than transforming the shape.

## BVH integration ideas

There are two clean ways to wire this into the existing systems.

### Approach 1: extend BVH to index “raycast shapes”

- Keep BVH entries as “(ComponentId, AABB)”.
- Let entries correspond to either:
  - renderables (current behavior), or
  - `RaycastableShapeComponent` nodes.

Then RayCastSystem’s narrow-phase dispatch is based on which component type was hit.

### Approach 2: keep BVH over renderables, but narrow-phase uses attached pick shape

- BVH hit returns renderable `cid`.
- RayCastSystem looks for a `RaycastableShapeComponent` child/ancestor and tests that.
- If no shape exists, fall back to current AABB-only behavior (or triangle tests).

This is less invasive but sometimes less precise if one renderable contains multiple pick regions.

## “Tighter-than-AABB” BVHs?

AABB BVHs are popular because they’re simple and fast.
If we need tighter fits for rotated rings/cones, we have options:

- Keep AABB BVH, but compute *better* AABBs (see circle extent formula above).
- Use multiple AABBs per logical object (e.g., ring split into 4 quadrants).
- Move to OBB BVH (more complex; likely not worth it initially).

For gizmos, “multiple AABBs per handle” is usually the sweet spot if broad-phase false positives are painful.

## Recommendation for first iteration

For gizmo picking specifically:
- Add “best-effort AABB” support so gizmo meshes become BVH candidates.
- Add an explicit `RaycastableShapeComponent` used only for gizmo parts.
- Stub narrow-phase at first (AABB hit == hit), then implement analytic ring + cone + capsule tests.
- Keep using AABB BVH as the broad-phase.

Triangle-level raycasts can remain a separate, later feature for “precise mesh picking”.

## Open questions

- Do we want picking to be world-constant thickness, or screen-constant thickness?
  - Editors often prefer screen-constant thickness (easier to select at any distance).
- How should priority work when ring and arrow overlap?
  - Likely: smallest `t` first, then a stable tie-breaker (e.g. axis order).
- Should we separate “raycast for interaction” from “raycast for gameplay/collision”?
  - Gizmo shapes are interaction-only.
