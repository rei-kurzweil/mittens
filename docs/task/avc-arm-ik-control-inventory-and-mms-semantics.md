# AVC arm IK inventory and possible MMS control semantics

Status: design sketch for review. No component or transform-stream API change is implemented here.

## Current ownership

| Concern | Current owner | Author-visible control |
| --- | --- | --- |
| Humanoid bone selection | `HumanoidBoneMap` plus AVC initialization | Map slots, including upper arm, lower arm, and hand. |
| XR input and hand target | `InputXR`, `XRHand`, AVC hand-target setup, and `TransformStreamSystem` | XR components and AVC hand smoothing/grip settings. |
| Body/root orientation | AVC-created transform pipeline and `TransformStreamSystem` | AVC initial yaw, body yaw threshold/rate, and related settings. |
| Arm chain creation | `AvatarControlSystem::try_init_splices` | Implicit per eligible side; AVC supplies the pole hints. |
| Arm solve | `IKSystem` with `IKChainComponent::TwoBoneIK` | AVC pole directions and `ik_debug()`; no direct handle to its generated arm chains in MMS. |
| End hand orientation | `IKSystem` using the hand target rotation | AVC-created chains set `copy_end_rotation: true`. |
| Scene-level diagnostics | The VR inspection example and runtime IK debug visuals | `ik_debug()` and the example's two body-local normal readouts. |

AVC creates one `IKChainComponent` per eligible arm with explicit upper-arm and lower-arm IDs, a hand end effector, a corrected controller target, a body-local pole hint, and an XR pose validity guard. The chain is parented under AVC for lifecycle cleanup. `IKSystem` owns the per-frame solve and emits local joint rotations. Its debug visuals are runtime-only and report the target, pole, bend normal, and solved elbow.

There is a general `IKChain` MMS component today. It accepts `weight`, `target`, and `end_effector`; `IKChain.two_bone_ik(pole, copy_end_rotation)` also parses. However, that constructor currently leaves the required root and mid joint IDs null, and the solver skips such a chain. Thus it is not yet a usable way to author a standalone two-bone arm solve. `IKChain` and transform pipelines are distinct systems: the pipeline prepares target transforms, while IK reads them after FK and solves the bones.

## Semantics worth exposing

The useful abstraction is an explicit **arm chain handle** with named ownership and addressable inputs. A scene should be able to opt into AVC's automatic wiring, inspect the resulting left/right chains, and override selected policies without creating a second solver for the same bones. Alternatively, a scene could disable AVC arm IK for one side and author a complete chain itself. Both paths need a clear single-writer rule for upper arm, lower arm, and hand rotations.

Possible MMS surface, for discussion only:

```mms
let avatar = AVC {
    // Existing setup omitted.
    left_arm_pole_direction([1, -1.5, 1])
}

// Proposed read-only handles after AVC has resolved the rig:
let left_arm = avatar.arm_ik("left")
let right_arm = avatar.arm_ik("right")

// Proposed control of inputs/policy, not a second IKChain component:
left_arm.pole_source(body_local_pole_operator)
left_arm.bend_normal_z_region(min_angle, max_angle)
left_arm.weight(1.0)
```

An alternative declarative form could attach a child configuration component to AVC, resolved with the bone map after GLTF initialization. The exact syntax is open. The important semantic points are:

- **Identity:** stable left/right chain handles resolve after rig import. A missing mapped bone or controller produces an explicit unavailable state.
- **Ownership:** changing a generated chain adjusts AVC's one active solve; adding a manual chain does not silently solve the same joints twice.
- **Spaces and units:** pole and bend-region inputs declare body-local or world space. Angles use radians; UI readouts may show degrees.
- **Lifecycle:** targets can come and go with XR tracking. The chain keeps an explicit validity/weight behavior, and stale component handles do not point at a replacement rig.
- **Ordering:** transform-stream operators may generate or filter a target pose or a pole-direction signal before the IK pass. IK consumes the resolved value after FK. The solver's output rotations are not fed back into the same transform stream within one tick.
- **Serialization:** an MMS-authored chain or policy keeps component/query references in saved output. Runtime AVC-generated IDs remain implementation details.

## Transform operator relationship

A transform stream is a good place for target filtering, grip-to-hand offset, body-relative pole targets, and perhaps an operator that computes a preferred elbow direction from tracked motion. It should expose a typed pose/vector output that an IK chain can consume. Treating the IK solve itself as an ordinary transform operator would need an explicit multi-joint output and update order, because it writes three bones rather than one transform. The design should not hide that difference behind a generic single-transform stage.

One possible end state is a pipeline feeding the chain, not replacing it:

```text
XR hand pose → smoothing/remap → named hand target ─┐
body pose → pole operator → preferred elbow vector ─┼→ TwoBoneIK → upper/lower/hand rotations
body pose → optional bend-region policy ────────────┘
```

## Open design decisions

1. Should manual control mean configuring AVC-generated chains, authoring standalone `IKChain`s, or both?
2. How should an MMS-authored two-bone chain specify root and mid joints: explicit component references, bone-map slots, or a resolved rig path?
3. Does a pole operator output a point, a direction, or a full plane preference? Which coordinate space is attached to that value?
4. Should bend limits belong to AVC, the `TwoBoneIK` solver, or a reusable policy component attached to the chain?
5. How should per-side tracking loss, explicit weight animation, and fallback pose interact?
6. What is the observable/debug surface for raw inputs, constrained bend plane, chosen elbow, and target error?

## Follow-up sequence

Resolve the live bend-plane investigation first, then choose the smallest semantic surface needed for a measured constraint. A later implementation can make manual root/mid resolution work and expose AVC chain handles without changing the solver math in the same step.

Related proposal: [Body-local bend-plane limits](avc-arm-ik-body-local-bend-plane-limits.md).
