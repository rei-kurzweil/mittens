# Swept collision events and projectile lifecycle

Date: 2026-11-11

Status: analysis only. This document describes a direction, not an approved
implementation. No engine or scripting changes are implied yet.

## "First consumer" naming

The last commit introduced `Acceleration`, `ProjectileImpact`, `ProjectileHit`,
and a native sphere sweep. The sweep itself is a general continuous-collision
mechanism, but the event and marker names describe the first thing that used
them: the STA rocket.

That is "first-consumer naming": the API contract is named after whichever
feature motivated the code, rather than after the reusable mechanism. It is fine
as a throwaway smoke test. It becomes misleading once vehicles, thrown objects,
or other physics-driven things need the same observation.

Two separate features got mixed together:

1. **Generic swept-contact observation.** Tell MMS when a physics/velocity-driven
   object hits something, without polling from script.
2. **Projectile lifecycle.** The conventional procedures a projectile performs
   on its first valid hit: stop, remove, spawn an effect, apply damage, notify
   the author, avoid duplicate hits.

The first should be general and minimal. The second is useful as a reusable
abstraction, ideally a `Projectile` component/composition that configures the
first for the common case.

## Current state

The current implementation (commit `70612c12`) does this:

- `Acceleration.local(vec3).max_speed(number)` is a general local acceleration
  provider. It walks up from a `Velocity`, stops at nested `Velocity`
  boundaries, and caps the velocity projection along its direction.
- `Zone.sphere(r) { ProjectileImpact {} }` opts a sphere zone into a native
  continuous sweep.
- Each fixed step, `VelocitySystem` asks `ProjectileMotion` to sweep the sphere
  from the current center to the proposed center and picks the earliest hit
  (`src/engine/ecs/system/projectile_motion.rs`).
- On a hit, the engine truncates movement to the hit fraction, zeroes linear
  velocity, disables the `VelocityComponent`, and emits one `ProjectileHit`
  scoped to the zone (`src/engine/ecs/system/velocity_system.rs`).
- MMS handles the visible lifecycle: explosion, damage, callback, and
  `remove_subtree` (`assets/components/projectiles/rocket.mms`).

The engine is therefore not only observing the hit. It is also deciding that
the moving object stops and becomes inert. MMS runs afterward. The rocket's
handler happens to agree with that policy, so the split looks cleaner than it
is.

## Problem 1: `ProjectileHit` is not projectile-specific

The detection path is:

- moving target owned by `Velocity`
- sphere zone under that target
- opt-in marker
- sweep against environment surfaces
- report earliest contact point and normal

Nothing in that list requires the moving object to be a projectile. It applies
to a fast-moving sensor, a thrown object, a vehicle probe, or a physics-driven
tool. The current names and the one-shot stop behavior encode the rocket's
policy, not the mechanism.

A general event should be named and shaped like the other zone observations:
`ZoneEntered` / `ZoneExited` are overlap observations for movers. A swept
observation belongs in the same vocabulary (for example, `ZoneSwept`), with a
marker meaning "this moving zone participates in swept collision" rather than
"this is a projectile."

## Problem 2: The engine owns the response

Event dispatch happens after the systems tick has already committed the
integrated pose (`Universe::update`, then `SystemWorld::process_commands`).
By the time an MMS `ProjectileHit` handler runs, the engine has already:

- clamped the movement
- zeroed the velocity
- disabled the velocity component

So MMS cannot decline a hit, ignore a particular target, redirect the projectile,
or decide that a collision with a car should behave differently from a collision
with a tree. It can only clean up after the engine has already made the
decision.

If the intended contract is "the engine reports, MMS handles," then one of these
has to change:

- the engine reports a contact before committing movement and waits for an
  authored decision, or
- the handler can return/queue a consume/ignore decision, or
- the event is a pure observation and the engine does not stop anything; the
  authored policy is responsible for stopping/removing the object.

This is a design decision, not a rename. It also affects responsiveness: if the
engine reports after integration and MMS reacts on the next update, the object
may visually pass through a thin wall for a frame.

## Problem 3: The collidable set is too narrow

`CollidableBvh` intentionally excludes `CollidableMode::Slide` collidables and
stores them separately as `movers`. The sweep only queries the static BVH, so it
sees:

- static box surfaces (including yaw)
- destroyed triangle meshes

It does not see slide colliders. Those include the player movement capsule, the
metal cans, and vehicle movement hulls. Static vehicle proxies and destroyed
vehicle meshes can be hit, but an object represented only by a slide collider
cannot.

The desired policy is the opposite of a projectile-only target set: vehicles and
objects in general should be hit, while the player body, accessories, and the
sakura tree should not. That needs:

- broad-phase coverage of movers as well as static surfaces
- continuous tests for capsule/sphere/box/mesh targets, as applicable
- relative motion for moving targets (the current sweep only advances the
  moving sphere against the surface's current frame)
- a policy for distinguishing the player rig/accessories/tree from everything
  else

The current self-filter only skips surfaces whose ancestry includes the moving
target. It does not express "never hit the player" and does not express "do hit
a car." The player exclusion currently works mostly by accident because slide
colliders are skipped entirely.

There is a tension here: if MMS owns the hit decision, the event must carry
enough identity for MMS to reject the player/tree and accept the car. But if MMS
rejects a contact, the projectile must continue, which means the engine cannot
have already stopped it. Target filtering and response ownership are therefore
the same problem.

## Proposed split

### A. Generic swept-contact observation

A physics-driven movable with an opted-in shape reports the earliest contact with
the collidable world each fixed step. No MMS polling, no per-frame `FrameTick`
subscription.

Minimum properties:

- opt-in marker on a zone or motion component
- continuous sweep against retained broad-phase candidates
- fixed-step, allocation-conscious, no per-draw work
- event payload identifies both sides: moving component, its zone, the hit
  collidable, the hit zone/frame, point, normal, and fraction
- reports contacts, does not decide lifecycle
- works for any `Velocity`-driven object, not just projectiles

Illustrative MMS shape (not final syntax):

```mms
let sensor = Zone.sphere(0.055) { SweepCollidable {} }
on(sensor, "ZoneSwept", fn(hit) {
    // hit.collidable, hit.other_zone, hit.point, hit.normal, hit.fraction
})
```

This layer should stay usable by authors who want their own response: a car
probe, a thrown can, a physics tool, or a custom projectile that does not want
the standard lifecycle.

### B. Projectile lifecycle

"Projectile lifecycle" names the standard set of procedures a projectile runs
when it hits something: stop motion, remove the projectile, spawn an impact
effect, apply damage, notify the author, and suppress duplicate hits. It may also
include lifetime expiry and cleanup of the velocity/acceleration subtree.

That lifecycle is useful enough to package, but it should not be the definition
of the generic event. The intended direction is a `Projectile`
component/composition that wraps the lower-level pieces, roughly:

```
Projectile
  └── Acceleration
        └── Velocity
```

The user should not have to create the zone, attach a sweep marker, subscribe to
a sweep event, and remember to stop/remove on impact for the common case. They
should configure the projectile and optionally override the impact policy.

Illustrative shape (not final syntax):

```mms
let rocket = Projectile
    .acceleration([0.0, 0.0, -12.0])
    .max_speed(18.0)
    .radius(0.055)
    .lifetime(6.0)
    .on_hit(fn(hit) {
        Damage.at(hit.point[0], hit.point[1], hit.point[2], 1.5, 8.0)
    }) {
    T { /* model */ }
}
```

Whether `Projectile` is:

- an MMS-authored composite (an asset function that builds the standard
  components and handlers), or
- a native component that contributes the standard lifecycle while invoking
  authored callbacks,

is an open question. The MMS-authored composite is closest to the current
ownership rule: the lifecycle procedures are written in MMS, and the engine only
provides the generic sweep observation. A native component is easier to make
efficient and harder to override, and risks re-encoding rocket policy in Rust.

Either way, `Projectile` should be a policy layer, not the home of the sweep
mechanism. `Acceleration` remains a general motion provider, and the generic
swept event remains usable without `Projectile`.

## Relationship to `ZoneEntered` / `ZoneExited`

`ZoneEntered` and `ZoneExited` observe overlap for enabled slide collidables.
They carry `collidable` and `movement_target`. The swept observation is a
different query: it asks whether a moving shape crosses something during a
step, including high-speed cases where an overlap test at either endpoint would
miss.

They should share vocabulary and payload conventions, but should not be merged.
Overlap events are persistent enter/exit state. A swept event is a contact
instant with a fraction along the motion. They may also need different debounce
and repeat semantics.

## Open questions

1. Event and marker names: `ZoneSwept` / `SweepCollidable`, `ZoneHit` /
   `ContinuousCollision`, or something else?
2. Respected ownership: pre-integration query, post-integration notification, or
   a consumable event. This determines whether MMS can really decide.
3. Target coverage: which shapes and which collidable modes participate, and
   whether moving targets use relative-motion sweeps.
4. Filtering location: authored predicate in MMS, engine roles/tags, or both. If
   MMS filters, what does "ignore" mean for motion?
5. Repeat semantics: one event per contact, one per step while touching, or
   enter/exit-style state for non-projectile movers?
6. `Projectile` packaging: MMS composite versus native component, and how much
   default lifecycle is hardcoded versus authored.
7. Owner exclusion: should the sweep always skip the source user's rig, or is
   that entirely a policy decision?
8. Damage and effects: should `Projectile` know about `Damage.at` and effect
   assets, or only call an authored callback?

## Recommendation summary

- Treat the generic sweep as an observation primitive for any velocity-driven
  collidable, not as projectile behavior.
- Rename/split before adding more consumers; do not let `ProjectileHit` become
  the public name for the general event.
- Let MMS own projectile lifecycle, and ensure the event contract actually gives
  MMS a decision rather than notifying it after the engine has stopped the
  object.
- Package the conventional hit policy as `Projectile` on top of
  `Acceleration`/`Velocity`, so authors do not have to wire the zone event by
  hand.
- Extend target coverage to vehicles and general objects, with explicit
  player/accessory/tree exclusion, before treating the API as stable.
- Keep performance constraints: fixed-step integration, retained broad phase,
  and no per-step allocations or script polling in the hot path.
