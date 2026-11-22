# Transform operator system split

Status: proposed follow-up. Implement only when requested or approved.

## Observation

`src/engine/ecs/system/transform_operator.rs` is about 1,860 lines: roughly
1,150 of production code and 700 of inline tests. The production part mixes
several operator families (fork/drop/pass channels, ancestor sampling,
translation sampling, camera-specific operators, stateful yaw follow and
temporal filters) with the evaluation loop and its stage-keyed state maps.

The 2026-11-16 body solver work added a small change here: yaw extraction
now uses the head's up axis when forward points steeply up or down.

## Proposed split

- Move the inline tests to `transform_operator_tests.rs`.
- Group operator evaluation by family into a `transform_operator/` folder:
  channel ops (pass/drop/extract), stateful rotation ops (yaw follow,
  temporal filter, with their state maps), sampling ops, and camera-specific
  ops. Keep the evaluation loop and dispatch in one place.
- Avoid per-op modules or trait objects: keep the `match` over op variants
  and the existing state maps so the per-frame path is unchanged.

## Validation

- `cargo test --lib transform_operator` and the avatar body-yaw tests pass
  unchanged.
- No new allocations or dispatch in the per-frame evaluation path.

## Hinge follow-up

Group authored operator settings and serialization in the component layer by
family. Keep structural basis propagation in `transform_system.rs` and give
operator refresh paths a shared boundary for replaying upstream operators
after a local edit. Keep local rotation composition with the stateless
operators. Preserve the current traversal and indexed dependency refresh.
Validate pivot and local-axis composition, live angle writes under nested
operators, and MMS round trips alongside the existing operator tests.
