# XRHand without its driven Transform creates a stationary laser on the scene root

Date: 2026-10-22
Status: corrected; user confirmed usable pointer interaction in the shared-materials VR scene.
Reported fixture: [shared materials XR](../../examples/shared-materials-xr.mms).
Related fixture: [shading models XR](../../examples/shading-models-xr.mms).

## Report and verified cause

The shared-materials scene rendered in VR, but its controller pointers were
unusable. At least one stationary laser appeared near floor height. No avatar
was present, suggesting an accidental avatar dependency.

Source inspection identified an authoring error and a misleading renderer
fallback. Both affected scenes used:

```mms
XRHand.new(true, "Left", "GripAim").laser() { Pointer {} }
```

OpenXR applies each controller pose to the first direct Transform child of
XRHand. With no such child, tracking has no transform to update. The laser
helper searched upward from Pointer and selected the outer scene Transform,
mounting the visible beam at the stationary scene origin. Both controllers
selected that same driver, so laser deduplication could leave just one beam.
The malformed pointer
also inherited that outer transform rather than a tracked controller pose.

This is not an avatar requirement. A complete controller-space pointer is:

```mms
XRHand.new(true, "Left", "GripAim").laser() {
    T { Pointer {} }
}
```

AVC, an avatar GLTF, humanoid maps, IK and RestAttachment are unnecessary for
ordinary tracked controller pointers. RestAttachment is an optional avatar-space
origin adjustment and retains its separate avatar requirements.

## Fix and evidence

- Both XR material fixtures now put their Pointer beneath an explicit tracked T.
- XRHand registration diagnoses a missing direct Transform with the expected
  authoring shape. No character prefab is needed to fix this fixture.
- The laser helper requires that driven Transform. When a Pointer is present,
  its chosen laser origin must be in that transform's subtree. It cannot borrow
  a scene/head ancestor or an untracked sibling Transform as the laser origin.
- A malformed-topology regression checks both a missing Transform and a Pointer
  beside an unused Transform; neither creates a misleading stationary beam.
- The two authored-scene regressions load the real MMS rigs with hardware
  session startup disabled. They use the production head/controller pose
  application path, vary head and controller samples over three frames, and
  verify both laser origins, pointer ray origins/directions and per-hand trigger
  routing without AVC. Existing avatar/rest-attachment behavior is checked too.

Headset follow-up: after the correction, the user confirmed that the
shared-materials test is usable and that independent shading, detaching sources
from the shared objects, and reattaching both work. This closes the reported
pointer blocker in that scene. A detailed per-hand beam/stereo checklist was
not separately reported. Character prefab work remains deferred.

Validation: 25 focused OpenXR, pointer, rest-attachment and material-control tests
pass, including both authored-scene pose/ray tests and both avatar fingertip
mounts. Formatting and whitespace checks pass. The release application was rebuilt.
