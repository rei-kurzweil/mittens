# Sweeping-zone query events: train example

Date: 2026-11-12

Status: proposed

Related:

- [Swept collision events and projectile lifecycle](../analysis/swept-collision-events-and-projectile-lifecycle.md)
- [STA rocket launcher](sta-rocket-launcher.md)

## Goal

Build a new example, `examples/sweeping-zone-query-events.mms`, from the
`examples/paper-craft.mms` base, then use it to prove generic swept-zone events
with a non-projectile mover.

The work is deliberately split:

1. Build the environment and prefabs first. The train must be completely inert.
2. Verify the environment.
3. Add the generic velocity-aware swept-zone event mechanism.
4. Drive the train with that event: it travels between two floating buildings,
   stops and reverses at each building, and continues the other way.

The example must not use `ProjectileImpact`/`ProjectileHit`, and must not depend
on projectile teardown behavior.

## Problem statement: velocity-aware collision events

The desired mechanism is broader than a projectile event: notify MMS when an
object controlled by the physics/velocity systems collides with something
static. Detection is inherently velocity-aware because a sweep needs the
mover's motion across the step; endpoint overlap tests miss fast contacts. That
coupling to `Velocity`/acceleration is probably acceptable, but it should be
explicit and should stop at detection.

The suggested opt-in is to bind a `Velocity` component to the collidable zone by
reference in MMS. The engine can then check for that binding, check whether any
relevant sweep listener exists, and only then run the sweeping zone query. A
separate projectile-like marker should not be needed. The velocity binding is the
part that necessarily couples detection to motion; the projectile lifecycle is
not part of it.

The projectile lifecycle is a separate concern. Stopping, removing, spawning an
effect, applying damage, and unbinding handlers on expiry are not part of the
generic collision event.

There is also an API-shape question. In the scripting shell an event handler is a
subscription, not a closure with lexical capture. A useful handler may need a
context payload containing data or component references: the moving target, its
velocity component, the sensor zone, the source/owner, and any filter data. That
context would let the handler decide whether a contact matters and what to do,
without querying unrelated state by name.

The train is a good non-projectile test for all of this because it:

- is driven by velocity rather than a projectile motion provider;
- has a long body and must reverse direction;
- hits static building endpoints rather than destroyable targets;
- has no expiry or impact lifecycle, so any projectile-specific coupling will
  show up immediately.

## Deliverables

1. `examples/sweeping-zone-query-events.mms`.
2. `assets/components/trains/railroad_track.mms`, exporting a reusable
   `railroad_track(...)` factory.
3. `assets/components/trains/boxcar.mms`, exporting a reusable `boxcar(...)`
   factory and a coupled two-car train composition.
4. The generic swept-zone event mechanism, in a separate slice.
5. Focused tests or smoke checks described below.

Exact asset paths may move under `assets/components/trains/` if that grouping is
inconsistent with existing asset organization; keep the factories reusable from
the example.

## Slice 1: environment and prefabs only

Copy `examples/paper-craft.mms` to `examples/sweeping-zone-query-events.mms` as
the starting scene. Keep the working viewer, input, sky, lighting, and scene
setup. Remove or defer unrelated weapon/prop content only if it conflicts with
the train scene.

### Buildings and track

- Add two floating buildings that serve as the track endpoints.
- Prefer purpose-built building blocks or reuse suitable paper-craft towers at a
  common height. Do not move unrelated scene content just to make the track fit.
- Run one straight elevated track between the two buildings.
- Place the train at the midpoint on the track.
- Leave clear space around the endpoints so the future train sensor can contact
  a building zone before the model visually enters the wall.
- The track itself should be visual-only by default. Do not give the rails or
  sleepers collidable surfaces that the train sensor would hit instead of a
  building.

### Railroad track prefab

`railroad_track(...)` should build:

- the wooden sleepers/ties underneath the track; and
- the two rails.

Each group must use its own `CombineMesh`:

- all wooden pieces underneath the rails are one combined mesh;
- both rails are one combined mesh.

The factory should accept enough parameters to place and size a straight track
(length, gauge, sleeper spacing, base height, material/color). Keep the generated
hierarchy named for inspection and tests.

### Boxcar prefab

`boxcar(...)` should build one boxcar out of thin cubes:

- floor, two side walls, two end walls, and a roof;
- doors partially open on at least one side of each car;
- a simple coupler/gap between the two cars;
- no motion, velocity binding, or sweep event handler.

The train composition should place two boxcars on the track with a visible gap,
matching the track gauge and height. Add a named collidable zone for the future
train sensor, but leave it inert in this slice: no bound velocity and no sweep
listener. A wheel proxy is optional; the first slice can be a simple boxcar body.

### Slice 1 acceptance

- The example loads with:
  `cargo run --release -- load examples/sweeping-zone-query-events.mms`.
- The train is static and does nothing.
- The example contains no `ProjectileImpact`, `ProjectileHit`, bound velocity on
  the train's collidable zone, or sweep event handler.
- Visual check: two floating buildings, a continuous track between them, two
  boxcars with partially open doors.
- Structural check: wooden track pieces share one `CombineMesh`; rails share a
  separate `CombineMesh`.
- No existing scene test or example is regressed.

## Slice 2: generic swept-zone event mechanism

Implement the additive generic mechanism described in the analysis document.
This slice must not rename or remove the projectile event, and must not change
current rocket behavior.

Minimum behavior:

- A collidable zone with a `Velocity` component bound to it by reference in MMS
  reports contact with static geometry during its step.
- The engine checks for a bound velocity and for a relevant sweep listener before
  running the sweeping zone query. Binding a velocity without a listener must
  not emit and should not pay for the sweep.
- The event is contact-only. A miss produces no event and there is no per-step
  heartbeat.
- The payload identifies the moving component/zone, the bound velocity, the hit
  collidable/zone, point, normal, and fraction, plus the context needed for the
  handler to decide relevance.
- The generic path does not own projectile disposal, expiry, damage, or effects.
- The existing `ProjectileHit` path continues to work unchanged.

Acceptance and smoke tests:

- A focused Rust test covers hit, miss, and initial overlap for the generic
  sweep.
- An MMS scene with a bound velocity and a listener receives the generic event.
- The same scene with no listener emits nothing and, if instrumented, does not
  run the sweep query.
- The same scene with a listener but no bound velocity emits nothing.
- The same scene emits no event on a miss.
- The old rocket tests still pass and still receive `ProjectileHit`.
- Generic and projectile sensors can coexist in one scene without cross-talk.

## Slice 3: train motion and reversal

Attach motion to the train and use the generic swept event to reverse it at the
buildings.

- Start the train moving toward building A.
- Bind the train's `Velocity` to its collidable zone by reference and register a
  listener for the generic swept event. The engine should do no sweep work until
  that listener exists.
- Give the sensor a shape that overlaps the leading end of the train, or one
  sensor at each end. The simplest correct first version may use one centered
  sensor; use a leading-end sensor if the train is late to stop.
- On contact with building A: stop, clear reverse state, and set motion toward
  building B.
- On contact with building B: stop and set motion back toward building A.
- Repeated contact events while still touching a building must not flip
  direction more than once for that arrival.
- Free travel between buildings must not emit sweep events.
- The example must not use `ProjectileImpact`/`ProjectileHit` or the projectile
  lifecycle.

Smoke tests:

- Run the example and observe the train hit A, reverse, hit B, reverse, and
  continue cycling.
- Event count per arrival is one; there is no event during open-track travel.
- If the train is removed or reset, its sweep handler is unbound and no stale
  events are delivered.
- A focused scene test can advance the clock and assert two endpoint reversals
  without duplicate callbacks.

## Open questions

1. Generic event name. Do not block slice 1 on this; use the name chosen by the
   analysis doc when the mechanism lands.
2. Velocity-binding API: builder name and placement on the zone versus its
   `Collidable` child, and whether the velocity-bound collidable remains a
   normal target for other queries.
3. Listener gating: scoped to the zone, ancestors, or global handlers; checked
   per step or cached; and behavior when a listener is added or removed after
   the velocity binding.
4. Handler context shape: which component references or data are delivered with
   the subscription/event, and whether they are captured at bind time or resolved
   at emit time.
5. Train sensor placement: one centered sphere, one leading sensor that flips
   with direction, or one sensor at each end.
6. Whether endpoint detection uses the building's existing static collider or a
   dedicated non-solid trigger zone.
7. Reversal mechanics: reset `Velocity` and reapply a local/world linear vector,
   or swap the motion provider's direction.
8. Whether the track needs a separate static collider for the player while
   remaining invisible to the train sweep.
9. Fuel/energy exhaustion is not part of the train slice, but the projectile
   lifecycle still needs a shared teardown path that unbinds sweep handlers and
   velocity bindings.

## Non-goals

- Renaming or deprecating `ProjectileHit`/`ProjectileImpact`.
- Changing the current rocket behavior or projectile lifecycle.
- Moving-target relative sweeps (the train hits static buildings here).
- Damage, destruction, or effects from train impacts.
