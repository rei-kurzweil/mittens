# Player keeps drifting after jumping onto crates

## Status

Reported during manual gravity-demo testing on 2026-10-05. Explicit surface
friction is now implemented, with a regression for lateral motion after an
oblique contact projection. The exact intermittent crate-climbing interaction
still needs interactive reproduction/verification; do not mark the report fully
resolved based only on the synthetic contact case.

## Symptom

After jumping onto or bumping into crates while trying to climb them, the player
sometimes acquires persistent directional velocity. The player keeps being
pulled in that direction and the velocity does not settle. There appears to be
no friction or damping to remove the remaining motion.

The reporter suspects surface contact response. The exact contact sequence and
source of the residual velocity still need confirmation.

## Repro starting point

1. Run `cargo run --release -- load examples/secondary-motion-desktop.mms`.
2. Jump onto the crate piles, including their top edges and corners. Repeat
   approaches from different directions; the reported behavior is intermittent.
3. Release movement keys after contact and observe whether the player continues
   moving on the crate or floor.
4. Record the affected crate, contact direction, grounded state, and Velocity
   before contact, immediately afterward, and after returning to flat ground.

This scene is a proposed reproduction fixture; the precise failing interaction
has not yet been isolated. The manually adjusted teleport pit is unrelated to
the suspected contact path and should be preserved during investigation.

## Expected behavior

Jumping onto crates should not leave the player with unintended, persistent
lateral motion. Grounded motion should settle according to an explicit player
friction/damping policy while intentional locomotion remains responsive.

Do not fix this by clearing every Velocity on every contact: the existing slide
response intentionally preserves tangential movement, and some demos explicitly
drive horizontal Velocity.

## Current code evidence and hypothesis

`StaticContactSystem::remove_inward_velocity` normalizes the contact correction
direction, projects the directly owning Velocity into world space, and removes
only its inward component. It retains tangential velocity and marks support
contacts grounded when the normal's Y component exceeds 0.5. This was the original frictionless behavior. Static surfaces can now author a
friction coefficient for a bounded tangential contact impulse.

A possible source is an oblique capsule/box edge or corner normal: projecting
initially vertical falling velocity onto that contact tangent can introduce
horizontal velocity. Flat-ground contacts subsequently preserve that horizontal
component. This is a hypothesis, not a confirmed diagnosis. Also check movement
target ownership and whether Input or another driver continues adding motion.

`Velocity.reset()` is an explicit stop/respawn operation. It should not become an
automatic workaround that hides incorrect contact response or an unspecified
friction policy.

## Investigation and regression coverage

- Capture world/local velocity and contact normal before and after each
  correction, including multiple contacts within one fixed substep.
- Separate initial overlap correction from swept landing and check that pose
  correction does not create persistent unintended motion.
- Verify capsule movement target and its directly owning Velocity; distinguish
  physical velocity from movement retained or applied by Input.
- Decide player grounded friction/damping behavior and how it interacts with
  deliberate horizontal Velocity, slopes, edges, and airborne motion.
- Add a deterministic crate-edge/corner jump reproduction, followed by flat
  support, that checks unintended drift settles without suppressing intentional
  movement. Keep existing tangent-preservation and thin-floor sweep tests.

## Relevant code

- [Desktop demo](../../examples/secondary-motion-desktop.mms)
- [Static contacts](../../src/engine/ecs/system/static_contact_system.rs)
- [Contact geometry](../../src/engine/ecs/system/collision_geometry.rs)
- [Velocity integration](../../src/engine/ecs/system/velocity_system.rs)
- [Desktop input](../../src/engine/ecs/system/input_system.rs)

## Initial friction change

See [surface-friction-for-static-collidables.md](../task/surface-friction-for-static-collidables.md).
The desktop secondary-motion floor and crates now author `friction(0.8)`.
No global player damping or automatic Velocity reset has been introduced. Zero
friction continues to preserve tangent speed; friction acts only during contact.
