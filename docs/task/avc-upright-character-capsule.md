# AVC auto-calibrated upright capsule

Date: 2026-07-19

Status: implemented.

## Contract

Every `AvatarControl` creates one runtime-only upright capsule by default.
`collision_disabled()` opts out, while `capsule_radius(radius)` overrides the
default 0.28 m controller radius. The radius is capped at half the measured
height for small avatars.

AVC samples the spawned renderable subtree once in `model_root` coordinates:

- `height = max_y - min_y`
- `center_y = (min_y + max_y) / 2`
- `effective_radius = min(authored_radius, height / 2)`
- `half_segment = height / 2 - effective_radius`

X and Z bounds are intentionally ignored, so sleeves, hair, and a wide T-pose
do not inflate the controller. If render bounds are delayed, AVC retries on
later ticks. Once a GLTF reports that it spawned but remains unmeasurable, AVC
falls back to the existing authored/head-bone height with floor Y at the
model-root origin.

The generated transform stream passes `model_root` translation, drops rotation
and scale, then applies `center_y` in world-up space. It owns
`Collision.kinematic`, `CollisionShape.capsule_y(...)`, and
`CollisionResponse.slide()`; the subtree is excluded from serialization.

Collision correction is computed at the capsule pose and applied as a
world-space displacement to the locomotion root. Desktop routes to AVC's
parent `driven_t`. XR shares the `InputXRGamepad` resolver and routes to the
transform ancestor above the owning `InputXR`, retrying while unavailable.
Generic authored colliders can use `movement_target(...)`; unresolved targets
skip movement and velocity changes for that frame. With no authored or runtime
target, response retains immediate-parent behavior.

## Completed checklist

- [x] Add normalized `CapsuleY` Rust, component, MMS, AABB, and serialization APIs.
- [x] Centralize inclusive cube/sphere/capsule intersections and shape-aware MTVs.
- [x] Add stable floor/ceiling, radial corner, tangent, and coincident fallbacks.
- [x] Hard-rename the legacy response API to `CollisionResponse*` without aliases.
- [x] Add authored and runtime movement targets with world-displacement routing.
- [x] Add reusable bounds-based upright-capsule inference.
- [x] Generate exactly one runtime capsule per enabled AVC and retry delayed data.
- [x] Route desktop and XR AVC correction to their locomotion transforms.
- [x] Remove AVC's orphan head splice and store the real `head_mount`.
- [x] Convert `secondary-motion-desktop` to one head-level input/driver.
- [x] Remove redundant active and commented AVC camera spheres.
- [x] Rename the collision-response specification and repository references.
- [x] Split broader movable-body semantics into a separate audit task.

## Non-goals

`CollisionMode::Kinematic` is intentionally unchanged. Movable-vs-movable
authority, continuous collision detection, manifolds, stairs/slopes, mass,
and additional inference heuristics belong to the collision-system audit.
