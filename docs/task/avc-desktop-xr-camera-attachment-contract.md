# AVC desktop/XR camera attachment contract

## Audit result

There is no intended authoring discrepancy between desktop `Camera3D` (`C3D`)
and XR `CameraXR` (`CXR`) when either is attached to an avatar through
`AvatarControl` (`AVC`). Both camera types are discovered as an AVC direct
child, or inside a single direct `Transform` wrapper, and are reparented below
the resolved humanoid camera anchor (normally the head).

The supported declarative form is therefore the same for desktop and XR:

```mms
AVC {
    T { avatar_gltf }
    T.position(0.0, 0.08, 0.06) {
        C3D { Pointer {} } // desktop
        // CXR { Pointer {} } // XR, in the equivalent scene
    }
}
```

`C3D` looks down local `-Z`. During AVC splice initialization, a desktop camera
path receives the required half-turn so its forward direction matches the
avatar's camera/head convention. An explicit half-turn on the wrapper is also
valid and makes the authored first-person orientation clear.

## Evidence

- `AvatarControlSystem::try_init_splices` checks every AVC direct child for
  `Camera3DComponent` and `CameraXRComponent`, including one-transform
  wrappers, before resolving eye offset and camera anchoring.
- Its camera reparenting branch handles both component types; only the desktop
  path adds the `π` yaw mount correction.
- The pre-existing `secondary-motion-desktop.mms` example manually attaches a
  reparentable camera rig to a GLTF head to support switching between fixed and
  first-person views. That is useful for its view-toggle feature, but it is not
  required for ordinary AVC first-person authoring.

## Regression coverage and follow-up

`examples/mittens-corp-agc-desktop.mms` uses the declarative one-transform
`C3D` path. Its example evaluation test confirms the desktop input, C3D, AVC,
and settings-only editor UI all materialize together.

If a camera fails to follow after this contract is used, investigate readiness
and topology instead of treating it as an XR/desktop feature gap:

1. Verify the C3D/CXR path is a direct AVC child (at most one wrapper
   transform), rather than nested under the GLTF model transform.
2. Verify the humanoid map resolves `CameraAnchor` or `Head` after GLTF import.
3. Verify exactly one enabled active desktop camera is selected.
4. Add a runtime transform-follow regression only if one of those contracts is
   satisfied and C3D/CXR behavior still diverges.
