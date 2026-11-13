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
