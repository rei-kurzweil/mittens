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
- opt-in marker (the current projectile path; the generic path would bind a
  velocity instead)
- sweep against environment surfaces
- report earliest contact point and normal

Nothing in that list requires the moving object to be a projectile. It applies
to a fast-moving sensor, a thrown object, a vehicle probe, or a physics-driven
tool. The current names and the one-shot stop behavior encode the rocket's
policy, not the mechanism.

A general event should be named and shaped like the other zone observations:
`ZoneEntered` / `ZoneExited` are overlap observations for movers. A swept
observation belongs in the same vocabulary (for example, `ZoneSwept`), opt-in
through a bound velocity rather than a marker meaning "this is a projectile."

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

## Problem 4: Velocity-aware events and handler context

The desired event is not merely "a zone overlaps something." It is "a mover
controlled by the physics/velocity systems crossed static geometry during its
step." Detection is therefore inherently velocity-aware: the engine needs the
mover's motion to perform the sweep. This coupling to `Velocity` and its
providers is probably necessary, but it should stop at detection.

That coupling is different from a projectile lifecycle. The engine may need to
know the velocity to decide *whether* a contact happened and *where* it happened.
It should not therefore own stopping, removal, damage, effects, or handler
teardown. Those are policy.

There is also an API-shape question around subscriptions. In the scripting shell
an event handler is not a closure with lexical capture. A useful handler may
need a context payload containing data or component references: the moving
target, its velocity component, the sensor zone, the source/owner, and filter
data. That context would let the handler decide whether a contact is relevant and
what to do about it without looking unrelated state up by name.

A better shape for the opt-in is to bind a `Velocity` component to the
`Collidable` child by reference in MMS, rather than to introduce a projectile-like
marker or to make the `Zone` itself a mover. The zone remains a spatial region;
the collidable carries the collision behavior and the motion it should be swept
with. When the engine visits that collidable, it can:

1. check whether a velocity component is bound;
2. check whether any sweep-event listener is interested in that zone's scope;
3. only then run the sweeping zone query for that step and report a
   penetration.

This keeps the velocity coupling where it belongs (the detection query) and
removes the need for an author to explain "this is a projectile" just to get a
swept event. Binding on `Collidable` is also the lower-bookkeeping choice:
collidables are already the unit indexed and iterated by the collision systems,
and `CollidableComponent` already stores component references. It avoids adding a
"moving zone" concept to a component that is otherwise useful as an abstract
region for explicit queries and future spatial modeling.

The event can still be scoped to the zone even when the velocity binding lives on
its collidable. That keeps the handler vocabulary consistent with the spatial
region while letting the collidable own the motion.

The train example is intended to exercise this distinction with a non-projectile
mover. See
[the train example task](../task/sweeping-zone-query-events-train-example.md).

## Proposed split

### A. Generic swept-contact observation

A `Zone` whose `Collidable` child has a `Velocity` component bound to it reports
the earliest contact with the collidable world each fixed step. The velocity
reference is the opt-in: it tells the engine which motion to use for the sweeping
zone query. No separate projectile/sweep marker is required.

The engine only performs the query when the collidable has a bound velocity *and*
at least one relevant listener exists for the swept event. Binding alone is not
enough. This avoids paying for sweeps on every collidable, and it matches the
fact that sweep events are contact-only: with no listener, there is nothing to
notify and no reason to query.

Minimum properties:

- velocity-component reference bound to the `Collidable` by an MMS builder
- event scoped to the parent `Zone` even though motion is owned by the collidable
- listener-gated: check for an interested handler in the zone/ancestor scope
  before doing work
- continuous sweep against retained broad-phase candidates
- fixed-step, allocation-conscious, no per-draw work
- first geometry uses the existing sphere-based sweeping zone query; exact
  mapping from zone shape to sweep shape is open
- event payload identifies both sides: moving component, its zone, the bound
  velocity, the hit collidable, the hit zone/frame, point, normal, and fraction
- reports contacts, does not decide lifecycle
- works for any `Velocity`-driven collidable, not just projectiles

Illustrative MMS shape (not final syntax; the binding lives on the
`Collidable`):

```mms
let motion = Velocity { body }
let sensor = Zone.sphere(0.4) {
    Collidable.velocity(motion) {}
}
on(sensor, "ZoneSwept", fn(hit) {
    // hit.velocity, hit.collidable, hit.other_zone,
    // hit.point, hit.normal, hit.fraction
})
```

This layer should stay usable by authors who want their own response: a train, a
car probe, a thrown can, a physics tool, or a custom projectile that does not
want the standard lifecycle.

### B. Projectile lifecycle

"Projectile lifecycle" names the standard set of procedures a projectile runs
when it hits something: stop motion, remove the projectile, spawn an impact
effect, apply damage, notify the author, and suppress duplicate hits. It also
covers the non-impact endings: lifetime expiry, running out of fuel/energy, and
cleanup of the velocity/acceleration subtree.

The sweep observation is contact-only: no event is emitted for a step that misses
everything. Consequently the projectile owns the handler binding, and its
termination path must unbind that handler. Removing the projectile subtree while
leaving a sweep handler registered is not a valid cleanup; whether the engine
tears the binding down implicitly or the author calls `remove()` explicitly is an
implementation detail, but the observable requirement is that no stale sweep
events are delivered after expiry/fuel exhaustion.

That lifecycle is useful enough to package, but it should not be the definition
of the generic event. The intended direction is a `Projectile`
component/composition that wraps the lower-level pieces, roughly:

```
Projectile
  └── Acceleration
        └── Velocity
```

The user should not have to create the zone, bind the velocity, subscribe to a
sweep event, and remember to stop/remove on impact for the common case. They
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

## Transition order and deprecation gate

The generic mechanism should be added before the projectile API is renamed or
its current behavior is deprecated.

1. **Add the generic observation path alongside the projectile path.** Introduce
   a separate velocity-backed swept-zone observation/event: bind a `Velocity` to
   a `Collidable`, gate the query on listener presence, and report contacts.
   Do not rename `ProjectileHit`/`ProjectileImpact`, and do not change the
   current rocket behavior yet.
2. **Prove response ownership on the generic path.** Show that an MMS handler can
   accept or decline a generic contact without the engine having already stopped
   the mover.
3. **Build the `Projectile` lifecycle on the generic path.** Migrate the rocket
   to the new mechanism while the old projectile-specific path still exists.
4. **Keep both paths until parity is demonstrated.** The existing rocket tests,
   expiry, one-shot behavior, effect, damage, and callback must pass through the
   new path before anything is removed.
5. **Deprecate the projectile-specific event and behavior.** Only after the
   generic mechanism and `Projectile` abstraction are proven should the old
   names and engine-owned stop behavior be deprecated. Rename or remove them
   after a compatibility window.

This ordering avoids replacing a working smoke-test path with an unproven
general API, and it gives the generic mechanism a chance to be exercised by
non-projectile movers before projectiles depend on it.

## Decisions to date (working assumptions)

These are the points that now seem settled enough to design slices around.
They are still working decisions, not final API commitments.

- **Generic observation is separate from projectile policy.** The reusable
  mechanism is a swept-contact observation for any velocity/physics-driven
  collidable, not a projectile-specific event.
- **The opt-in is a velocity reference on a `Collidable`.** Bind a `Velocity`
  component to the `Collidable` child by reference in MMS. The engine uses that
  motion for the sweeping zone query; a separate projectile/sweep marker is not
  needed for the generic path. The parent `Zone` remains a spatial region and
  does not become a "moving zone."
- **Sweep events stay zone-scoped.** The velocity binding lives on the collidable,
  but the event is scoped to the parent zone so handlers continue to subscribe to
  the spatial region, with the collidable and bound velocity in the payload.
- **The sweep is listener-gated.** The engine checks whether any relevant
  listener exists for the swept event before performing the query. Binding a
  velocity without a listener should not cost a sweep and should not emit
  anything.
- **"Projectile lifecycle" is the name for the conventional hit procedures:**
  stop motion, remove the projectile, spawn the impact effect, apply damage,
  call the author callback, and suppress duplicate hits. Lifetime expiry and
  motion cleanup belong to the same lifecycle.
- **MMS owns projectile disposal and hit handling.** The engine should report
  contacts; it should not be the thing that decides a projectile is finished.
- **The common case should be packaged as a `Projectile` abstraction.** Users
  should not have to create a sweep zone, bind the velocity, subscribe to the
  sweep event, and remember the stop/remove sequence by hand. The intended
  composition is `Projectile` over `Acceleration` over `Velocity`.
- **Collision targets are broad.** Vehicles and objects in general should be
  hit. The player body, player accessories, and the sakura tree should not be.
- **No script polling.** Swept detection stays event-driven and fixed-step.
- **Sweep events are contact-only.** A miss is not observable and produces no
  event. There is no "still moving" or "no hit this step" notification.
- **Expiry/fuel exhaustion tears down the sweep binding.** When a projectile
  expires or runs out of fuel/energy, it must unbind its swept-zone handler
  along with stopping motion or removing its subtree. A stale handler that
  keeps receiving sweep events after the projectile is done is a bug.
- **Acceleration remains general.** It is not projectile-specific and should
  keep working as an independent provider.
- **Keep the performance shape.** Retained broad phase, fixed-step integration,
  no per-step allocations or script callbacks in the detection hot path.
- **Add the generic path before renaming or deprecating the projectile path.**
  The generic swept-zone event/mechanism should exist and be usable on its own
  before `ProjectileHit`/`ProjectileImpact` are renamed.
- **Do not deprecate projectile behavior until the replacement is proven.**
  Keep the current projectile event and engine-owned stop behavior until the
  generic mechanism and the `Projectile` lifecycle pass the existing rocket
  behavior through the new path.
- **The current implementation is a smoke-test slice, not the final API.**
  `ProjectileHit`/`ProjectileImpact` and the engine-owned stop behavior are
  expected to change, but only after the additive generic path is in place.

## Test-sized slices

The order below keeps each slice independently demonstrable. Earlier slices
should not require the later ones to be useful.

### Slice 1: add the generic swept-zone mechanism alongside the projectile path

Introduce a separate generic event for swept-zone queries, opted into by binding
a `Velocity` component to a `Collidable` by reference in MMS. Reuse the existing
sphere sweep and `CollidableBvh` geometry internally if useful, but keep this as
a distinct path.

The engine should:

1. find `Collidable` components with a bound velocity;
2. check whether any relevant listener exists for the swept event;
3. only then run the sweeping zone query for the step;
4. emit a contact event if the swept zone penetrates something.

Binding a velocity without a listener must not produce an event. A miss must not
produce an event either; there is no miss/heartbeat notification.

The generic path should report contacts without zeroing velocity or disabling a
component. Include identity in the payload: moving collidable/zone, bound
velocity, hit collidable, hit zone/frame, point, normal, and fraction.

Explicitly out of scope for this slice:

- renaming or removing `ProjectileHit` / `ProjectileImpact`
- changing the current rocket's engine-owned stop behavior
- changing what the existing projectile event means
- deciding the final builder method name on `Collidable` (the placement is
  settled: the binding lives on the collidable, not the zone)
- deciding whether a velocity-bound collidable keeps its normal target role

Smoke tests:

- An MMS scene with a zone/collidable pair, a bound velocity, a listener, and a
  thin static wall emits the new generic event with a sensible point/normal.
- The same scene with no listener emits nothing and, if instrumented, does not
  run the sweep query.
- The same scene with a listener but no bound velocity emits nothing.
- A listener on an ancestor scope is sufficient; removing the listener stops
  future events.
- The same scene emits no event at all on a miss.
- A scene with the new path does not emit `ProjectileHit`.
- The existing rocket scene still emits `ProjectileHit` and behaves exactly as
  before.
- A scene containing one generic swept collidable and one projectile sensor shows
  no cross-talk between the two events.

This is the easiest meaningful slice because it is additive: the existing
projectile path remains the compatibility baseline while the generic mechanism
is proven.

### Slice 2: make the generic path MMS-authoritative

Decide and implement how a generic observed contact is accepted or ignored by
MMS. The engine should not decide that a generic mover is finished. Depending on
the contract, this is a consumable event, an explicit response, or a
pre-integration query.

Smoke tests:

- A handler removes the mover; the root is gone and no stale collision persists.
- A handler ignores/declines the contact; the mover continues and can still hit
  a later target.
- A valid target behind an excluded target is still reachable when the first
  contact is declined.
- Ordering is documented: either the generic event arrives before movement is
  committed, or it is explicitly a post-step notification.

This slice settles the question that the current rocket path avoids: whether MMS
owns the response or only the cleanup.

### Slice 3: `Projectile` lifecycle on the generic path

Build the reusable projectile layer on top of slices 1 and 2: acceleration,
velocity bound to the swept `Collidable`, and a default hit handler that
stops/removes, spawns an effect, applies damage, and calls the author callback.
Migrate the rocket to the new generic path while leaving the old projectile
behavior available.

Smoke tests:

- Spawn a projectile with the default lifecycle; step until impact; assert the
  effect exists, the root is removed, and the author callback ran once.
- Fire two projectiles; assert two independent lifecycles and no double callback
  for one hit.
- Spawn one with no target; assert lifetime expiry removes it and the impact
  callback did not run.
- After expiry, keep stepping the scene and assert the sweep handler is unbound:
  no further sweep event or impact callback is delivered, and neither a stale
  listener nor a stale velocity-to-collidable binding remains scoped to the
  removed projectile.
- If fuel/energy exhaustion is implemented separately from lifetime, repeat the
  teardown test for that path: motion stops, the sweep handler is unbound, and
  later contacts do not resurrect the projectile.
- Override the hit callback and assert the default damage/effect policy can be
  replaced or augmented.
- Consumer MMS contains no explicit `on(sweep zone, "…")` wiring.
- Existing rocket behavior is reproduced through the generic path before the old
  path is touched.

The packaging decision (MMS composite versus native component) is still open,
but either version is testable with the same assertions.

### Slice 4: compatibility and deprecation gate

Keep the old projectile-specific event and engine-owned stop behavior until the
new path has parity with it. Then mark the old path deprecated, document the
replacement, and only later rename or remove it.

Smoke tests:

- Existing projectile scenes continue to load and behave during the
  compatibility window.
- New scenes use the generic event directly, or use `Projectile` without wiring
  the zone event by hand.
- The old and new rocket paths produce equivalent lifecycle outcomes in the same
  focused test matrix.
- Deprecation diagnostics point at the replacement API without breaking existing
  scenes.

Do not rename the projectile events or remove the engine-owned projectile
behavior before this gate passes. The generic event must be usable on its own
before projectile naming and semantics are changed.

### Slice 5: slide-sphere targets

Add `CollidableMode::Slide` sphere targets to the swept query. Start with
sphere-versus-sphere, which covers the cans and the existing vehicle movement
hulls without needing capsule or box narrow-phase work.

Smoke tests:

- A projectile aimed at a slide-sphere can emits a hit and identifies the
  collidable.
- A nearest-hit test with a slide sphere and a static wall picks the earlier
  fraction.
- The moving projectile's own subtree is still excluded.
- A miss through empty space still emits nothing.

This is a small geometry addition with straightforward unit tests, and it proves
that the target set is no longer static-only.

### Slice 6: inclusion/exclusion policy

Make "hit vehicles and objects, not the player rig/accessories/tree" concrete.
Depending on the ownership decision, this is either an authored MMS predicate or
an engine role/tag filter; the event payload from slice 1 should make either
possible.

Smoke tests:

- A projectile aimed at a car/object collidable emits.
- A projectile aimed at the player capsule, an accessory, and the tree does not
  emit (or emits a rejected/filtered event that leaves motion untouched,
  depending on the chosen contract).
- The same projectile continues through an excluded target and can still hit a
  valid target behind it.
- A handler that declines a hit does not stop the projectile.

This slice forces the "MMS can actually decline" decision. It is easy to test
once that contract exists, but it should not be merged before slice 2 settles
who owns the response.

### Slice 7: moving-target relative sweep

Account for a moving vehicle/object crossing the projectile during the same
fixed step, rather than sweeping only against the target's current pose.

Smoke tests:

- Two moving spheres that would miss under endpoint-only testing produce a
  contact with the expected fraction.
- A projectile and a vehicle crossing in front of it hit at the crossing point.
- A projectile and a target receding from each other do not produce a false hit.

This is the hardest geometry slice; keep it separate from the static/mover
coverage work.

### Slice 8: remaining target shapes

Add capsule, box, and mesh handling for slide targets as needed. The existing
player/vehicle shapes determine priority. Unit tests can reuse the same
nearest-hit, initial-overlap, and miss cases already used for sphere sweeps.

## Open questions

1. Event name: `ZoneSwept`, `ZoneHit`, `ContinuousCollision`, or something else?
2. Velocity-binding API: builder name and signature on `Collidable`; whether a
   velocity-bound collidable keeps its normal static/slide target role; what
   happens if the velocity is removed, disabled, or reparented.
3. Sweep shape: the first version uses the existing sphere sweeping query. How
   does a zone's authored shape map to the sweep shape, and when do capsule/box
   sweeps follow?
4. Listener gating: does the engine check scoped handlers on the zone, ancestors,
   or global handlers? Is the check per step, cached with invalidation, or
   captured at bind time? What happens when a listener is added or removed after
   the binding?
5. Respected ownership: pre-integration query, post-integration notification, or
   a consumable event. This determines whether MMS can really decide.
6. Target coverage: which shapes and which collidable modes participate, and
   whether moving targets use relative-motion sweeps.
7. Filtering location: authored predicate in MMS, engine roles/tags, or both. If
   MMS filters, what does "ignore" mean for motion?
8. Repeat semantics: one event per contact, one per step while touching, or
   enter/exit-style state for non-projectile movers?
9. `Projectile` packaging: MMS composite versus native component, and how much
   default lifecycle is hardcoded versus authored.
10. Owner exclusion: should the sweep always skip the source user's rig, or is
    that entirely a policy decision?
11. Damage and effects: should `Projectile` know about `Damage.at` and effect
    assets, or only call an authored callback?
12. Fuel/energy model: is exhaustion just lifetime/animation, or a real budget
    consumed by acceleration? Who triggers teardown, and is handler unbinding
    automatic with subtree removal or explicitly authored?
13. Handler lifetime: should any projectile-term path (hit, expiry, fuel,
    cancellation, removal) share one teardown routine that unbinds the sweep
    handler and suppresses later events?
14. Subscription context: which component references or data should a sweep
    handler receive, and are they captured at bind time or resolved at emit
    time? The scripting shell has no closure capture for this.

## Recommendation summary

- Treat the generic sweep as an observation primitive for any velocity-driven
  collidable, not as projectile behavior.
- Make the opt-in a `Velocity` reference bound to the `Collidable`, not a
  projectile-specific marker or a "moving zone." Skip the sweep entirely when no
  relevant listener exists.
- Add the separate generic event/mechanism first. Do not rename
  `ProjectileHit`/`ProjectileImpact` or let them become the public name for the
  general event.
- Let MMS own projectile lifecycle, and ensure the event contract actually gives
  MMS a decision rather than notifying it after the engine has stopped the
  object.
- Package the conventional hit policy as `Projectile` on top of
  `Acceleration`/`Velocity`, so authors do not have to wire the zone event by
  hand.
- Keep sweep events contact-only. Do not emit miss/heartbeat events. Ensure every
  projectile termination path, including expiry and fuel exhaustion, unbinds the
  sweep handler so no stale contacts are delivered.
- Keep the existing projectile event and engine-owned stop behavior until the
  generic mechanism and `Projectile` lifecycle reach parity. Deprecate and
  rename only after that gate passes.
- Extend target coverage to vehicles and general objects, with explicit
  player/accessory/tree exclusion, before treating the API as stable.
- Keep performance constraints: fixed-step integration, retained broad phase,
  and no per-step allocations or script polling in the hot path.
