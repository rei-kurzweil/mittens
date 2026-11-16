# Velocity system test module split

Status: proposed follow-up. Implement only when requested or approved.

## Observation

`src/engine/ecs/system/velocity_system.rs` is about 970 lines, and roughly 550
of them are the inline `#[cfg(test)] mod tests` block at the top of the file.
The production part (fixed-step scheduling, gravity, acceleration, projectile
sweep, the `ZoneContact` hook, and integration) is about 420 lines and cohesive.

The generic swept-zone producer was added as its own module
(`swept_zone_events.rs`, with the box narrow phase in `box_sweep.rs`), so the
velocity step only snapshots poses and calls it.

## Proposed split

- Move the inline tests to `velocity_system_tests.rs`, matching
  `static_contact_tests.rs` and `climbing_tests.rs`.
- Keep `step` as one function. Do not split integration, the projectile sweep,
  and the swept-zone hook into separate passes: each fixed step must keep one
  enumeration of velocities and no per-step allocations.

## Validation

- `cargo test --lib velocity` and `cargo test --lib swept_zone_events` are unchanged.
- The `twilight_rooftop` and rocket scene tests still pass.
