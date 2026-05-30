# IK solver API drift breaks tests

## Status

Open test debt.

## Symptom

`cargo test` does not compile because some IK tests still instantiate older
`IKSolver` variant shapes.

Current compiler errors include:

- `IKSolver::TwoBoneIK` initializers missing `root_joint_id` and `mid_joint_id`
- `IKSolver::AimConstraint` initializers missing `copy_position` and
  `target_position_offset`
- an `AimConstraint` match pattern that only mentions `offset_yaw`

## Why this is happening

`IKSolver` has grown more explicit runtime wiring:

- `AimConstraint` now stores `copy_position` and `target_position_offset`
- `TwoBoneIK` now stores explicit `root_joint_id` and `mid_joint_id`

Most production creation paths already know about the newer fields. For example,
the MMS registry defaults the new `AimConstraint` fields and uses null
sentinels for MMS-authored `TwoBoneIK` root/mid IDs.

The failing tests still use the old constructors directly, so they fail before
any tests can run.

## Temporarily gated tests

These tests are currently gated with `#[cfg(any())]` so unrelated test work can
run:

- `src/engine/ecs/system/ik_system.rs`: `resolves_forward_reference_on_first_tick`
- `src/meow_meow/tests.rs`: `roundtrip_ikchain_target_and_end_effector_via_selectors`
- `src/meow_meow/tests.rs`: `roundtrip_ikchain_guid_handle_preserves_target_guid`
- `src/meow_meow/tests.rs`: `roundtrip_ik_chain_aim`

## Fix direction

Do not just add dummy fields everywhere and call it done. The tests should be
updated to match the new IK contract:

- `AimConstraint` fixtures should specify whether they are rotation-only or
  position-copying, and assert roundtrip preservation of both new fields.
- `TwoBoneIK` tests should either construct a real upper-arm/lower-arm/hand
  chain and pass explicit root/mid/end IDs, or become MMS registry tests that
  intentionally assert null root/mid sentinel behavior for authored
  `two_bone_ik(...)`.
- Serialization/roundtrip tests should verify whether explicit runtime joint IDs
  are intentionally omitted from MMS output, since `to_mms_ast` currently emits
  only `pole_direction` and `copy_end_rotation` for `TwoBoneIK`.

