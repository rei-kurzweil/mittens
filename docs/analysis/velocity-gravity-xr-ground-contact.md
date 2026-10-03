# Velocity, gravity, and XR avatar ground contact

Status: implementation review and proposed flow, 2026-10-03. The linear
`Velocity` pose driver exists. Gravity acting on that velocity, floor contact
for its target, and a grounded XR avatar do not yet exist as one path.

This note connects the [Velocity/physics plan](../task/velocity-forces-and-pluggable-physics.md),
[XR grounding slices](../task/xr-avatar-grounding-first-slices.md), and
[skinned-model grounding plan](../task/skinned-mesh-grounding-and-floor-contact.md).
Those documents hold the broader requirements and implementation steps.

## What runs today

- `Velocity { T { ... } }` stores parent-local linear speed in m/s and moves its
  immediate child transform at a bounded fixed timestep. One-shot
  `translate`/`translate_world` calls change speed. The first XR button scene is
  `examples/mittens-corp-linear-velocity.mms`; it deliberately has no gravity
  or physical floor contact. Angular velocity and general forces are pending.
- `GravityComponent` currently supplies a coefficient to an ancestor lookup in
  `CollisionResponseSystem`. That system stores its **own** runtime velocity on
  each `CollisionResponseComponent`. It does not update `VelocityComponent`.
- `CollisionSystem` detects overlaps. `CollisionResponse.slide()` corrects
  existing static overlaps for opted-in colliders; AVC creates such a capsule
  for its avatar. Collision geometry or an overlap event alone does not hold a
  body up. The old slide path is not a reliable free-fall driver: with no
  overlapping static collider it returns before integrating gravity.
- The frame schedule runs collision detection and old response **before**
  `VelocitySystem`, then OpenXR and gamepad locomotion. Thus a new Velocity
  displacement and later gamepad displacement do not receive same-frame floor
  contact through that earlier collision pass. The focused grounding task
  requires a scheduling change, including camera/eye publication after the
  final corrected ancestor pose.
- The `capsule-stick-figure.mms` scene has an identity outer Bisket grounding
  root. The ordinary `mittens-corp.mms` Bisket still has its locomotion root
  directly around `InputXR`. The linear-Velocity XR scene has the extra
  `Velocity`/grounding wrapper for a motion test, not a grounded character.

These are separate working pieces, not yet a falling avatar implementation.
The older [XR pose-grounding analysis](xr-avatar-pose-grounding.md) addresses
tracking origin, avatar feet, and HMD calibration. Its 2026-03 topology is
historical; it should not be read as the current physics schedule.

## Which transform falls

The physics target is an **outer placement/grounding transform**, not the
skinned mesh vertices, bones, tracked HMD transform, or controller transforms:

```text
world / stage
  Velocity                                  owns linear state
    T (grounding root)                      gravity + contact move Y here
      T (locomotion root)                   gamepad moves XZ / yaw here
        InputXR -> tracked head/controllers
          AVC -> GLTF root -> joints        body pose, IK, animation
```

Each descendant inherits the grounding displacement once. The skinning system
then computes joint matrices from the resulting world transforms. A separate
upright capsule or authored character proxy represents the avatar for floor
contact. The deformed render mesh is not an exact contact shape. Its proxy
must account for effective model scale and foot offset; its bottom, not the
grounding root origin or HMD Y, defines the resting height.

This composition needs a tracking-origin policy. In XR, physical crouching
changes HMD height relative to the rig; it is not a downward gravity request.
Calibrate the avatar/body-to-tracking-origin relationship (and account for
recenter events) so falling the outer root moves the camera and body together
without treating head motion as a fall. `LOCAL` versus `STAGE` reference space
also changes how the scene establishes a physical floor reference. The
*virtual* floor used for contact still comes from explicit scene geometry.

## Proposed step from airborne to resting

For the first slice, use a fixed step and one static floor, with a declared
movement target and proxy. A world-space gravity acceleration is sufficient;
mass is unnecessary for uniform `g`.

1. Sample tracked input and resolve gamepad locomotion for the step. Establish
   the proposed ancestor pose, and update the proxy from that pose.
2. Add gravity to the first-class velocity state: `v += g * dt`. Apply a jump
   or other one-shot impulse once at a step boundary. A continuous thrust or
   drag provider contributes each step under an explicit lifetime policy.
3. Integrate the owned grounding root: `x_proposed = x + v * dt`. Convert
   between world acceleration/contact normals and Velocity's parent-local
   stored vector deliberately, especially if an ancestor rotates or scales.
4. Query the proxy's proposed motion against static collidables. For a first
   flat floor, a bounded step plus a floor crossing/sweep test can prevent
   passing completely through it; a final-pose overlap test alone cannot
   guarantee that. Pick the earliest valid support contact and correct the
   root so the proxy bottom is at or above the floor.
5. If moving into a contact normal `n`, remove the inward velocity component:
   `v = v - min(dot(v,n), 0) * n`. This is the velocity-level effect of a
   unilateral contact constraint. It does not add bounce. Keep tangent
   velocity unless a separate friction or locomotion policy changes it.
6. Store the corrected pose and velocity before the next substep. Publish a
   grounded/contact observation with normal and support identity. Propagate
   final transforms, then publish XR cameras, rays, avatar pose/IK, skinning,
   and render state from that same corrected ancestor pose.

The normal force here is **not** a permanent upward `mg` force component. At
rest, the solver enforces no penetration and cancels gravity's inward velocity
for each step. A resting contact can be cached or put to sleep later for
efficiency, but `grounded` is an observed support state, not a substitute for
the collision constraint. If the floor disappears or the proxy walks off its
edge, there is no support; gravity makes it fall on a subsequent step. A small
contact tolerance and stable support selection avoid visible jitter. A ceiling
uses the same inward-normal rule with the opposite normal.

## Where friction fits

Static friction means a grounded **dynamic** body resists tangential force up
to a limit proportional to its support impulse; kinetic friction acts while
sliding. Neither is needed to keep an avatar from falling through a floor.
The current XR gamepad writes horizontal locomotion pose directly, so physical
static friction cannot usefully arbitrate that movement without changing its
authority. For the first grounded XR character, project desired horizontal
motion along blocking surfaces and preserve intentional XZ control. If later
locomotion becomes force-driven, contact can use a material friction coefficient
and a tangential impulse limit. Generic damping is drag; it is not static
friction, and the legacy `friction_y` field should not define the new contact
model. Wall sliding, slopes, stairs, step offsets, and moving platforms need
an explicit character-controller policy after the floor slice.

## Ownership and migration decisions

- Use one velocity state: gravity, jump, and other velocity drivers feed
  `VelocityComponent`; contact updates that same state. Do not pair it with
  `CollisionResponse`'s private velocity on the same movement root.
- Give one system authority to commit each grounding transform/channel per
  substep. The contact phase can correct the integrator's proposal; it must
  not race another pose writer. Keep gamepad XZ on its distinct inner root.
- Migrate or explicitly retire today's `Gravity`/`CollisionResponse` API as
  described in [static non-penetration migration](../task/retire-collision-response-to-static-nonpenetration.md).
  The generated AVC capsule's current `slide()` must be disabled or retargeted
  when the new ground contact owns Bisket, to avoid double correction.
- Use the zone shape/query foundation for authored spatial tests. It currently
  classifies points only; shape overlap and floor crossing/sweep queries must
  be added before it can replace the old collision worker for contact. Bare
  zones remain query-only; a collidable role opts a floor into contact.
- Update scheduling as a unit: current collision snapshots precede Velocity
  and gamepad motion, while XR eye publication precedes gamepad motion. The
  new contact must see the proposed final pose, and cameras must see the
  corrected pose in the same presentation frame. The fixed-step integrator
  currently batches all elapsed steps into one displacement; force/contact
  integration needs a contact check and velocity update **per substep**.
- Start with one named static floor and measured proxy bottom. Log root Y,
  proxy bottom, floor top, vertical speed, support normal/ID, and correction.
  Verify falls from more than one height, stable rest, horizontal walking,
  walking off the edge, crouch/recenter, and absence of a second pose writer.

General rigid-body forces need mass/inertia and a body authority policy;
moving-body contact and true friction need a broader solver or backend. They
are later slices of the existing [physics architecture task](../task/velocity-forces-and-pluggable-physics.md),
not prerequisites for one gravity-driven avatar on a static floor.
