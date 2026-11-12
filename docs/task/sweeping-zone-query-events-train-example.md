# Sweeping-zone query events: train example

Date: 2026-11-12

Status: slice 1 implemented (inert visual fixture); generic swept-zone runtime work is deferred and unvalidated. See [the review and recovery tracker](velocity-bound-swept-zones-review-and-recovery.md) for scope, disposition, and staged follow-up.

Related:

- [Swept collision events and projectile lifecycle](../analysis/swept-collision-events-and-projectile-lifecycle.md)
- [Listener gating trade-offs and collision scenarios](listener-gating-tradeoffs-and-collision-scenarios.md)
- [STA rocket launcher](sta-rocket-launcher.md)

## Goal

Build a non-projectile train/environment scene in small stages. The current scene and its headless test establish only the static visual fixture: buildings, track, train, and an inert sensor. They do **not** prove velocity-bound sweeps, listener gating in a producer, or event delivery.

The intended follow-up sequence is deliberately staged:

1. Keep and validate the event listener gate independently.
2. Maintain an explicit registry of collidables bound to velocity.
3. Prove gate decisions and skipped-query counts with an instrumented fake query.
4. Implement and verify a sphere sweep against retained static-BVH candidates.
5. Wire the generic event into MMS and test listener/no-listener/hit/miss cases.
6. Drive the train only after the generic event path is proven.

Do not extend the experimental `MovingProxy`/`swept_shape` implementation as though its math were validated; its removal/replacement is tracked in the recovery document.

## Train fixture

The scene is based on `examples/paper-craft.mms`. It reuses two floating buildings as endpoints and adds a visual track and a two-car train. The track is visual-only to avoid train-sweep contacts with rails/sleepers. The train's named zone/collidable sensor remains inert: no velocity binding, no sweep handler, and no movement.

`src/scripting/paper_craft_scene_tests.rs` checks scene structure, placement, separate `CombineMesh` batches, and the absence of projectile or generic sweep setup. This is fixture coverage only, not a collision test.

## Generic swept-zone work: deferred

The desired API direction remains a velocity reference on a collidable and a zone-scoped, contact-only swept event that reports contact without imposing projectile lifecycle policy. The event scaffolding and `ListenerGate` do not yet constitute a complete runtime feature. In particular, registration/resolution, listener-gate use in a producer, validated sweep math, MMS event mappings/payloads, and real event tests must each be established explicitly.

The current projectile path remains a separate compatibility baseline. Do not rename or change `ProjectileHit`/`ProjectileImpact` or rocket behavior as part of the train/generic work.

## Deferred train motion

After the generic sphere path and MMS delivery have focused tests:

- bind the train sensor's collidable to its velocity by reference;
- register a scoped listener on the zone or an applicable ancestor;
- verify the engine skips event-only query work when no listener exists;
- verify a static building hit produces one event with useful contact data and a miss produces none;
- add train movement and ensure each endpoint arrival reverses exactly once.

No velocity binding, sweep event, projectile impact marker, or script polling should be added to the inert fixture before those prerequisites are met.
