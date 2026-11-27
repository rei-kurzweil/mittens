# `RestAttachment`

`RestAttachment` declares an immutable imported rest-space offset independently from pose
retargeting:

```mms
XRHand(true, Left, GripAim).laser() {
  T {
    RestAttachment.new(
      "[name='J_Bip_L_Hand']",
      "[name='J_Bip_L_Middle3']"
    ) {
      Pointer {}
    }
  }
}
```

The driven `T` must remain the direct child of `XRHand`; OpenXR and AVC use that topology as the
tracked pose target. `RestAttachment` wraps pointer content beneath the driven transform and must
not be inserted between `XRHand` and that transform.

The first reference is the rest-space anchor and the second is the attachment target. Both accept
MMQ selectors or `@uuid:` references and round-trip in the same two-argument expression.

## Resolution contract

The consumer supplies the owning imported GLTF instance through its normal topology. Both
references must resolve exactly once inside that GLTF's `spawned_node_transforms`; GUIDs outside
the instance and missing or ambiguous selectors are invalid. This permits ordinary imported
transforms as well as skin joints.

The retained result is the target's immutable rest model expressed in anchor-rest-local space.
The target must be a descendant of the anchor. Resolution before GLTF nodes exist waits for
initialization; consumers calculate and publish their runtime attachment once, rather than
rediscovering nodes every frame.

`RestAttachment` does not select a pose source or prescribe orientation. PointerSystem uses the
offset only for laser origin. When its anchor is an AVC hand target, PointerSystem independently
requires `JointBasisRetargetingSystem::basis_for(anchor)` for orientation and uses that basis's
generation. Missing, invalid, or conflicting bases prevent the avatar attachment; there is no
controller-space fallback that hides the error.

## Boundaries

The component contains no XR, controller, hand, finger, laser, humanoid-slot, IK, or basis
geometry. Other consumers may use the same anchor-to-target rest transform without involving AVC
or PointerSystem.
