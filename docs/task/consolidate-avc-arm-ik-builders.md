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

## Final semantics

- A table field that is absent uses the engine default for that field. An empty table means all defaults. A side without a table keeps today's default behavior.
- `forbidden_bend_normal_z_degrees` is the complete exclusion list for that side. `[]` means no exclusions. The array is never appended to another AVC setting.
- Reject a second table-builder call for the same side. Validate keys and values before updating AVC. Unknown keys, wrong types, nonfinite numbers, zero-length poles, weights outside `[0, 1]`, or invalid angle pairs are errors identifying the arm and field. Angles are degrees in `[-180, 180]` and body-local about +Z; split wraparound exclusions into two pairs.
- AVC remains the owner of rig bindings, XR targets, chain creation, and tracking validity. The table changes solve policy only. There is no new `TwoBoneIK` ECS child and no second IK solver for the same arm.

## Phase 1: add the per-arm table API

1. Add a typed per-arm policy to `AvatarControlComponent`. Preserve authored-field presence for serialization, including `forbidden_bend_normal_z_degrees = []`. Temporarily translate calls to the four old builders into that policy, tracking their origin so a same-side old/new mix can be rejected. Old-only scenes still resolve their previous values.
2. Register `left_two_bone_ik` and `right_two_bone_ik` in `src/scripting/runtime_config.rs`. In `src/scripting/component_registry.rs`, parse either table value form used by MMS (`Value::Map` or object-backed table), validate the whole value, then assign the typed policy. Reject duplicate calls rather than applying the last one.
3. In `AvatarControlSystem`, use the per-arm policy when constructing each generated `IKChainComponent`: pole direction, end rotation, weight, and exclusion ranges. Put effective ranges on the chain's two-bone policy so `IKSystem` reads them there; preserve the body-local reference needed to evaluate those angles. Keep the existing solver math and debug clipping status.
4. Update `AvatarControlComponent::to_mms_ast` to emit the table methods, including when the scene was authored with old builders. A saved scene may contain resolved tables instead of the original factory invocation; preserving the factory source is a separate feature. Document the schema, units, and defaults.
5. Exercise the new API in a focused scene or test, while preserving the existing inspection scene's values and behavior. Do not invent model-specific bend ranges.

During this short transition, if an AVC mixes old and new settings for the **same side**, report a configuration error. Do not implement a merge or precedence policy that phase 2 would immediately discard. Old-only scenes must continue to work until phase 2 lands.

## Phase 2: migrate everything and remove the old builders

1. Convert **all** repository MMS callers of the four old builders to the per-arm tables, retaining their exact poles and ranges. Include `examples/ik-rest-pose-vr.mms` and the other VR examples found by search. Convert Rust callers and assertions, including the inspection example and scripting tests, to the new typed policy. Add factory files only where they improve reuse.
2. Remove the four old MMS builder registrations and handlers, and the corresponding Rust `with_left/right_arm_pole_direction` and `with_left/right_arm_forbidden_bend_normal_z_degrees` methods. Remove their dedicated AVC fields and temporary translation/origin tracking after every reader uses the per-arm policy.
3. Delete old serialization paths for those calls. Update component docs and remaining task prose that presents them as current authoring syntax. Historical bug reports can stay historical.
4. Search source, examples, assets, and active documentation for the four names. There should be no live use or parser support. An old scene loaded after this phase should fail with a targeted message pointing to `left_two_bone_ik({ ... })` or `right_two_bone_ik({ ... })`.

## Acceptance checks

- No-config AVC arms behave as before. A table for only one side leaves the other side unchanged.
- Phase 1 preserves old-only scenes while rejecting old/new mixes on the same side. Phase 2 migrates all repository scenes and tests to tables.
- An explicit empty range list means no exclusions; an omitted range field uses the default empty list. Duplicate table calls and bad fields fail with useful errors.
- Scene save/reload and AVC reinitialization preserve the effective policy, including explicit empty lists and `copy_end_rotation = false` or `weight = 0.0`.
- The VR inspection scene runs with Reimu and bisket, maintains hand/controller tracking, and its clipping readout reflects the effective restrictions. AVC still creates no more than one two-bone chain per eligible arm.
- Phase 2 removes the four builder methods and dedicated AVC fields, and a repository search finds no live callers.

## Code to change

- `src/engine/ecs/component/avatar_control.rs`: per-arm policy, defaults, serialization, old field removal.
- `src/scripting/runtime_config.rs` and `src/scripting/component_registry.rs`: MMS builder signatures and table parsing.
- `src/engine/ecs/system/avatar_control_system.rs`: resolve effective per-arm policy when constructing chains.
- `src/engine/ecs/component/ik_chain.rs` and `src/engine/ecs/system/ik_system.rs`: store and consume effective restrictions on the generated chain.
- `examples/ik-rest-pose-vr.mms` and relevant component documentation: migrate authoring examples.
