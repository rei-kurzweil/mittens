# Train box-sweep validation: square tunnel and circular aperture

## Purpose

Use the existing static train scene as a focused geometric validation for a future generic, velocity-bound sweep producer. Unlike the spherical projectile, the train sensor should use an explicitly rectangular box shape. The test should demonstrate both that the box can pass through an opening that fits it and that it is correctly rejected by a differently shaped opening that does not.

This is a test-design document, not a claim that generic sweeps or train motion are implemented. The current train fixture is static/inert. Keep this work separate from the existing projectile impact path and do not use `ProjectileHit` as a substitute for the generic event.

## Geometry scenario

Choose a fixed travel direction and keep the train's orientation constant through both obstacles. The train's leading sensor should be a box whose cross-section and dimensions are explicit and independently asserted by the test.

1. **Square tunnel — expected pass.** Make a straight tunnel through one building with a square opening larger than the train box in both transverse dimensions. Leave a documented clearance on every side. The train box should sweep through the tunnel without reporting a hit against the tunnel opening or interior surfaces.
2. **Approximately circular aperture — expected block.** In a separate building or test fixture, make a round opening whose diameter is larger than the box's smaller transverse dimension, but smaller than its larger transverse dimension. A sphere-like proxy based only on the smaller dimension could fit, while the actual rectangular cross-section cannot. The box sweep must report contact with the aperture boundary and must not report a clear traversal.

For example, for a box cross-section about 1.8 units wide by 1.0 unit high, use a square tunnel with clear dimensions greater than 1.8 by 1.0, and a circular opening around 1.35 units in diameter. Treat those as starting values only: verify the authored collision geometry, transforms, margins, and chosen sweep convention before locking them into tests. The circular hole must be represented by collision geometry that actually leaves a hole; do not model it as a solid box that fills the opening.

## What the test is intended to establish

- The sweep uses the authored box dimensions, not the projectile sphere sweep or a sphere substituted for the train.
- A box translated through free space and a sufficiently large square opening produces no false positive.
- The same box cannot pass through an aperture narrower than one of its transverse dimensions, even if a smaller spherical approximation would fit.
- A thin obstacle or aperture boundary crossed between step endpoints is detected by the continuous sweep, rather than only by endpoint overlap.
- The hit result is stable and meaningful: the expected obstacle is identified, the fraction is within the step interval, and the contact point/normal are finite and consistent with the boundary.
- No hit is emitted for the square-tunnel pass; the circular-aperture case reports a hit. Assert candidate/hit identity rather than relying only on train appearance.

## Suggested staged implementation and tests

### Stage 1: geometry-only unit fixtures

Before moving the train or wiring MMS, create deterministic static collision fixtures for the square tunnel and round aperture. Test the box sweep directly with known start/end poses. Include a control sphere sweep through the same circular aperture to show why the box case is a distinct shape test; choose the sphere radius from the intended sensor contract, not as an arbitrary workaround.

Test at minimum:

- square aperture pass with positive clearance;
- rectangular-box collision with the narrower circular aperture;
- a small clearance change around the box's limiting dimension;
- a fast crossing where neither endpoint overlaps the obstacle;
- an initial overlap and a clean miss;
- reported fraction in `[0, 1]`, finite contact point and normal, and normalized normal if required by the API.

Avoid exact tangency as the only pass/fail criterion. Add a separate tangent/boundary case if tangency semantics are part of the sweep contract.

### Stage 2: static-BVH integration

Run those same fixtures through the retained static BVH. Verify the swept bounds include the relevant tunnel/aperture candidates and that the narrow phase determines pass versus hit. Do not create a second independently draining contact index; respect the existing BVH/contact-change lifecycle.

### Stage 3: generic producer and listener gate

Only after the box math and BVH candidate behavior pass focused tests, connect the generic producer:

- the collidable explicitly references its velocity source;
- no applicable event listener means no event-only query work;
- a matching scoped listener enables only movers whose zone scope is under that listener scope;
- a matching global or `Any` global listener uses the all-eligible-movers path without per-mover ancestry checks;
- the train's square tunnel traversal produces no contact event;
- the train's circular-aperture attempt produces one contact event with the intended moving and target identities.

Use a counted/instrumented query in gate tests so skipped work is asserted deterministically. Keep global-handler overhead measurements separate from collision correctness tests.

### Stage 4: scene-level train demonstration

Make the train interactive/moving only after the generic event producer is proven independently. Then use the scene as a visual smoke test: pass through the square tunnel and stop at or report contact with the circular aperture. Do not make the train reverse or alter velocity as an implicit engine response unless a later, separately specified response contract requires it; generic sweep events should remain observational by default.

## Important boundaries

- Preserve existing projectile movement, clamping, lifecycle, and `ProjectileHit` behavior.
- Do not gate static contact solving or any collision result needed for physical response.
- Do not treat the current inert train scene as evidence that generic sweep math, listener gating in a producer, or event delivery works.
- Do not generalize to capsules, arbitrary convex shapes, triangle-mesh sweeps, moving targets, or collision masks in this same slice. Validate the box contract first, then add other shapes in separate tests.
- Keep the square tunnel and round opening visibly and geometrically distinct. Decorative geometry alone is not a valid collision test.

## Acceptance criteria

The box-sweep slice is ready to build on when deterministic tests prove that the rectangular train sensor passes through the adequately clear square tunnel, collides with the narrower circular aperture, catches a fast crossing, and reports valid contact data. The focused BVH and geometry tests should pass before the train scene is made interactive.
