# Velocity-bound swept-zone work: review and staged recovery

Status: review checkpoint / implementation plan. The generic swept-zone feature is **not complete**. The train scene is visual/inert fixture coverage; it does not demonstrate generic collision behavior. The current `Surface::swept_shape` / `MovingProxy` code in `collidable_bvh.rs` is an unvalidated experiment and is not approved for runtime use. Do not wire it into a producer or treat it as correct. Remove or replace that narrow-phase experiment in a dedicated code change before implementation proceeds.

This tracker records why the attempt was set aside, what is worth retaining, and how to resume with testable increments. It supersedes any implication in earlier task notes that the generic sweep math itself has been validated.

## Review outcome

The branch attempted several distinct concerns in one pass: a public event and component binding, listener-interest routing, shape proxies and continuous sweep math for multiple shapes, mesh handling, train fixture coverage, and a benchmark. That made it difficult to validate each contract independently. The intended runtime path is not yet demonstrated end to end, and the added `MovingProxy` narrow phase has known correctness gaps, including movement sampling, scale handling, and contact normal transformation. Box/mesh handling also relies on approximations that need independent geometric tests.

The useful architectural direction remains: a velocity-bound collidable is an explicit opt-in; listener interest can avoid optional event-only detection; candidate lookup should reuse the retained static BVH; generic contacts should remain separate from projectile lifecycle policy. The lesson is to validate the signal gate, registration/indexing, and one narrow-phase shape independently before expanding the feature.

## Keep as scaffolding (not a completed feature)

- **`ListenerGate` in `RxWorld`:** useful handler-interest snapshot for a specific event kind, including `Any`, scoped subscriptions, ancestor routing, and inactive/bypassed event bindings. Its tests are independent of sweep geometry.
- **The velocity-reference-on-`Collidable` idea:** a reasonable future opt-in contract, but it is not yet a maintained registry or a proven runtime binding. MMS parsing/serialization and resolution behavior still need explicit coverage.
- **`ZoneSwept` as a proposed event identity/payload:** useful as an additive observation contract. Keep it separate from `ProjectileHit`; do not claim scripts can use it end to end until the MMS signal mapping, payload conversion, and runtime emission are all present and tested.
- **The existing retained `CollidableBvh`:** keep the static broad-phase infrastructure and its lifecycle tests. Any additional consumer must respect shared contact-change ownership; do not create a second independently draining index.
- **The benchmark and gate regression tests:** keep as gate/filtering measurements. The benchmark uses synthetic work and is not a physics-performance or sweep-correctness result.
- **The train assets/example:** keep as a visual fixture if useful. It is currently intentionally inert and must not be used as evidence that generic contact events work.

## Replace / do not rely on yet

- **Current `MovingProxy`, `moving_proxy`, `advance_to_contact`, `probe_box`, and `Surface::swept_shape`:** considered an unvalidated experiment. Do not call, extend, or build event behavior on this implementation. Remove it or replace it with a separately tested narrow phase before enabling runtime use.
- **Any assumed end-to-end generic event path:** there is no verified runtime integration that resolves `Collidable.velocity_source`, checks listener interest, queries broad-phase candidates, calls the generic narrow phase, and emits `ZoneSwept` with MMS-visible payload fields.
- **Any assumption that the synthetic benchmark proves collision savings:** it demonstrates how skipping synthetic per-mover work behaves for none/scoped/global listener layouts only.

The existing projectile-specific path is the compatibility baseline and is out of scope for this recovery. Do not change its movement clamping, velocity disabling, `ProjectileHit` semantics, or rocket MMS handler as part of the generic observation work.

## Listener-gate semantics to preserve

For requested event kind `K`, event dispatch can reach active handlers registered globally for `K` or `Any`, and scoped handlers registered for `K` or `Any` at the event scope or an ancestor.

- No active matching-kind or `Any` handlers: skip work whose sole purpose is producing that optional event.
- Scoped handlers only: query movers whose event scope is within one of those handler scopes.
- Matching global `K` or global `Any` handler: every otherwise-eligible mover is observable, so use a global fast path rather than checking ancestry per mover.
- A global subscription for an unrelated kind does not open the gate for `K`.
- Listener interest is necessary, not sufficient: mover opt-in, enabled state, supported bindings/shapes, collision filters, and broad-phase candidates still apply.
- Never gate physical contact solving, movement integration, projectile impact response, or other consumers that need collision results independent of event subscribers.

Build one gate snapshot at a defined stable simulation-step boundary and reuse it for the movers in that step. Specify when binding additions, removals, bypasses, and delivery-enable changes affect the next snapshot.

## Recovery plan: narrow, verifiable slices

### Slice A — clean boundary and documentation

- Keep the inert train/environment fixture and label its test as structural/visual coverage only.
- Ensure the experimental multi-shape `MovingProxy` code is removed or isolated from usable runtime code; do not expose it as a working sweep API.
- Keep `ListenerGate` tests for scoped ancestry, `Any`, global, and bypassed binding behavior.
- Keep the benchmark clearly labeled synthetic.

**Acceptance:** the branch does not present `swept_shape` as a validated or active runtime feature; the existing rocket/projectile tests and behavior are unchanged.

### Slice B — listener-gate benchmark interpretation

- Retain the current none/scoped/global layouts and report work counts as well as elapsed time.
- Explain that the global case intentionally performs all work; it should use a global fast path to avoid scope checks in a future producer.
- Use repeated samples/warmup for timing comparisons. Do not infer real collision performance from synthetic arithmetic.

**Acceptance:** a local run can reproduce the filtering counts, and documentation does not call it an end-to-end collision benchmark.

### Slice C — explicit bound-collidable registry, no sweep math

- Maintain collidables with a declared velocity reference, indexed by the resolved velocity component, with dependency tracking for reference/topology changes.
- Keep unresolved declarations registered for late resolution; disabling a velocity suppresses work but does not erase the authored binding.
- Add lifecycle tests for binding changes, resolution, removal, disable/re-enable, and multiple collidables per velocity.
- Do not scan all collidables to discover opt-ins every fixed step.

**Acceptance:** tests prove the registry contents and update behavior; still no generic sweep is emitted.

### Slice D — gate integration with an instrumented fake query

- Build a gate snapshot once per step.
- Count attempted event-only query calls and prove zero when there is no listener.
- Prove scoped listeners only enable movers in matching ancestry; prove global `K` and global `Any` enable all registered eligible movers without per-mover ancestry checks.
- Keep projectile and static-contact systems untouched.

**Acceptance:** deterministic tests validate query counts for none, unrelated kind, scoped same branch, scoped other branch, `Any`, matching global, mixed scoped/global, and bypass/removal transitions.

### Slice E — sphere-only generic sweep

- Reuse the already-used sphere-vs-static sweep and retained static BVH where its contract fits; do not start with general `MovingProxy` abstractions.
- Resolve the bound velocity and its driven transform explicitly; derive start/end motion from the actual integration step.
- Query conservative swept bounds, run exact sphere narrow phase, and select/document earliest contact semantics.
- Emit a passive, contact-only event. Do not stop or disable the generic mover.
- Add numerical tests for initial overlap, clean miss, thin-wall crossing, translated/rotated surface, scaled mover, earliest of multiple surfaces, and correct finite/unit normal and fraction.

**Acceptance:** focused Rust tests pass and establish the math before shape generalization.

### Slice F — MMS event integration and train behavior

- Wire event-kind parsing, script subscription registration, payload conversion, and component-reference access as one separately tested change.
- Add a scene test with a listener and a real hit, plus no-listener/disabled-binding/miss cases.
- Then animate the train and test one reversal per endpoint arrival with no projectile APIs.

**Acceptance:** an MMS handler receives usable hit identity/point/normal/fraction; train motion works without `ProjectileHit` or script polling; old rocket tests remain unchanged.

### Later, separate follow-ups

Only after the sphere path is proven, decide and test capsule sweeps, box sweeps, triangle-mesh sweeps, moving-target relative motion, collision groups/masks, and response/ignore semantics. Each addition should have its own geometry fixtures and tests. Do not combine shape generalization with registry, dispatch, or lifecycle changes in one patch.

## Benchmark result recorded from local run

The branch benchmark was reported with 100 groups × 20 movers × 100 rounds:

```text
no listeners: ungated=1.359045ms (200000 queries), gated=41ns (0 queries)
one group:    ungated=1.315868ms (200000 queries), gated=1.013309ms (2000 queries)
global:       ungated=1.306972ms (200000 queries), gated=1.580927ms (200000 queries)
```

Interpretation: no-listener and one-scoped-group cases skipped synthetic query work; the global case did not skip work and paid the repeated scope-check overhead. This supports a global fast path in the future producer. The measurements are from a single synthetic run and are not collision-query performance data.

## Resume checklist

- [ ] Remove/isolate the unvalidated `MovingProxy` and `swept_shape` experiment before extending it.
- [ ] Keep listener-gate semantics/tests and synthetic benchmark; do not connect them to projectile impact.
- [ ] Implement and test the velocity-bound collidable registry independently.
- [ ] Integrate gate checks against a counted fake query before any collision math.
- [ ] Implement and verify one sphere sweep against retained static-BVH candidates.
- [ ] Wire MMS signal parsing/payload delivery and prove real hits/misses/no-listener behavior.
- [ ] Only then drive the train and consider additional shapes.
