# Refactor: consolidate AVC arm IK builders into per-arm preset tables

Status: task draft; no runtime changes are implemented. The proposed table API is described in [AVC two-bone IK preset tables](avc-two-bone-ik-preset-tables.md).

## Goal

Make `left_two_bone_ik(config)` and `right_two_bone_ik(config)` the canonical MMS authoring surface for AVC's two-bone arm IK policy. Each takes a table that can come from an imported MMS factory. AVC continues to create exactly one arm chain per eligible side.

The four existing arm IK methods map as follows:

| Existing AVC builder | New table field |
| --- | --- |
| `left_arm_pole_direction(dir)` | `left_two_bone_ik({ pole_direction = dir })` |
| `right_arm_pole_direction(dir)` | `right_two_bone_ik({ pole_direction = dir })` |
| Repeated `left_arm_forbidden_bend_normal_z_degrees(start, end)` | `left_two_bone_ik({ forbidden_bend_normal_z_degrees = [[start, end], ...] })` |
| Repeated `right_arm_forbidden_bend_normal_z_degrees(start, end)` | `right_two_bone_ik({ forbidden_bend_normal_z_degrees = [[start, end], ...] })` |

The table also exposes `copy_end_rotation` and `weight`, which the generated AVC chain currently hard-codes to `true` and `1.0` respectively. Keep `hand_rotation_smoothing`, `head_ik_eye_height`, and `ik_debug` on AVC: they control tracked-target filtering, head IK, and diagnostics rather than a per-arm two-bone solve.

## Intended MMS

```mms
import { inspection_arm_ik } from "../assets/components/ik/inspection.mms"

let arm_ik = inspection_arm_ik()
AVC {
    left_two_bone_ik(arm_ik.left)
    right_two_bone_ik(arm_ik.right)
    // Other AVC settings and children...
}
```

The factory returns data, with no component creation or runtime ID references:

```mms
export fn inspection_arm_ik() {
    return {
        left = {
            pole_direction = [1.0, -1.5, 1.0]
            forbidden_bend_normal_z_degrees = [
                [-178.0, -115.0],
                [-100.0, -60.0]
            ]
        }
        right = { pole_direction = [-1.0, -1.5, 1.0] }
    }
}
```

These are the current inspection example's left intervals, not a measured general preset for Reimu or bisket.

## Semantics and compatibility

- A table field that is absent inherits the corresponding legacy AVC value, then the current engine default. An empty table changes nothing. A side without a table retains today's behavior.
- A present `forbidden_bend_normal_z_degrees` array replaces the complete legacy list for that side. `[]` explicitly removes all exclusions. Repeated legacy calls still append to their legacy list during migration.
- If old and new builders are both used on one AVC, a present table field wins regardless of method-call order. Emit a clear migration warning for that field; do not merge range lists. The other arm and omitted fields keep their prior values.
- Reject a second table-builder call for the same side. Validate keys and values before updating AVC. Unknown keys, wrong types, nonfinite numbers, zero-length poles, weights outside `[0, 1]`, or invalid angle pairs are errors identifying the arm and field. Angles are degrees in `[-180, 180]` and body-local about +Z; split wraparound exclusions into two pairs.
- Keep the four old methods accepted for existing MMS scenes and Rust callers during this refactor. Migrate repository examples to the tables and mark the old methods deprecated in documentation. Removing the methods is a separate compatibility decision.
- AVC remains the owner of rig bindings, XR targets, chain creation, and tracking validity. The table changes solve policy only. There is no new `TwoBoneIK` ECS child and no second IK solver for the same arm.

## Implementation steps

1. Add typed optional per-side overrides to `AvatarControlComponent`. Keep authored-field presence, including an explicit empty exclusion list. Retain legacy fields and builder methods for compatibility.
2. Register `left_two_bone_ik` and `right_two_bone_ik` in `src/scripting/runtime_config.rs`. In `src/scripting/component_registry.rs`, parse either table value form used by MMS (`Value::Map` or object-backed table), validate the whole value, then assign the typed override. Reject duplicate calls rather than applying the last one.
3. In `AvatarControlSystem`, resolve one effective policy per side immediately before constructing each generated `IKChainComponent`. Use it for pole direction, end rotation, and chain weight. For restrictions, move the effective range list to the chain's two-bone policy and have `IKSystem` read it there; preserve the body-local reference needed to evaluate those angles. Keep the existing solver math and debug clipping status.
4. Update `AvatarControlComponent::to_mms_ast` to emit one complete **effective** policy table per configured side when saving. This normalizes legacy-only and mixed-syntax scenes without losing inherited fields or an explicit empty exclusion list. An unconfigured side may be omitted. A saved scene can contain resolved tables instead of the original factory invocation; preserving the factory source is a separate feature.
5. Convert `examples/ik-rest-pose-vr.mms` to table calls without changing its current numbers. Search other MMS files for the four old builders and migrate suitable examples. Do not create model-specific presets until their values are measured.
6. Update AVC component documentation with table schema, units, defaults, mixed-syntax precedence, and migration examples.

## Acceptance checks

- No-config AVC arms behave as before. A table for only one side leaves the other side unchanged.
- Old-only MMS, new-only MMS, and mixed syntax resolve to the specified effective policy independent of call order.
- An explicit empty range list clears legacy ranges; an omitted range field inherits them. Duplicate table calls and bad fields fail with useful errors.
- Scene save/reload and AVC reinitialization preserve the effective policy, including explicit empty lists and `copy_end_rotation = false` or `weight = 0.0`.
- The VR inspection scene runs with Reimu and bisket, maintains hand/controller tracking, and its clipping readout reflects the effective restrictions. AVC still creates no more than one two-bone chain per eligible arm.

## Code to change

- `src/engine/ecs/component/avatar_control.rs`: typed overrides, defaults, serialization, legacy compatibility.
- `src/scripting/runtime_config.rs` and `src/scripting/component_registry.rs`: MMS builder signatures and table parsing.
- `src/engine/ecs/system/avatar_control_system.rs`: resolve effective per-arm policy when constructing chains.
- `src/engine/ecs/component/ik_chain.rs` and `src/engine/ecs/system/ik_system.rs`: store and consume effective restrictions on the generated chain.
- `examples/ik-rest-pose-vr.mms` and relevant component documentation: migrate authoring examples.
