# Avatar camera anchoring

Date: 2026-08-06
Updated: 2026-10-28

Avatar cameras are attached through the owning GLTF's `HumanoidBoneMapReport`; AVC has no camera
or head bone-name fields.

Resolution order is:

1. an explicit `camera_anchor` selector resolving uniquely to any `Transform`;
2. one semantically named central camera/eye anchor validated below the mapped head;
3. a retained generated transform at the mapped eye midpoint below the head;
4. the mapped head, with AVC's authored eye-height offset available as an expert fallback.

A missing or ambiguous explicit camera selector remains in report diagnostics even when a fallback
keeps the camera operational. Relevant attachment or GLTF lifecycle events produce a new map
generation; AVC switches to the new retained target without polling the skeleton each frame.

Direct `Camera3DComponent` and `CameraXRComponent` children of AVC, including camera components
wrapped by one transform carrying an authored eye offset, are reparented beneath the operational
anchor during initialization. Model-root height calibration uses immutable GLTF rest poses, never
an animation-mutated live pose. Head/HMD offset calibration composes the full
head-to-operational-anchor rest path with the authored wrapper, then accounts
for the displaced head's retained rest rotation/scale. Using only the wrapper
offset misaligns the anatomy with the measured XR view when the anchor differs
from the head pivot.

XR view matrices use the owning InputXR's driven head world matrix and head-relative
OpenXR eye poses from the same prepared frame. This lets InputXR floor correction
reach the view and tracked hands together. The mapped avatar anchor aligns anatomy
and camera topology; its wrapper is not an additional rendered headset offset.
See [XR settings and floor correction](xr-settings.md).

XR projection uses a 1 cm near plane and a 100 m far plane. The former 10 cm
near plane clipped nearby avatar torso surfaces when looking down even with
connected neck geometry; projection must preserve these surfaces without
moving the measured headset view.

## Desktop first-person authoring

Input and InputXR both drive a spliced head through AVC's shared head/neck/body solve. Declare
C3D directly under AVC or inside a single direct Transform wrapper so the eye offset is captured
before the camera is attached to its mapped anchor. Two nested wrappers or a later manual
GLTFInitialized head attachment do not participate in that discovery contract.

```mms
AVC {
    T { avatar_gltf }
    T.position(0.0, 0.08, 0.12).rotation(0.0, 3.14159265, 0.0) {
        C3D { near(0.01) Pointer {} }
    }
}
```

Use a 1 cm near plane for this close first-person C3D. Its general default is still 10 cm;
Sakura's actual skinned outer shirt surface at about 8 cm was clipped by that default even with
correct neck bending and head translation. Camera geometry and clipping must both be validated.

The corrected Sakura desktop scene has CPU-skinned front-face/near-plane checks for downward
pitch and combined roll, plus inspected Vulkan captures using its authored C3D matrices. User
smoke feedback reports improved body visibility. See the
[reproduction and results](../bugs/sakura-desktop-first-person-neck-bend-regression.md).
