# System-world teardown and event-payload module boundaries

Status: proposed follow-up. Further splits require an explicit request or approval.

## Observation

Adding the net system appended small, per-family blocks to two large files:

| File | Approximate lines | Growth point |
| --- | ---: | --- |
| `src/engine/ecs/system/system_world.rs` | 3,090 | `remove_subtree_immediate` per-component teardown chain |
| `src/scripting/runner.rs` | 2,190 | `event_arg_value` `EventSignal` → MMS `Value` conversion |

Every new I/O or interaction family adds another arm to both.

## Proposed split

- Move `event_arg_value` and its helpers (`headers_to_value`, `look_value`, …)
  into a `runner/event_payloads.rs` module grouped by family (input, HTTP, net,
  XR). Keep the function name stable: `documentation_tests.rs` locates the
  converter body by splitting on `fn event_arg_value` and `impl MeowMeowRunner`,
  so update that test in the same change.
- Move the teardown chain out of `remove_subtree_immediate` into a
  `system_world_teardown.rs` sibling (like `system_world_update.rs`), keeping
  a single pass over `owned_subtree(root)` and the existing typed lookups.

## Constraints and validation

No dynamic dispatch or per-node allocation in teardown; keep one subtree walk.
Validate with `cargo test --lib` (documentation catalog tests included) and a
subtree-removal-heavy scene such as `examples/sweeping-zone-query-events.mms`.

## Naming review (2026-11-13)

The multiplayer naming pass also updates the existing event/intent enums,
runner conversion, and world hooks. A future split should group signal enums
and intent routing by cohesive family alongside the payload/teardown proposal
above. Preserve enum representation, direct dispatch, and allocation behavior;
validate catalog coverage and multiplayer end-to-end tests. No split is part
of this rename.

## MMS channel integration

UDP integration adds registration routing, teardown and Sample payload conversion. Keep the native endpoint separate from the ECS adapter. Preserve batched/direct dispatch and avoid per-target per-frame allocations. Validate with channel MMS and avatar tests.

Remote-avatar pose support touches AVC/OpenXR registration only to select generic external pose ownership. Future splits should group pose-driver discovery/initialization separately from avatar solving, and tracked-device registration separately from XR frame orchestration. Preserve direct target handles and validate local/external hand isolation and IK activation.
