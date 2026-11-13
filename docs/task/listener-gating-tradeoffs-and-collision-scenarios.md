# Listener gating: semantics, trade-offs, and collision scenarios

Status: design/benchmark note. This document describes the intended use of the `ListenerGate` scaffolding on this branch; it is **not evidence that generic swept-zone queries are fully integrated or production-ready**.

## Why gate work?

A producer may have expensive work to do before it can emit an event: enumerate eligible moving collidables, calculate swept bounds, query a broad phase, run narrow-phase tests, and construct event payloads. If no active handler can receive that event, doing those tests produces no observable result. A listener gate lets the producer skip that event-specific work.

This optimization is useful only when the producer can safely skip detection. It must not be used to suppress physical contact response, movement integration, unrelated events, or behavior whose result is needed even without an event listener.

The gate is a snapshot of handler registrations in `RxWorld`, not a collision filter and not a spatial index. It answers whether a handler could receive an event based on event kind and routing scope. It does not answer whether a mover can hit a surface.

## Event routing semantics relevant to the gate

For an event with kind `K` and event scope `S`, dispatch considers:

1. active global handlers registered for `Any`;
2. active global handlers registered for `K`;
3. active scoped handlers registered for `Any` at `S` or an ancestor of `S`;
4. active scoped handlers registered for `K` at `S` or an ancestor of `S`.

A gate for `K` therefore needs to account for both `K` and `Any`, and must ignore inactive/bypassed bindings. A global listener is global **for the matching kind**: an unrelated global event handler must not open the gate for `K`.

`ListenerGate::allows(world, scope)` represents the scoped/ancestor question, and `any()` means there is at least one active global or scoped listener for the requested kind or `Any`. The current API does not expose a distinct global-fast-path accessor; adding or using one is a future implementation detail.

## Scoped versus global handlers

### Scoped handler

A scoped handler subscribes on a component. An event emitted on a descendant scope can reach that handler by walking the event scope's ancestry. This gives a natural ownership boundary: a handler on a rocket zone observes that rocket's hit events, and a handler on one train root can observe its train's zone event without observing every other train.

**Trade-off:** scoped handlers allow useful work filtering. The producer may sweep only moving zones for which an active handler is attached at the zone or an ancestor. Determining that may require an ancestry check per eligible mover unless scopes are indexed/cached. For a small number of movers this is likely cheap; for many movers it may matter.

### Global handler

A global handler receives every event of its registered kind, regardless of event scope. A matching global handler is a broad subscription, not a way to observe only one object. If a global handler exists for `ZoneSwept`, every eligible mover could produce an event that it can receive.

**Trade-off:** the producer cannot use scope to discard movers solely because no scoped listener applies. It still can—and should—use other eligibility rules such as explicit velocity binding, enabled state, supported shape, collision masks/groups, and broad-phase candidates. A global handler should not force unrelated collision systems or non-opt-in objects into the swept-query set.

If a matching global handler exists alongside scoped handlers, it already makes every event of that kind observable globally. The producer can take a global fast path and avoid per-mover scope checks. Scoped handlers remain meaningful to dispatch; they do not narrow the set of events that the global handler can see.

## Collision-specific boundaries

There are multiple collision-related workloads with different contracts. Do not treat all of them as one event-driven query.

| Workload | Example | Should listener gating skip its detection? |
| --- | --- | --- |
| Generic swept-zone observation (proposed `ZoneSwept`) | A velocity-bound train sensor reports a contact with a static building; MMS decides how to respond | Yes, if no applicable listener exists, because the sweep exists only to produce this observation. |
| Existing projectile impact (`ProjectileHit`) | Rocket sphere sweeps static geometry; engine clamps/stops the rocket and emits a zone-scoped hit | Not from the generic `ZoneSwept` gate. This is a separate compatibility path with physical/lifecycle effects. Any optimization needs a separate proof that it preserves impact response. |
| Static contact / sliding movement | Avatar or moving body is kept from penetrating floors/walls; velocity grounding/friction may be updated | No. Contact solving is required for movement correctness even if no event handlers exist. |
| Zone overlap observation (`ZoneEntered` / `ZoneExited`) | An enabled sensor observes slide movers and tracks overlap transitions | Potentially, but only with a gate specifically for these event kinds and careful overlap-state semantics. A gate for `ZoneSwept` must not control this system. |
| Ray, pointer, grab, and other events | A ray hit is delivered to the hit object/ancestors and perhaps global observers | A producer may use a matching-kind listener precheck only if detection has no other consumer or side effect. Do not generalize a collision gate to them automatically. |

The current rocket asset registers `on(zone, "ProjectileHit", ...)`, so that handler is scoped to the rocket's zone. `VelocitySystem` currently calls the projectile sweep path and emits `ProjectileHit`; the generic listener gate is not currently used by that old path. The rocket behavior is therefore not equivalent to a generic, passive `ZoneSwept` observation.

## Decision table for a generic swept-event producer

Assume an object has an enabled, supported collidable explicitly bound to a velocity and an eligible static candidate index is available:

| Active handlers for requested event | Gate result / intended work |
| --- | --- |
| None for `ZoneSwept` and none for `Any` | `any() == false`; skip listener-only enumeration, swept bounds, BVH candidate query, and narrow phase. Do not emit. |
| Only unrelated event kinds | Same as no listener for `ZoneSwept`; unrelated handlers must not keep this path active. |
| One scoped handler on the moving zone | Query that mover; it is in scope. Emit a hit event if contact occurs. |
| One scoped handler on an ancestor | Query descendant movers whose event scope descends from that ancestor; unrelated branches can be skipped. |
| Scoped handlers on a different branch only | Skip this mover's sweep; no handler could receive its event. |
| Several disjoint scoped handlers | Query only movers whose zone scope is under at least one registered listener scope. One gate snapshot can serve all movers in the step. |
| Active global `ZoneSwept` handler only | All eligible bound movers are observable. Skip ancestry filtering; still apply mover eligibility and broad/narrow phase. |
| Active global `Any` handler only | Same global fast path for `ZoneSwept`, because `Any` receives it. |
| Global matching handler plus scoped handlers | Same all-eligible-movers fast path. Scoped listeners continue to receive events according to normal dispatch. |
| Global handler for an unrelated event kind plus scoped `ZoneSwept` handlers | The unrelated global listener does not open the global path; use scoped filtering. |
| All matching bindings bypassed/disabled/removed | Treat as no active listener after the gate is rebuilt/refreshed; skip event-only query work. |
| Handler registration changes during a simulation step | Define snapshot timing. Prefer building once at a stable step boundary; changes then apply on the next gate snapshot, avoiding inconsistent per-mover behavior. |
| Listener exists but mover has no velocity binding / is disabled / unsupported | Do not sweep it. Listener interest is necessary, not sufficient for mover eligibility. |
| Listener exists, eligible mover, no geometric hit | Do not emit a miss/heartbeat event; observation is contact-only. |
| Listener exists, eligible mover, hit occurs | Emit the event on the moving zone scope; dispatch delivers to applicable scoped ancestors and global handlers. The generic observation itself does not stop or remove the mover. |

## What the current benchmark tells us

The `listener_gate` benchmark compares the same synthetic query workload with and without listener filtering. The reported run had 100 groups × 20 movers × 100 rounds:

- no listeners: 200,000 ungated synthetic queries versus zero gated queries;
- one scoped group: 200,000 versus 2,000 queries;
- global listener: 200,000 queries in both modes, with the gated version slower in that run because it still performed `allows()` for every mover.

This demonstrates the *work-elimination shape* for none/scoped listeners. It is not a collision benchmark: its query body is synthetic, and it does not exercise BVH maintenance, candidate counts, real narrow-phase math, or event dispatch. The global timing points to an avoidable implementation overhead: when the requested kind (or `Any`) has an active global listener, process eligible movers without a per-mover scope check. Repeat benchmarks with warmup/multiple samples before drawing timing conclusions.

## Proposed runtime structure

At a stable fixed-step boundary:

1. Build one gate for the generic swept event kind.
2. If no listeners exist, skip the generic swept-observation work for this step.
3. Enumerate a retained registry of explicitly velocity-bound collidables, rather than scanning every component. Keep unresolved reference registrations if they may become valid later; omit disabled/ineligible movers from active query work without losing their binding.
4. If there is a matching global or `Any` global listener, bypass scope filtering. Otherwise call the scoped gate against each moving zone's event scope/ancestry.
5. For interested eligible movers, query the retained static BVH using conservative swept bounds, then run validated narrow-phase sweeps.
6. Emit the earliest or otherwise contractually selected hit according to the agreed event semantics; do not mutate velocity/movement for the generic observation unless a later response contract explicitly says so.
7. Keep projectile impact and static-contact paths independent until separately migrated and tested.

This is a design sketch, not a claim that the current branch performs these steps end to end. In particular, a listener gate should be integrated only after the actual generic producer and its eligible-mover registry are defined.

## General handler guidance beyond collision

Global handlers are appropriate for genuinely system-wide concerns: telemetry, analytics, a central event router, editor inspection, or a world-level policy that intentionally observes all events of one kind. They trade locality for convenience and can make producer-side event-interest optimizations less selective.

Scoped handlers suit per-object behavior and lifecycle ownership: a projectile reacts to its own hit, a train reverses at its own endpoint, or a local subtree manages its interactions. They can provide work filtering when the event producer controls expensive detection and the event scope is known before detection.

A producer-side listener gate is valid only when all of the following hold:

- the work exists solely to produce an optional event/observation;
- no independent consumer needs the result;
- skipping it cannot alter physical state or required bookkeeping;
- the gate mirrors actual dispatch semantics (`K`, `Any`, active status, scope ancestry, and any special routing rules);
- handler lifecycle changes become visible at a clearly defined snapshot boundary.

Otherwise, optimize the work with an appropriate index or cache, but do not use absence of event handlers as permission to skip it.

## Follow-up questions to settle

- Is `ZoneSwept` per-step contact-only observation, or should it have enter/stay/exit state?
- Is the producer meant to report only the nearest static hit per mover per step, or all contacts?
- Which collidable modes/shapes are supported for generic movers and static targets?
- How are collision groups/masks represented so logical collision eligibility is independent of component type?
- Does one active global listener intentionally observe every opted-in mover in the world?
- How are event handler additions, bypasses, delivery-enable changes, and removals reflected in gate snapshots?
- Which timing/candidate metrics should the real benchmark report: eligible movers, scope-allowed movers, BVH node visits, candidates, narrow-phase tests, hits, and elapsed time?

Until these are answered and tested, keep the gate additive and keep the existing projectile and contact behavior unchanged.
