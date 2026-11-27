# Separate XR pointer attachment from joint-basis retargeting

Status: implemented for the 0.8 breaking-change window. `RestAttachment(anchor, target)` is the
chosen Phase 1 surface; Phase 2 removes the XR landmark fields, builder methods, compatibility
registration, and shortest-arc fallback.

## Summary

The current `XRHand` avatar-laser configuration combines two independent concerns:

1. correcting an imported target joint's rest basis so a canonical pose can drive it; and
2. choosing a rest-space attachment position and orientation for a pointer laser.

These concerns must be separated before, and then removed in conjunction with, the legacy XR
compatibility-definition path. `JointBasisRetargetingSystem` should remain independent of XR,
controllers, pointers, lasers, and anatomical hand recipes.

## Current coupling

`XRHand.laser_from_avatar_hand(...)`, `laser_from_avatar_finger(...)`, and
`palm_from_avatar_knuckles(...)` currently provide landmark references that are used for multiple
purposes:

- deriving a hand-joint orientation correction;
- locating a fingertip laser mount;
- choosing the pointer's anatomical roll;
- preserving older MMS scenes without an explicit `JointRetargetBasis` component.

The retained retargeting implementation therefore has an XR-specific compatibility adapter that
converts these fields into `RetargetBasisDefinition`. This is a migration convenience, not part of
the desired architecture.

## Target architecture

Pose driving and pointer attachment should be independent pipelines:

```text
tracking or other canonical pose source
                  ↓
          generic pose driver
                  ↓
 JointBasisRetargetingSystem::basis_for(target)
                  ↓
          driven target transform
```

```text
Pointer
   ↓
explicit generic rest-space attachment declaration
   ↓
one-time attachment transform beneath the chosen target
   ↓
laser origin and orientation
```

`JointRetargetBasis` continues to define only the canonical rest frame for a target transform:

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

It must not define pointer attachment position, select a tracking source, or contain XR-specific
behavior.

## Phase 1: introduce generic rest-space attachment

Design and implement a generic way to attach runtime or authored content to an imported transform
using immutable rest data. The API should not mention XR, hands, fingers, or lasers.

The chosen surface keeps the pose-driver transform directly beneath its source:

```mms
XRHand(true, Left, GripAim).laser() {
  T {
    RestAttachment.new("#J_Bip_L_Hand", "#J_Bip_L_Middle3") {
      Pointer.new()
    }
  }
}
```

The attachment contract must specify:

- owning-GLTF and armature/node resolution scope;
- selector and `@uuid:` behavior;
- whether the attachment uses the target origin or an explicit rest-local offset;
- how orientation is chosen independently from position;
- initialization before and after `GltfInitialized`;
- cleanup and GLTF respawn invalidation;
- serialization round-trip behavior;
- whether attachment targets may be ordinary imported transforms as well as skin joints.

PointerSystem should calculate the attachment once from immutable rest data. It must not rediscover
landmarks every frame. Pointer orientation may consume a retained joint basis when explicitly
requested, but attachment position must not be inferred from `JointRetargetBasis`: pointer
placement is outside that component's contract.

## Phase 2: remove XR-owned retargeting configuration

After the generic attachment mechanism is available:

1. Migrate all MMS assets, examples, and tests using `laser_from_avatar_hand`,
   `laser_from_avatar_finger`, or `palm_from_avatar_knuckles`.
2. Author explicit `JointRetargetBasis` components under each relevant `GLTF`, or use the future
   semantic bone-map path where available.
3. Author pointer attachment independently through the Phase 1 mechanism.
4. Remove `JointBasisRetargetingSystem::register_xr_compatibility`.
5. Remove XR landmark-to-`RetargetBasisDefinition` conversion from AVC and PointerSystem.
6. Remove or redefine the legacy `XRHand` laser builder methods so they configure only pointer
   behavior, never target-joint basis correction.
7. Remove `avatar_finger`, `avatar_hand_up`, and `avatar_palm_width` from `XRHand` once no remaining
   attachment API depends on them.

AVC and other pose consumers should receive a resolved target transform ID through their normal
topology/semantic resolution, then query:

```text
basis_for(target)
```

They must not interpret finger selectors or know which authoring source produced the retained
basis.

## Forward-only configuration

The old forward-only XR recipe cannot satisfy a two-axis `JointRetargetBasis`. During this task,
choose and document one of these policies:

- require a complete forward/up basis and migrate or reject forward-only scenes; or
- introduce a separate generic one-axis retargeting mode that is not owned by XR.

Do not retain an XR-specific shortest-arc path merely to avoid making this decision.

## Compatibility and failure policy

Decide explicitly whether this migration is breaking or includes a deprecation window. The final
architecture must have a single source-independent retained basis per target.

Missing, invalid, or conflicting basis definitions must remain visible to consumers. Pointer
attachment fallback must not hide a retargeting error, and retargeting fallback must not infer a
pointer attachment.

## Acceptance criteria

- `JointBasisRetargetingSystem` has no XR-, controller-, pointer-, laser-, hand-, or finger-specific
  registration API.
- `XRHand` contains no landmark fields used to define a target joint's canonical basis.
- A generic pose driver can use the same retained basis for XR and non-XR pose sources.
- Pointer attachment position is configured and retained independently from joint-basis
  correction.
- Existing avatar-pointer examples are migrated to explicit basis plus attachment declarations.
- Authored `JointRetargetBasis` and future semantic bone-map definitions use the same generic
  registration/replacement path.
- No selector discovery, rest-pose calculation, or definition reconciliation occurs every frame.
- Tests cover ordinary imported transforms in addition to skin joints where supported.
- Tests cover initialization ordering, scoped resolution, attachment cleanup, GLTF respawn,
  serialization, missing basis, invalid basis, conflicts, and missing attachment targets.

## Out of scope

- humanoid automapping itself;
- articulated-finger driving;
- IK expansion;
- choosing between controller and hand-tracking pose sources;
- editor UI beyond what is required to author or inspect the new attachment component.
