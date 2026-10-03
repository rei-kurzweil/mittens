# Task: pass two-bone IK preset tables to AVC

Status: design proposal. The `left_two_bone_ik` and `right_two_bone_ik` methods below do not exist yet. This proposal supersedes the child-component design originally drafted in this file.

## Decision

Add `left_two_bone_ik(config)` and `right_two_bone_ik(config)` builder methods to `AVC`. Each accepts an MMS table of settings for the arm chain AVC already creates. An imported MMS factory can return the table. No additional ECS component is needed for this use case.

```mms
import { reimu_arm_ik } from "../assets/components/ik/reimu.mms"

let arm_ik = reimu_arm_ik()
AVC {
    left_two_bone_ik(arm_ik.left)
    right_two_bone_ik(arm_ik.right)
    // Existing avatar, XRHand, and camera children...
}
```

The preset module returns plain data:

```mms
export fn reimu_arm_ik() {
    return {
        left = {
            pole_direction = [1, -1.5, 1]
            copy_end_rotation = true
            weight = 1.0
            forbidden_bend_normal_z_degrees = [
                [-178.0, -115.0],
                [-100.0, -60.0]
            ]
        }
        right = {
            pole_direction = [-1, -1.5, 1]
        }
    }
}
```

The angles above reproduce the current **inspection-scene prototype**, not a validated Reimu preset. Name and ship model-specific presets only after measuring them. Bisket need not inherit Reimu's left exclusions.

For a small scene, inline tables are enough:

```mms
AVC {
    left_two_bone_ik({
        pole_direction = [1, -1.5, 1]
        forbidden_bend_normal_z_degrees = [[-178.0, -115.0], [-100.0, -60.0]]
    })
}
```

## Why this shape

MMS already has table values, arrays, imported functions that return data, and builder methods that accept values. Existing examples use tables for presets; `assets/components/mouth_response/rei_2026.9.mms` returns a plain table. The Rust side already handles table arguments for some builder methods, such as `EditorUI.panels`.

A single `two_bone_ik({ left = ..., right = ... })` method could also work. Separate side methods fit the existing `left_arm_pole_direction` and `right_arm_pole_direction` AVC API and let a scene configure only one arm. A factory can still return both sides in one object, as shown above. We should choose one canonical builder surface; adding both forms now would create two spellings with unclear precedence.

An ECS `TwoBoneIK` child component would be useful if the policy needed its own identity, live editor manipulation, independent lifetime, or references from other components. A static, saved preset passed to AVC does not need that machinery. The actual runtime `IKChainComponent` remains in place and remains the single solver for each eligible arm.

## Current behavior to preserve

`AvatarControlSystem` resolves upper arm, lower arm, hand, and XR target, then creates one `IKChainComponent` per eligible arm. It currently copies pole directions from AVC and sets `copy_end_rotation: true`. `IKSystem` reads body-local bend-normal exclusion ranges directly from AVC. The VR inspection scene authors pole directions and two left exclusion ranges through AVC methods.

MMS can parse `IKChain.two_bone_ik(...)`, but that path leaves root and middle joint IDs null. It is not presently a usable standalone chain. The new AVC methods must configure the generated chain rather than instantiate another one.

## Table schema and override rules

| Key | Value | Omitted behavior |
| --- | --- | --- |
| `pole_direction` | Three finite numbers, in AVC's body/model-local pole space | Use the legacy `left_arm_pole_direction` / `right_arm_pole_direction`, then the engine default. |
| `copy_end_rotation` | Boolean | Use AVC's current `true` default. |
| `weight` | Finite number from 0 to 1 | Use the generated chain's current `1.0` default. |
| `forbidden_bend_normal_z_degrees` | Array of `[start, end]` degree pairs | Use the corresponding legacy AVC exclusion list, or no exclusions. |

An explicit empty exclusion array, `[]`, clears the legacy list for that side. A nonempty array replaces it as one value; it does not append. Each pair has distinct finite endpoints in `[-180, 180]`. Normalize endpoint order as the current AVC methods do. Split an exclusion that crosses the ±180° seam into two pairs. Reject unknown keys and malformed values with a message naming the side and key. An empty table inherits everything.

One call per side is allowed. Repeating `left_two_bone_ik(...)` or `right_two_bone_ik(...)` should be an authoring error, rather than an order-dependent merge. The same applies to mixing these methods with a future whole-arm-object builder. Legacy individual AVC settings may coexist during migration; each table field takes precedence over its legacy counterpart.

AVC continues to own bone selection through `HumanoidBoneMap`, XR target construction, tracking validity, chain lifetime, and initialization. The tables contain policy only; they do not override joint IDs or target references. If a side has no eligible arm, keep its config on AVC and report why no chain was created.

## Runtime representation and save behavior

Parse the table at the MMS/Rust boundary into a typed per-arm policy or override struct on `AvatarControlComponent`. Preserve field presence separately from field value, especially for `forbidden_bend_normal_z_degrees = []`. Validate the full table before mutating AVC, including nested arrays and both angle endpoints.

At chain creation, compute the effective policy per side and pass the pole, copy-end-rotation setting, and weight into the generated chain. Store the effective exclusion ranges with that chain's two-bone policy so `IKSystem` reads the configuration of the chain it is solving. The body-local frame used for angular evaluation must remain explicit and valid across chain recreation.

`AvatarControlComponent::to_mms_ast` must emit the new method calls and the effective authored tables so saved scenes retain the settings. A serialized scene may contain the resolved tables rather than the original factory call; preserving the import/function source would require a separate source-preservation feature. Runtime joint IDs, target IDs, cached bend normal, and clipping state should not serialize.

The inspector or startup diagnostic should show the effective left and right settings and whether each value came from the table, a legacy AVC method, or a default.

## Implementation and acceptance

1. Add the two builder methods to the MMS runtime schema and component registry, with strict table parsing and side-specific errors.
2. Add typed per-side overrides to AVC and serialize them back as table-valued builder calls.
3. Resolve effective policies when AVC creates its arm chains. Move exclusion-range ownership to the chain policy while keeping current behavior when no table is provided.
4. Move the inspection scene's current pole and exclusion values into table calls. Add Reimu and bisket factory files only after measuring appropriate values for each model.
5. Check partial tables, explicit empty exclusions, malformed/unknown fields, duplicate side calls, save/reload, AVC reinitialization, and that only one chain drives each arm. In VR, confirm both models still track hands and the red clipping readout reflects the actual solved result.

Refactor task: [Consolidate AVC arm IK builders](consolidate-avc-arm-ik-builders.md). Related work: [AVC arm IK inventory](avc-arm-ik-control-inventory-and-mms-semantics.md) and [body-local bend-plane limits](avc-arm-ik-body-local-bend-plane-limits.md).
