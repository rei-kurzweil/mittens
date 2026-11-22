# Deliver damage impacts without ECS graph churn

Status — proposed.

## Current behavior

`Damage.at(x, y, z, radius, amount)` creates a temporary `DamageComponent` for
each impact. During render preparation, `DestroyableSystem` collects the
indexed damage components, sorts them, removes their subtrees, and routes the
impact volumes to overlapping destroyable surfaces. A subthreshold hit still
goes through component creation and removal even though it produces no cut.

Creating and removing these components changes `World::query_revision()`.
The next collision-index sync can revisit all registered collidables and their
dependencies. Damage delivery also allocates and frees temporary components and
maintains their indexes. The frame-time contribution of this churn has yet to
be measured.

## Proposed change

- Keep the existing MMS `Damage.at(...)` call and validation. Route the center,
  radius, and amount into a typed engine signal. A damage intent fits the
  existing command path because the call requests a mutation.
- Route the intent directly into a reusable queue owned by the destruction
  system for that world. Drain it once at the existing render-preparation
  stage, in submission order. Keep current blast overlap, per-cell damage,
  worker backpressure, and cut publication behavior.
- Migrate native producers and fixtures to the same payload path. Audit scene
  loading and runtime callbacks before removing `DamageComponent` and the
  damage-ID index. Verify that each request is applied once and that pending
  impacts are discarded with their world.

## Validation

- Exercise both scene-authored and callback-issued `Damage.at(...)` calls,
  invalid inputs, several hits in one frame, and teardown with queued impacts.
- Confirm that submitting a hit creates no ECS component. In a warmed scene,
  verify that a subthreshold impact changes no query revision or collision-index
  refresh count attributable to impact delivery.
- Run the pickup, static destruction, and rocket impact fixtures. Check
  accumulation, overlapping targets, and publication of visible and collidable
  holes at the same threshold as before.
- Replay the same automatic-fire sequence before and after. Record allocations
  per hit, collision-index refresh counts, and CPU frame-time percentiles.
  Measure impact delivery separately from target scanning and mesh cuts.

## References

- [Current destruction behavior](sakura-runtime-destruction.md)
- [Impact consumption](../../src/engine/ecs/system/destroyable_system.rs#L496)
- [Collision index invalidation](../../src/engine/ecs/system/collidable_bvh.rs#L263)
