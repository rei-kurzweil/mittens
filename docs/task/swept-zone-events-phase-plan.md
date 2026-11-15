# Collision event unification: plan and backlog

Date: 2026-11-12

Status: planning note. No generic zone-collision event or producer is
implemented. This document records the event-first order, the corrections we
made while reading the older design docs, and the deferred demo/validation work.

## Purpose

Unify collision observations behind one event vocabulary before building scene
demos:

1. **Event surface.** Shared identity plus optional `SweptContact`, a
   `ZoneCollided` event, `SignalKind`, MMS plumbing, catalog docs, and dispatch
   tests. This lands with no producer and no scene.
2. **Detection and production.** A reusable sweep query, velocity binding,
   listener gate, and an observational producer tested with a headless scene.
3. **Additive lifecycle payload.** `ZoneEntered` / `ZoneExited` adopt the shared
   identity and optional `swept` fields without changing their transition
   semantics.
4. **Deferred demos and shape validation.** Train sphere reversal, box sweep
   fixtures, and the circular-aperture scene.
5. **Deferred eligibility unification.** Collision classes, membership groups,
   and relative/mover-vs-mover sweeps.
6. **Deferred convergence.** The projectile response becomes a consumer of the
   shared detector and event rather than a parallel special case.

The event machinery is intentionally shape- and scene-agnostic. Do not make the
first landing depend on the train example, a moving asset, or an MMS scene.

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
work and should not be smuggled into the first event slice.

## Priority: unify events before demos

The event vocabulary and delivery path are the durable part of this work. Scene
demos and shape validation are consumers; they should not gate the event model.

Start with the pieces that need no moving asset, no train, and no MMS scene:

1. **Event surface.** Shared payload types, `ZoneCollided`, `SignalKind`, MMS
   parsing/payload conversion, catalog docs, and dispatch tests. This is
   testable with no producer.
2. **Detection core.** A reusable sweep that returns collision identity plus
   optional `SweptContact`, independent of projectile response. Test it directly
   on fixtures.
3. **Eligibility and producer.** `velocity_source` resolution, gate snapshot,
   and observational emission from the fixed step. Test with a synthetic mover
   and a static box in a headless world.
4. **Additive lifecycle payload.** `ZoneEntered` / `ZoneExited` adopt the shared
   identity and optional `swept` fields without changing transition semantics.
5. **Deferred consumers.** Train sphere reversal, box aperture validation,
   class/group filtering, and projectile convergence.

That order keeps the first landing reviewable: data shapes, parser, tests, and
docs. The demo can be replaced by a headless scene test until the visual example
is worth building.

## Relationship to existing docs

### `sweeping-zone-query-events-train-example.md`

This is the train demo task document (workstream E1).

Notes and corrections:

- Its **slice 2** ("generic swept-zone event mechanism") is our **Phase 1**.
- Its **slice 3** ("train motion and reversal") is the Phase 1 scene demo, using
  a sphere probe rather than the box sensor. It is deferred; the first
  producer proof is a headless scene test.
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

- Its **Stage 3** ("generic producer and listener gate") is actually Phase 1B
  work (workstream C). Phase 2 assumes the producer and gate already exist and
  only validates box geometry and the two fixture scenarios.
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

## Untangled backlog

`[ ]` not started, `[-]` partially present, `[x]` implemented in this checkout.
"Deferred" means intentionally after the event surface and detection core, not
blocked by a decision.

### A. Event surface (first landing; no producer)

- [ ] A1. Settle the public name and vocabulary. `ZoneCollided` is the working
      name. Reconcile it with the overlap/contact rule in
      `spatial-collision-and-physics-naming.md`: `ZoneEntered` / `ZoneExited`
      are overlap observations, and a swept hit is also an observation rather
      than a solved contact. The event name and guide wording must agree.
- [ ] A2. Add shared payload types: collision identity (`zone`, optional
      `collidable`, `other_zone`, optional `other_collidable`, optional
      `velocity`) and `SweptContact` (`fraction`, `point`, `normal`). Keep
      `swept` strictly all-or-nothing.
- [ ] A3. Add `EventSignal::ZoneCollided` and `SignalKind::ZoneCollided`,
      including the `EventSignal::kind()` arm.
- [ ] A4. MMS plumbing: parse in `world_evaluator.rs` and `host.rs`, payload
      conversion in `runner.rs`, event-name registration in `runtime_config.rs`.
- [ ] A5. Catalog and documentation test: add a `catalog:signal` entry to
      `docs/how_to/guide/signals.md` and the `documentation_tests.rs` arrays.
      While there, add `ProjectileHit`, which currently has no catalog entry.
- [ ] A6. Dispatch tests with a directly constructed event: scoped and global
      delivery, nested `swept` map, `swept == null`, and the required
      `event.swept != null` guard before nested field access.
- [ ] A7. Additive lifecycle payload: add optional `other_collidable` and
      `swept` to `ZoneEntered` / `ZoneExited`. Keep existing fields required and
      keep `ZoneObservationSystem` behavior unchanged.
- [ ] A8. Write the `ProjectileHit` compatibility note: it stays unchanged until
      workstream F and is not the public name for the unified event.

### B. Detection core (no scene)

- [ ] B1. Extract a reusable collidable sweep returning the moving zone and
      collidable, hit zone and collidable, and optional `SweptContact`. Reuse
      `Surface::sphere_sweep` and `TriangleMesh::sweep`; do not call the
      projectile's clamp/stop/disable code.
- [ ] B2. Define nearest-hit selection and self-subtree exclusion. Preserve the
      fixed-step, allocation-conscious path.
- [ ] B3. Direct fixture tests: hit, miss, initial overlap, nearest of two,
      `fraction` in `[0, 1]`, finite point/normal, and a mesh opening that
      produces no hit.
- [ ] B4. Box and capsule dispatch stay in workstream E. Do not approximate a
      cube with a sphere.

### C. Eligibility, gate, and producer

- [ ] C1. Resolve `velocity_source` to a runtime id and retain the binding.
      Define behavior for removal, disable, reparent, and unresolved references.
- [ ] C2. Snapshot `ListenerGate` once per fixed step and pass it into
      `VelocitySystem::step`, or give the producer scoped access to `RxWorld`.
- [ ] C3. Gate tests with counted queries: no listener, only unrelated global,
      scoped mover, scoped ancestor, unrelated branch, matching global, `Any`
      global, and disabled/bypassed binding.
- [ ] C4. Repeat semantics: decide whether continuous contact emits every step,
      once per `(mover, surface)` edge, or only on first contact. Initial overlap
      reports `fraction = 0`, so `swept` presence alone does not dedupe.
- [ ] C5. Emit `ZoneCollided` from the velocity substep before `delta_world` is
      applied, with no motion mutation.
- [ ] C6. Headless MMS scene test with a synthetic mover and static box; the
      train is not required.
- [ ] C7. Coexistence test: the rocket still receives `ProjectileHit`, and the
      generic path never emits it.
- [ ] C8. Decide whether a velocity-bound sensor remains a normal static target
      for other queries.

### D. Collision class and groups (deferred implementation)

- [ ] D1. Define class separately from `CollidableMode`: static,
      velocity-driven, externally driven, dynamic.
- [ ] D2. Define membership/filter groups and where eligibility is resolved
      (broad phase, not event dispatch).
- [ ] D3. Add relative and mover-vs-mover sweeps only after classes and groups
      exist.
- [ ] D4. Keep the `ZoneCollided` payload and routing unchanged when
      classes/groups land; test that a class change does not rename the event.

### E. Scene demos and shape validation (deferred)

- [ ] E1. Train sphere probe, velocity binding, listener, and reversal demo.
- [ ] E2. Square-tunnel and circular-aperture geometry fixtures.
- [ ] E3. Static-BVH candidate integration for box sweeps.
- [ ] E4. Box sensor aperture pass/block scene.
- [ ] E5. Restore the authored box sensor only after E2 and E3 pass.

### F. Projectile convergence (deferred)

- [ ] F1. Reimplement projectile stop/disable as a consumer of the shared
      detector and `ZoneCollided` emission, preserving current behavior.
- [ ] F2. Add a compatibility/deprecation gate before renaming or removing
      `ProjectileHit` / `ProjectileImpact`.

### First landing checklist (A1-A6)

1. Payload types compile and are exercised by a directly constructed event.
2. `on(zone, "ZoneCollided", ...)` parses.
3. Payload conversion exposes identity and `swept` (or null).
4. Catalog marker and documentation test pass.
5. No system emits the event yet; no behavior changes.

A7 is a separate additive payload landing. It touches live `ZoneEntered` /
`ZoneExited` producers, so it should follow the event surface and be reviewed
for MMS payload compatibility on its own.

### What each track is waiting on

| Track | Can start when | Needs the train demo? |
| --- | --- | --- |
| A. Event surface | Now | No |
| B. Detection core | Now; reuses the retained BVH | No |
| C. Producer | A, B, and resolved velocity binding | No |
| D. Class/group | Model can be decided now; implementation after C | No |
| E2/E3. Box fixtures and BVH | B; pure math can start even earlier | No |
| E1. Train sphere demo | C | Yes |
| E4/E5. Box scene | E2/E3 and C | Yes |
| F. Projectile convergence | C, once the event contract is proven | No |

## Phase 1: sphere-first generic event

> Phase 1 now lands in three pieces. **1A** is workstream A (event surface, no
> producer). **1B** is workstreams B and C (detection core, gate, producer,
> headless tests). **1C** is the train demo in workstream E and is deferred.
> "Sphere-first" refers to the detector shape, not to the demo order.

### Goal

Emit a generic, observational contact event when a `Collidable` with a bound
`Velocity` sweeps its parent `Zone` through static geometry, without changing
motion or the projectile path. The event surface from 1A exists before any
producer emits it.

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

Deferred (workstream E1). Do not start this until the event surface, detection
core, and headless producer test are green.

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

Train demo (deferred; replace with the headless scene test until then):

- run `cargo run --release -- load examples/sweeping-zone-query-events.mms`;
- the train contacts each building, reverses once per arrival, and travels
  between endpoints without spurious events;
- removing or resetting the train leaves no stale listener delivering events.

### Phase 1 acceptance

1A, event surface:

- The event is available in MMS and documented in the signal catalog.
- Directly constructed events dispatch to scoped and global handlers.
- `swept` is a complete nested payload or `null`; the guide example guards
  nested access with `event.swept != null`.

1B, detection and producer:

- No listener means no event-only query work.
- The generic path never mutates motion and never emits `ProjectileHit`.
- Sphere hit/miss/overlap and nearest-hit tests pass.
- A headless MMS scene with a synthetic mover receives the event.
- Existing projectile and static-contact behavior is unchanged.

1C, deferred:

- The train reverses using the sphere probe with one event per arrival.

### Phase 1 non-goals

- box sweeps, capsules other than the existing internal use, or mesh sensor
  shapes;
- moving targets or relative-motion sweeps;
- damage, destruction, effects, or projectile lifecycle;
- renaming or deprecating `ProjectileHit` / `ProjectileImpact`;
- changing static contact solving or any physical response;
- collision masks/groups beyond the existing static BVH eligibility.

## Phase 2: box sweep validation

> Deferred behind the event surface and detection core. The geometry fixtures
> and static-BVH integration (E2/E3) may start after workstream B and do not
> need the train demo.

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
   Reconcile the choice with the overlap-versus-contact wording in
   `spatial-collision-and-physics-naming.md`.
2. Optional-sweep semantics: confirm that `swept` is strictly all-or-nothing,
   define `fraction` as `[0, 1]` within the step with no sentinel values, and
   decide whether `ZoneEntered`/`ZoneExited` adopt the same `swept` struct
   immediately (A7) or in a follow-up.
3. Event/lifecycle relation: does an overlap detector also emit
   `ZoneCollided` on the entry step, every overlapping step, or not at all
   while `ZoneEntered`/`ZoneExited` own persistent state? Recommendation:
   lifecycle variants remain the only once-per-transition events, and
   `ZoneCollided` is instant detection-specific contact.
4. Repeat semantics: an initial overlap reports `fraction = 0` every step, so
   contact-only is not automatically one-event-per-contact. Decide between
   producer-side `(mover, surface)` edge dedupe and an MMS-side guard. The train
   reversal requires at most one action per arrival.
5. Gate snapshot timing and transport: build it once per substep in
   `SystemWorld` and pass it into `VelocitySystem::step`, or give the producer
   direct access to `RxWorld`.
6. Runtime binding resolution: resolve `velocity_source` each step or store a
   resolved id. Also decide what happens when the velocity is removed,
   disabled, or reparented.
7. Whether a velocity-bound sensor collidable remains a normal static target
   for other queries.
8. Whether the first producer reports only the nearest static hit per mover per
   step or all contacts. Start with nearest.
9. Payload compatibility for A7: adding `other_collidable` and `swept` to
   `ZoneEntered`/`ZoneExited` should be additive in MMS. Confirm that existing
   handlers can ignore the new keys and that missing keys resolve to
   `Value::Null`.
10. Collision class/group model: define classes (static, velocity-driven,
    externally driven, dynamic) and separate membership/filter groups before
    enabling mover-vs-mover pairs. Keep this out of the event payload and out of
    the first producer slice.

## Suggested implementation order

Event-first, in landing-sized slices:

1. Shared payload types and `ZoneCollided`/`SignalKind` (A2-A3).
2. MMS parse, payload conversion, catalog, and documentation test (A4-A5).
3. Direct dispatch and nested-`swept` tests (A6).
4. Additive `ZoneEntered`/`ZoneExited` payload adoption (A7).
5. Reusable sweep core and direct fixture tests (B1-B3).
6. `velocity_source` resolution and retained binding (C1).
7. Gate snapshot transport and gate tests (C2-C3).
8. Repeat semantics decision (C4).
9. Producer emission and headless scene test (C5-C7).
10. Collision class/group model (D), then relative sweeps.
11. Box geometry fixtures and static-BVH integration (E2-E3).
12. Train sphere demo (E1), then box scene (E4-E5).
13. Projectile convergence (F1-F2).

## Boundaries

- Do not reuse `ProjectileHit` for the generic observation.
- Do not change the rocket's current behavior or naming.
- Do not gate static contact solving or any result needed for physical response.
- Do not silently approximate one collision shape with another.
- Do not add per-draw work, per-step allocations, or script polling to the
  detection path.
- Keep the static BVH and fixed-step integration contracts.
