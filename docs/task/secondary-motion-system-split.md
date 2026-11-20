# Split `secondary_motion_system.rs` by responsibility

Status: proposed (not started)

`src/engine/ecs/system/secondary_motion_system.rs` is about 1,450 lines. Binding,
rest targets, and stretch already live in sibling files via `#[path]`. The
remaining file mixes:

- system state, registration, lifecycle signals, and rebind bookkeeping;
- the per-frame tick (global pause, rest weights, accumulator, dirty roots);
- collision frame construction and per-chain contact gathering;
- the swing solver (`simulate_step`, `clamp_swing`, sphere/plane exclusion);
- writing joint rotations (`apply_rotations`) and debug snapshots.

## Proposal

Move the solver math (`simulate_step`, `clamp_swing`, `exclude_*`, `SwingLimit`)
to `secondary_motion_solver.rs`, and collision frame/contact gathering to
`secondary_motion_contacts.rs`. Both stay child modules of the system, like
`binding`, so `BoundChain`/`JointConfig` remain private and nothing becomes
dynamically dispatched.

## Constraints

- Keep `simulate_step` a direct, monomorphic call in the fixed-step loop; no
  trait objects or per-step allocation.
- `ChainContacts` stays built once per chain per frame, not per substep.
- No behavior change: existing `secondary_motion_*_tests.rs` must pass
  unchanged.

## Validation

`cargo test --lib secondary_motion`, plus a before/after
`CAT_DEBUG_SECONDARY_MOTION=1` run of `examples/mitten.mms` comparing
secondary-motion frame time.
