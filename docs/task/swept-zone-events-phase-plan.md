# Zone collision events: sphere-first phase plan

Date: 2026-11-12

Status: planning note. No generic zone-collision event or producer is
implemented. This document records the phase split and the corrections we made
while reading the older design docs.

## Purpose

Split the generic zone-collision event work into two phases:

1. **Phase 1 — event machinery with explicit sphere sweeps.** Change the train
   example to use an explicitly authored sphere sensor, then prove the generic
   event end to end: velocity binding, listener gate, producer, payload, MMS
   delivery, and the train reversal demo.
2. **Phase 2 — box sweep validation.** Restore the box sensor and follow
   `train-box-sweep-validation.md` for the square-tunnel pass and
   circular-aperture block.

The event machinery is intentionally shape-agnostic. Phase 2 should add a
narrow-phase shape arm, not reshape the event or the gate.

## Collision model direction

A collision is one concept; the detection method is not the event identity.
Every collision event identifies the two zones/collidables. Detection methods
differ in what extra geometry they can report: a current-pose overlap may know
only that the pair intersects, while a continuous sweep can also report the
fraction along the step, the contact point, and the normal. That sweep-only
detail is bundled into one optional struct rather than spread across optional
top-level fields.

Three axes must stay separate:

1. **Eligibility/filtering** — collision class and collision groups.
2. **Detection method** — current-pose overlap, continuous sweep, relative
   sweep, mesh narrow phase.
3. **Observation lifecycle** — instant contact versus persistent
   entered/exited state.

**Classes** describe motion authority and participation: static,
velocity-driven, externally driven (slide/movement target), and later dynamic
bodies. **Groups** are explicit membership/filter masks that decide which pairs
may interact. They are orthogonal: a velocity-driven collidable still has
groups, and a static collidable still has a class. Do not encode either in the
event kind.

This plan keeps enter/exit as lifecycle events and adds one instant collision
event, working name `ZoneCollided`. The base payload is the normal collision
identity. The things only a continuous motion query can provide — fraction,
point, normal — live together in an optional `SweptContact` field. A basic
current-pose collision emits `swept = null`; a swept collision fills it.
Enter/exit can adopt the same optional struct when their producers use a sweep,
so there is one contact vocabulary rather than two.

They remain separate variants only because enter/exit own persistent pair
state and once-per-transition emission; that is a lifecycle difference, not a
payload or collision-concept difference. If the lifecycle later collapses into
one event with a phase field, the shared payload makes that a mechanical
change.

Today `CollidableMode::{Static, Slide}` conflates "static target" with "slide
mover driven by a movement target." Splitting class from group is follow-up
work and should not be smuggled into the Phase 1 sphere slice.

## Relationship to existing docs

### `sweeping-zone-query-events-train-example.md`

This is the primary task document.

Notes and corrections:

- Its **slice 2** ("generic swept-zone event mechanism") is our **Phase 1**.
- Its **slice 3** ("train motion and reversal") is the Phase 1 scene demo, using
  a sphere probe rather than the box sensor.
- Its slice 1 leaves the train sensor as
  `Zone.cube([...]) { Collidable.static() {} }` in
  `assets/components/trains/boxcar.mms`. Phase 1 changes that to an explicit
  sphere probe. The cube is not converted implicitly.
- The line "first geometry uses the existing sphere-based sweeping zone query;
  exact mapping from zone shape to sweep shape is open" is resolved here:
  **explicit sphere zone, no automatic cube-to-sphere mapping**.
- The document's status says "slice 1 implemented". That is accurate for the
  inert scene. It does not mean any part of the generic event exists.

### `train-box-sweep-validation.md`

This is the **Phase 2** document.

Notes and corrections:

- Its **Stage 3** ("generic producer and listener gate") is actually Phase 1
  work. Phase 2 assumes the producer and gate already exist and only validates
  box geometry and the two fixture scenarios.
- Its warning is preserved: do not use the Phase 1 sphere result as evidence
  for the box sweep, the square-tunnel pass, or the circular-aperture
  rejection.
- Its Stages 1–2 (geometry fixtures and static-BVH integration) remain the
  entry point for Phase 2 and should land before the box sensor is made
  interactive.

### `swept-collision-events-and-projectile-lifecycle.md`

Design analysis, not an approved implementation.

Notes and corrections:

- `ZoneSwept` is a **working name** in that document; the event name is still
  open. This plan uses `ZoneCollided` as the working name because the event is
  a collision, while "swept" describes one detection method. The sweep is not
  a separate collision concept.
- The settled placement is retained: the `Velocity` reference lives on the
  `Collidable`, and the event is scoped to the parent `Zone`.
- The transition order is retained: add the generic path alongside the
  projectile path, and do not rename or remove `ProjectileHit` /
  `ProjectileImpact`.
- The payload requirements are retained: moving zone/collidable, bound
  velocity, hit zone/collidable, point, normal, and fraction.

### `listener-gating-tradeoffs-and-collision-scenarios.md`

Notes and corrections:

- `ListenerGate` scaffolding exists in
  `src/engine/ecs/signals/rx_world.rs` and is covered by
  `tests/listener_gate.rs`, but **no producer uses it yet**.
- The document references a `listener_gate` benchmark. No benchmark target
  exists in this checkout: there is no `benches/` directory and no
  `[[bench]]`/criterion entry in `Cargo.toml`. Treat the quoted numbers as
  historical, not reproducible here.
- The current gate exposes `any()` and `allows(world, scope)`. It has no
  separate global fast-path accessor, but `allows()` returns `true`
  immediately when a matching global handler exists. Do not add another
  accessor unless profiling shows a need.

### `docs/how_to/guide/signals.md`

The user-facing event catalog and the MMS exposure rules.

Notes:

- A new event must be added to the catalog and to the documentation test.
- The MMS plumbing pattern is already established by `ProjectileHit`,
  `ZoneEntered`, and `ZoneExited`. There is no new dispatch mechanism to build.

## Current state (verified in this checkout)

- `CollidableComponent::velocity_source` and the `Collidable.velocity(...)`
  builder exist. References may be a live handle, a scoped selector, or a
  `@uuid:` GUID, and they survive serialization.
- Nothing resolves `velocity_source` to a runtime `ComponentId`, and no system
  consumes it.
- `RxWorld::listener_gate(kind)` exists and follows `Any` plus the requested
  kind, active status, and scope ancestry. It is unused by producers.
- `CollidableBvh` is the retained static broad phase. `Surface::sphere_sweep`
  provides sphere-versus-box and sphere-versus-mesh sweeps.
- `ZoneSweepHit { moving, surface, fraction, point, normal }` is the existing
  hit type. It does not carry the hit `Collidable` or the bound velocity.
- The projectile path is marker-based (`ProjectileImpact`), sphere-only, and
  owns the response: it truncates movement, zeroes velocity, disables
  `VelocityComponent`, and emits `ProjectileHit`.
- The narrow phase supports spheres and capsules. `CollisionShape::Cube` maps
  to a zero radius in `TriangleMesh::sweep`, and `Surface::sphere_sweep` takes
  a radius rather than a shape. **Box sweeps do not exist yet.**
- The train sensor is still a cube and is inert: no velocity binding, no
  listener.

## Phase 1: sphere-first generic event

### Goal

Emit a generic, observational contact event when a `Collidable` with a bound
`Velocity` sweeps its parent `Zone` through static geometry, without changing
motion or the projectile path.

### Required behavior

- The event is scoped to the moving `Zone`.
- Opt-in is the explicit `Collidable.velocity(...)` binding. A separate marker
  is not added.
- The engine checks for a bound velocity **and** a relevant listener before
  querying. No listener means no sweep work and no event.
- A matching global or `Any` global listener uses the all-eligible-movers path.
  Otherwise the scoped gate is checked against the moving zone's ancestry.
- The sweep is continuous across the fixed step, not an endpoint overlap test.
- Contact-only: no event on a miss and no per-step heartbeat.
- The generic path does **not** modify velocity, disable components, apply
  damage, spawn effects, or remove anything.
- Unsupported zone shapes are skipped by the Phase 1 swept detector. Do not
  substitute a sphere for a cube or any other shape. Unsupported geometry is a
  detector limitation, not a rule that suppresses the event concept: a detector
  that can establish the pair but not stable sweep geometry emits with
  `swept = null`.

### Event shape (working draft)

Working name: `ZoneCollided`. It is a collision event, not a sweep event; the
detection method does not change the event kind.

```text
ZoneCollided {
    zone,              // Zone the handler is scoped to
    collidable,        // Option<Collidable> owned by that zone
    other_zone,        // counterpart Zone
    other_collidable,  // Option<Collidable> owned by the counterpart
    velocity,          // Option<Velocity>; present for velocity-bound movers
    swept,             // Option<SweptContact>
}

SweptContact {
    fraction,          // [0, 1] along this step's motion
    point,             // world-space contact point
    normal,            // world-space contact normal
}
```

- The base payload is the normal collision identity: the two zones, their
  collidables when present, and the motion source when there is one.
- `swept` is the bundle of things a continuous sweep provides and a basic
  current-pose collision does not. It is either present as a complete struct or
  absent entirely.
- A basic overlap emits `swept = null`. It does not fabricate a point, normal,
  or fraction. If a basic overlap later gains stable separation geometry, that
  is a separate capability and does not change this contract.
- A swept detector always fills the struct. It should not emit a partially
  populated `SweptContact`.
- MMS handlers use `if event.swept != null { ... }`. This follows the existing
  partial/null payload convention in the signal guide.
- `ZoneEntered` / `ZoneExited` remain lifecycle events for persistent overlap.
  When their producer uses a sweep (for example the downward floor-crossing
  check for teleport pits), it can attach the same `SweptContact` struct.
- `collidable`, `other_collidable`, and `velocity` are optional because a
  collision can involve a pure sensor zone or a statically/externally driven
  side that has no bound velocity. They are identity/eligibility context, not
  part of the sweep bundle.
- A `detection`/`source` field is not required. Consumers that must know
  whether the contact came from a sweep check whether `swept` is present.

Names are not final. The important shape is: mandatory collision identity,
optional all-or-nothing sweep detail.

### Producer placement

Run inside the velocity fixed substep, before the proposed `delta_world` is
applied. This is the point where the sweep motion is already known and matches
the existing projectile path's timing.

Proposed flow:

1. Snapshot one `ListenerGate` for the event kind at the fixed-step boundary.
2. If `!gate.any()`, skip all generic sweep work for the substep.
3. Enumerate enabled collidables with a resolvable `velocity_source` whose
   resolved velocity is the current one.
4. Skip movers whose parent zone is disabled or whose shape is unsupported.
5. If there is no global listener and `!gate.allows(world, zone)`, skip that
   mover.
6. Sync/query the retained static BVH with conservative swept bounds; run the
   sphere narrow phase; skip surfaces in the mover's own subtree.
7. Emit the nearest contact. Do not mutate motion.

### Binding resolution

`velocity_source` is currently a `ComponentRef`, not a runtime id. Either:

- resolve it each step with `resolve_component_ref`, or
- add a resolved id alongside the source, mirroring
  `movement_target_id`.

Prefer a small retained registry of velocity-bound collidables if per-step
enumeration shows up in profiles. Do not scan every component in the hot path
without measuring.

### Train example change

In `assets/components/trains/boxcar.mms`, replace the inert cube sensor with an
explicit sphere probe, or add a sphere probe alongside the retained inert cube.

- A single centered sphere is acceptable for the first event test.
- Use a leading-end sphere for the reversal demo, or two end spheres, so the
  probe contacts the building before the body visually enters it.
- Name the sphere clearly as a temporary Phase 1 probe.
- Bind its velocity and register the listener in
  `examples/sweeping-zone-query-events.mms`.
- If the cube remains, it must stay inert and the producer must skip it.

### Phase 1 tests

Focused Rust tests:

- hit, clean miss, and initial overlap for the generic sphere sweep;
- nearest-hit selection when two static surfaces are in the swept bounds;
- self-subtree surfaces are excluded;
- `fraction` is in `[0, 1]`, and point/normal are finite;
- an unsupported cube sensor produces no event and no panic.

Gate tests (instrumented query count where possible):

- bound velocity, no listener: no event and no sweep;
- listener, no bound velocity: no event;
- scoped listener on the moving zone: event;
- scoped listener on an ancestor: event;
- scoped listener on an unrelated branch: no event and no sweep;
- global or `Any` listener: all eligible movers are observed;
- bypassed or removed binding stops future work.

Scripting/scene tests:

- an MMS scene with a bound velocity and `on(zone, "ZoneCollided", ...)` receives
  the event with the expected identities;
- the same scene emits no event on a miss;
- the rocket still receives `ProjectileHit` and behaves as before;
- generic and projectile sensors coexist without cross-talk.

Train demo:

- run `cargo run --release -- load examples/sweeping-zone-query-events.mms`;
- the train contacts each building, reverses once per arrival, and travels
  between endpoints without spurious events;
- removing or resetting the train leaves no stale listener delivering events.

### Phase 1 acceptance

- The new event is available in MMS and documented in the signal catalog.
- No listener means no event-only query work.
- The generic path never mutates motion and never emits `ProjectileHit`.
- Sphere hit/miss/overlap tests pass.
- The train reverses using the sphere probe with one event per arrival.
- Existing projectile and static-contact behavior is unchanged.

### Phase 1 non-goals

- box sweeps, capsules other than the existing internal use, or mesh sensor
  shapes;
- moving targets or relative-motion sweeps;
- damage, destruction, effects, or projectile lifecycle;
- renaming or deprecating `ProjectileHit` / `ProjectileImpact`;
- changing static contact solving or any physical response;
- collision masks/groups beyond the existing static BVH eligibility.

## Phase 2: box sweep validation

### Goal

Follow `train-box-sweep-validation.md` to add rectangular box sweeps and use the
train's authored box sensor as the shape-validation consumer.

### Scope

- Stage 1: deterministic square-tunnel and circular-aperture geometry fixtures,
  including the control sphere sweep through the aperture.
- Stage 2: run the fixtures through the retained static BVH and verify
  candidate selection and narrow-phase results.
- Stage 3: connect the existing Phase 1 producer to box shapes. The event
  payload and gate semantics should not change.
- Stage 4: make the train interactive with the box sensor only after the
  focused geometry and BVH tests pass.

### Phase 2 notes

- Add a shape dispatch point in the narrow phase rather than in the event or
  gate layer.
- Keep `Surface::sphere_sweep` and the projectile path intact.
- The event remains contact-only and observational; no automatic reversal or
  stop is owned by the engine.
- Restore or keep the square tunnel and circular aperture as visibly and
  geometrically distinct fixtures. Decorative geometry is not a collision test.
- Do not accept sphere-sweep results as evidence for the box cases.

### Phase 2 acceptance

- The box passes the adequately clear square tunnel with no contact event.
- The box reports contact with the narrower circular aperture.
- A fast crossing is caught between endpoints.
- Contact identity, fraction, point, and normal are valid and stable.
- The sphere probe/event tests from Phase 1 remain green.
- The train's box sensor is the only shape used for the aperture assertions.

## Open decisions

1. Final event name: `ZoneCollided` (working), `ZoneContact`,
   `ZoneCollision`, or similar. Prefer a name that describes the collision
   rather than the query technique; `ZoneSwept` only names one detector.
2. Optional-sweep semantics: confirm that `swept` is strictly all-or-nothing,
   define `fraction` as `[0, 1]` within the step with no sentinel values, and
   decide whether `ZoneEntered`/`ZoneExited` adopt the same `swept` struct
   immediately or in a follow-up.
3. Repeat semantics: an initial overlap reports `fraction = 0` every step, so
   contact-only is not automatically one-event-per-contact. Decide between
   producer-side `(mover, surface)` edge dedupe and an MMS-side guard. The train
   reversal requires at most one action per arrival.
4. Gate snapshot timing and transport: build it once per substep in
   `SystemWorld` and pass it into `VelocitySystem::step`, or give the producer
   direct access to `RxWorld`.
5. Runtime binding resolution: resolve `velocity_source` each step or store a
   resolved id. Also decide what happens when the velocity is removed,
   disabled, or reparented.
6. Whether a velocity-bound sensor collidable remains a normal static target
   for other queries.
7. Whether Phase 1 reports only the nearest static hit per mover per step or
   all contacts. Start with nearest.
8. Collision class/group model: define classes (static, velocity-driven,
   externally driven, dynamic) and separate membership/filter groups before
   enabling mover-vs-mover pairs. Keep this out of Phase 1 implementation, but
   do not hardcode swept-only assumptions into the event.

## Suggested implementation order

1. Event variant, optional `SweptContact`, `SignalKind`, and MMS plumbing.
2. Binding resolution and mover enumeration helper.
3. Gate snapshot passed into the velocity substep.
4. Shape-aware sphere sweep extracted from the projectile path without changing
   projectile behavior.
5. Focused Rust and gate tests.
6. MMS scene test.
7. Sphere probe, listener, and train reversal in the example.
8. Signal guide/catalog update.
9. Phase 2 box fixtures and narrow phase.

## Boundaries

- Do not reuse `ProjectileHit` for the generic observation.
- Do not change the rocket's current behavior or naming.
- Do not gate static contact solving or any result needed for physical response.
- Do not silently approximate one collision shape with another.
- Do not add per-draw work, per-step allocations, or script polling to the
  detection path.
- Keep the static BVH and fixed-step integration contracts.
