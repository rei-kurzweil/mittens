# Humanoid bone map, conservative automapping, and MMS presets

Date: 2026-08-06

Status: implemented for `mittens-engine 0.8.0`; final smoke checks remain

## Contract

An avatar has one semantic map owned by its GLTF instance. `HumanoidBoneMap.new()` defaults to
Auto. An explicit `.slot(name, ComponentRef)` or `.absent(name)` always wins over inference;
`.automap_disable()` leaves every other slot unresolved. There is no legacy AVC-name mode.

```mms
HumanoidBoneMap.new()
  .slot("left_hand", "[name='J_Bip_L_Hand']")
  .slot("camera_anchor", "#camera_socket")
  .absent("neck")
  .automap_disable()
```

At most one authored map may belong to a GLTF. Multiple maps produce an invalid retained report.
AVC requests an implicit Auto report when no map is authored. GLTFs without AVC are scanned only
when they contain a map or a report is explicitly requested.

## Slots and reports

`HumanoidSlot` covers the center chain, bilateral shoulder/arm/hand chains, all three rotating
bones of each finger, eyes, legs/feet/toes, and `camera_anchor`. The 0.10 hand-tracking slice
expanded the original palm-basis landmarks to complete explicit finger chains. Authored state is
`Unspecified`, `Reference`, or `Absent`.

The generation-numbered GLTF report distinguishes skin joints, arbitrary transforms, and generated
anchors. It records explicit, convention/name, topology/geometry, symmetry, derived-eye-midpoint,
head-fallback, absent, unresolved, ambiguous, and invalid-explicit outcomes. Head is required for
AVC. Each arm activates independently when upper arm, lower arm, and hand validate; neck and lower
body gaps do not block VR operation.

## Resolver rules

- Anatomical candidates are restricted to the owning GLTF's `armature_joint_transforms`.
- Explicit selectors must resolve uniquely. Only `camera_anchor` may explicitly select an arbitrary
  `Transform` outside the skin-joint set.
- Exact convention-aware tokens are preferred and checked against topology. Helper, twist,
  collider, adjustment, and secondary-motion joints are rejected or penalized. A suffix such as
  `J_Bip_C_Head.001` is not an exact `head` match.
- Ties and topology-only guesses remain unresolved and retain their candidates. Geometry may
  corroborate semantics but may not invent handedness or forward direction.
- Resolution is event-driven at registration, `GltfInitialized`, removal/respawn, and relevant
  topology changes; it is never a per-frame skeleton scan.
- VRM/VRMC metadata and LLM classification are deferred.

## Camera and hand behavior

Camera resolution is: explicit transform; unique central semantic camera/eye anchor; generated
retained eye midpoint below the head; mapped head fallback. A broken explicit camera selector stays
diagnosed while the operational fallback keeps the camera usable. A later matching attachment
causes a new generation and AVC notification.

Map-derived hand bases use middle-proximal to middle-distal as forward and little-proximal to
index-proximal as palm width. An authored `JointRetargetBasis` remains the expert override and wins
for the same hand. `JointRetargetBasis` and `RestAttachment` remain general-purpose components.

## MMS presets and migration

Reusable factories live in `assets/components/humanoid_bone_maps/`: `vroid.mms`, `bisket.mms`, and
`pc-rei.mms`. Bisket and PC-Rei reuse the shared VRoid convention but remain discoverable modules.
Repository AVC examples use implicit Auto and no longer repeat bone-name strings or hand bases.

## Release gate and deferrals

The 0.8 gate covers head, independent arms/hands, hand landmarks/bases, eyes, and camera anchoring,
including two avatar instances without cross-resolution, incomplete/nonhumanoid rigs, respawn, map
removal, late camera attachment, and controller/hand source switching. Lower-body inference is
reported but leg IK is not a blocker.

Full finger driving was deferred from 0.8 and now has an explicit VRoid/OpenXR slice in
[shared hand tracking](hand-tracking-system-and-finger-retargeting.md). Broader finger automapping,
general animation retargeting, VRM metadata, LLM proposals, multi-avatar selection below one AVC,
and the editor mapping panel remain deferred. The editor work is tracked in
[humanoid-bone-mapping-editor.md](humanoid-bone-mapping-editor.md).
