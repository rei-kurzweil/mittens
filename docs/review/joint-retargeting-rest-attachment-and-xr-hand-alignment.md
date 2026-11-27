# Joint retargeting, rest attachment, and XR hand alignment review

Date: 2026-08-05

Status: implemented and verified in `vtuber-mirror-example`; follow-up design work identified

## Purpose

This review records the 0.8 hand-alignment work after replacing the legacy XR-owned landmark
configuration. It explains the two new authored components, the retained/runtime machinery beneath
them, the regressions found during migration, why the final correction works, and which parts
should become more generic next.

The most important architectural result is that XR no longer defines an avatar's rest basis. XR
provides a live pose. The imported avatar declares how its own joints relate to one canonical pose
frame, and all compatible pose sources drive that same frame.

## Outcome

The current path has three independent responsibilities:

| Responsibility | Declaration or source | Runtime owner |
| --- | --- | --- |
| Describe an imported joint's canonical rest basis | `JointRetargetBasis` | `JointBasisRetargetingSystem` |
| Describe an immutable offset from one imported node to another | `RestAttachment` | currently resolved and materialized by `PointerSystem` |
| Supply a live canonical pose | controller Aim/GripAim or synthesized hand-tracking landmarks | `OpenXRSystem` and AVC |

`vtuber-mirror-example` has been verified in-headset after the final transform-host fix. Controller
Aim now drives the avatar hand without the previous approximately 90-degree upward wrist bend.

## The components involved

### `JointRetargetBasis`

[`JointRetargetBasisComponent`](../../src/engine/ecs/component/joint_retarget_basis.rs) is the new
authored definition of one canonical two-axis frame for one imported armature joint.

```mms
GLTF.new("../avatar.glb") {
  JointRetargetBasis.new(
    "#J_Bip_L_Hand",
    "#J_Bip_L_Middle1",
    "#J_Bip_L_Middle3",
    "#J_Bip_L_Little1",
    "#J_Bip_L_Index1"
  )
}
```

Its five references mean:

- target: the transform whose authored local basis must be normalized;
- forward: `Middle1 -> Middle3`;
- up: `Little1 -> Index1`.

The retained system expresses both directions in target-rest-local space, projects up away from
forward, and constructs a right-handed orthonormal frame. By contract:

- canonical `-Z` maps to anatomical forward;
- canonical `+Y` maps to little-to-index across the knuckles;
- canonical `+X` is the remaining right-handed axis.

The system retains both conversion directions:

```text
canonical_to_target_rest
target_rest_to_canonical
```

AVC uses `target_rest_to_canonical` to make an imported hand follow a canonical pose source.
PointerSystem uses `canonical_to_target_rest` to orient avatar-relative pointer content. Both query
the same target-keyed retained entry rather than independently rediscovering or recalculating
landmarks; the retained generation is available for attachment invalidation work.

This component is deliberately joint-specific. It resolves only within the owning GLTF's
`armature_joint_transforms`, diagnoses duplicate definitions, and has retained lifecycle status.
That narrowness is useful: ordinary scene transforms do not automatically acquire skeletal
retargeting semantics.

See the normative component and retained-system specifications:

- [JointRetargetBasis component](../spec/joint-retarget-basis-component.md)
- [JointBasisRetargetingSystem](../spec/joint-basis-retargeting-system.md)

### `RestAttachment`

[`RestAttachmentComponent`](../../src/engine/ecs/component/rest_attachment.rs) is the second new
authored component. It describes an immutable rest-space relationship between two imported nodes:

```mms
XRHand(true, Left, GripAim).laser() {
  T {
    RestAttachment.new("#J_Bip_L_Hand", "#J_Bip_L_Middle3") {
      Pointer {}
    }
  }
}
```

The two references are:

- anchor: the imported node whose rest-local space is the reference frame;
- target: a descendant imported node whose rest position supplies the attachment offset.

This separates fingertip placement from hand orientation. The fingertip is not part of the hand
basis definition, and the hand basis does not imply that a pointer should exist.

The component itself contains no XR, hand, pointer, or laser fields. The resolver in
[`rest_attachment.rs`](../../src/engine/ecs/system/rest_attachment.rs) is also expressed in terms
of an owning GLTF and two `ComponentRef` values. However, the current behavior is only partially
generic:

- only PointerSystem discovers and materializes the declaration;
- PointerSystem supplies the owning avatar GLTF through AVC topology;
- PointerSystem creates the runtime mount and reparents the pointer;
- the resolver returns a full matrix, but PointerSystem currently consumes its translation and
  gets orientation independently from `JointBasisRetargetingSystem`;
- target must be a descendant of anchor rather than merely another node in the same imported rest
  space.

Therefore `RestAttachment` is currently a generic data description used by one specialized
consumer, not yet a general attachment behavior available to arbitrary scene content.

See [the RestAttachment specification](../spec/rest-attachment-component.md).

### `BoneRestPoseComponent` is supporting infrastructure, not one of the two new declarations

[`BoneRestPoseComponent`](../../src/engine/ecs/component/bone_rest_pose.rs) predates this slice but
is essential to it. GLTFSystem creates one immutable local-TRS sidecar for every spawned imported
node before animation, IK, or AVC can mutate the live transform.

The current name is narrower than the behavior. It is attached to every imported node, including
ordinary transform-only and mesh nodes, not only skin joints. Its data is also a local transform
snapshot rather than a complete skeletal "pose."

A future breaking rename to `ImportedRestTransformComponent` or `RestLocalTransformComponent`
would describe the actual contract more accurately. This is the clearest genericity improvement
identified by the review. It should remain runtime-generated and immutable; renaming it should not
turn it into author-facing mutable configuration.

## Runtime flow

The controller path is now:

```text
OpenXR Grip position + Aim orientation
                    |
                    v
       raw tracked transform under XRHand
                    |
                    v
  target_rest_to_canonical correction from retained basis
                    |
                    v
      corrected visual hand target used by arm IK
                    |
                    v
          imported avatar hand joint
```

The hand-tracking fallback uses the same downstream path. OpenXR joint positions synthesize a
canonical frame from `MIDDLE_PROXIMAL -> MIDDLE_DISTAL` and
`LITTLE_PROXIMAL -> INDEX_PROXIMAL`. AVC then applies the same retained avatar correction. Runtime
wrist-joint quaternion axes are no longer assumed to be equivalent to controller Aim axes.

The avatar-relative pointer path is separate:

```text
RestAttachment(anchor, fingertip)
              |
              v
one-time anchor-to-fingertip rest translation
              +
canonical_to_target_rest orientation from retained basis
              |
              v
runtime mount beneath the corrected hand target
              |
              v
            Pointer
```

## Why the final fix worked

The basis geometry was correct, but the retained entry was not published for real imports.

GLTFSystem stores the authored `GLTF` declaration beneath a transform host, then spawns imported
nodes beneath that host. The imported nodes are siblings of the `GLTF` component, not descendants
of it. The first retained implementation correctly used the `GLTF` component as ownership and
resolution scope, but incorrectly used it as the ancestor boundary when accumulating immutable
rest matrices.

That traversal failed with `joint is not beneath its owning GLTF`. AVC saw an invalid definition
and therefore had no correction to apply. On Bisket, anatomical finger-forward is approximately
hand-local `+Y`. Driving that uncorrected transform with canonical Aim `-Z` made the fingers point
up, producing the observed 90-degree wrist bend.

The fix keeps the two concepts distinct:

- ownership and selector scope remain the authored `GLTF` instance;
- rest-matrix accumulation starts at the GLTF's transform host for real imports;
- hand-built armatures whose joints are genuinely beneath `GLTF` retain that direct topology.

A real `assets/models/bisket.glb` lifecycle test now proves that the authored definition moves
from `WaitingForGltf` to `Ready` and maps its anatomical forward/up directions to canonical
`-Z`/`+Y`.

## Migration regressions and lessons

### The origin-seeking hands

The first example migration inserted `RestAttachment` between `XRHand` and its driven transform.
OpenXR and AVC require the tracked `TransformComponent` to be the direct child of `XRHand`. Once
that topology was broken, AVC could create or select an unattached transform at the origin, so IK
made the hands behave like compass needles toward world origin.

The corrected topology is:

```text
XRHand
  T                         direct tracked transform
    RestAttachment
      Pointer
```

AVC now refuses to create an origin target when that direct tracked transform is missing. This
turns a dangerous visual fallback into an observable initialization/configuration failure.

### A correct cache can still be unavailable

The 90-degree regression demonstrated that geometry tests alone were insufficient. Synthetic
fixtures placed joints directly beneath `GLTF`, so they did not reproduce GLTFSystem's real spawn
topology. The retained basis math passed while production entries became `Invalid`.

Lifecycle tests involving a real imported asset are therefore required for systems whose
contracts span authored declarations and spawned runtime nodes.

### Pose-source frames must be explicit

Controller Aim already supplies a canonical `-Z`-forward frame. Raw wrist/palm joint orientation
is a different semantic input and was previously passed through with identity correction. The
hand-tracking fallback now constructs the same canonical frame geometrically before AVC applies
the avatar correction.

The general rule is:

```text
source-specific acquisition -> canonical pose frame -> target-specific retained conversion
```

Source-specific behavior belongs before the canonical boundary. Imported-avatar behavior belongs
after it.

## Generality review and recommendations

### Keep `JointRetargetBasis` focused for now

The component has a clear invariant: one imported armature joint, two anatomical directions, one
canonical basis. Generalizing it to arbitrary transforms now would weaken its scoped resolution
and conflict semantics without a demonstrated consumer.

`HumanoidBoneMap` should resolve semantic slots and create the same internal
`RetargetBasisDefinition`; it should not add XR behavior or change basis geometry.

### Rename the rest-pose sidecar when the next breaking rename is practical

Recommended direction:

```text
BoneRestPoseComponent -> ImportedRestTransformComponent
```

Before renaming, audit all consumers and decide whether the type means:

- immutable local TRS captured specifically at import time; or
- a more general immutable authored/rest-local transform that non-GLTF producers may also stamp.

The former is the current implemented contract and is the safer name.

The current MMS serialization surface also deserves review: the runtime sidecar stores
translation, rotation, and scale, while its `to_mms_ast` representation only emits translation.
A runtime-generated non-authored component may be better made explicitly non-serializable than
given a lossy author-facing representation.

### Generalize `RestAttachment` behavior only with a second real consumer

There are two coherent futures:

1. Keep it as a small descriptor used by specialized systems. In that case, document that
   consumers own GLTF selection, lifecycle, mount creation, and orientation policy. A name such as
   `ImportedRestOffset` may be more precise than `RestAttachment` because the component does not
   itself attach anything.
2. Promote it to a true generic attachment facility. That requires a retained system that owns
   resolution/status, an explicit way to identify the imported instance when the declaration
   lives outside its subtree, generic child mounting, cleanup/respawn handling, and a decision
   between translation-only and full relative-transform modes.

The review recommends not expanding the public API speculatively. First extract and reuse a
generic imported-rest transform query/result beneath the current pointer path. Promote or rename
the component when a second consumer—such as a weapon socket, wearable, camera marker, or effect
anchor—proves the required semantics.

If sibling-node relationships become useful, the generic calculation should be:

```text
inverse(anchor_rest_model) * target_rest_model
```

rather than requiring target to be an actual descendant of anchor. That is a broader contract and
should arrive with tests for multiple imported instances, singular transforms, cleanup, and GLTF
respawn.

## Verification completed

Automated verification for the final implementation included:

- retained joint-basis tests, including real Bisket GLTF initialization and geometry;
- AVC source-switch and corrected-target topology tests;
- OpenXR GripAim composition and synthesized hand-basis tests;
- PointerSystem corrected mount tests;
- RestAttachment resolver and MMS round-trip tests;
- `cargo check --all-targets`.

Runtime verification completed in `vtuber-mirror-example`: controller-driven hands follow Aim
without the previous fixed 90-degree upward offset.

## Recommended next work

1. Verify both hands in `bisket-vr-demo`, including Aim roll and source switching between
   controllers and optical hand tracking.
2. Add runtime diagnostics that report retained basis status and selected rest-space root beside
   AVC's active pose source.
3. Add a real-import RestAttachment test so pointer placement is covered against GLTFSystem's
   transform-host topology, not only hand-built fixtures.
4. Decide and execute the `BoneRestPoseComponent` rename/non-serialization cleanup during the 0.8
   breaking window.
5. Prototype a second non-pointer rest attachment consumer before deciding whether
   `RestAttachment` becomes a retained generic system or is renamed to a descriptor.
6. Continue the existing hand-tracking quality work for temporal landmark filtering and forearm
   twist distribution. Those are downstream quality concerns, not reasons to reintroduce
   XR-specific avatar basis definitions.

## Related documents

- [JointRetargetBasis component](../spec/joint-retarget-basis-component.md)
- [JointBasisRetargetingSystem](../spec/joint-basis-retargeting-system.md)
- [RestAttachment component](../spec/rest-attachment-component.md)
- [Separate XR pointer attachment from joint retargeting](../task/separate-xr-pointer-attachment-from-joint-retargeting.md)
- [Imported humanoid pose-basis detection and conversion](imported-humanoid-pose-basis-detection-and-conversion.md)
- [XR hand tracking wrist kink and jitter](../bugs/xr-hand-tracking-wrist-kink-and-jitter.md)
- [Humanoid bone mapping and avatar slot resolution](current-humanoid-bone-mapping-and-avatar-slot-resolution.md)
