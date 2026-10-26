# Grabbable: optional hand placement

Date: 2026-10-26
Status: implemented; headset feel/grip placement pending manual acceptance.

```mms
T.position(0.0, 1.0, -1.0) {
    Grabbable.to_hand(true) {}
    // Author the held model relative to this Transform's grip frame.
    T.rotation(0.0, 3.14159265, 0.0) {
        GLTF.new("assets/models/sakura-machine-pistol.glb")
    }
}
```

`Grabbable.to_hand(true)` is an opt-in constructor; `.to_hand(true/false)` is also
available as a builder on the existing on/off/parent modes. Default is false.
Serialization preserves the flag with the existing mode.

For XR controller/hand pointers, the moved Transform's local translation eases
to zero in the resolved grip frame, and its rotation eases
to identity. This uses the existing exponential levitation speed (12 per second).
The initial reparent preserves world pose; there is no instantaneous jump. Scale
is preserved. Bounds-based surface clearance and the initial sideways offset do
not displace the held grip in this mode. AVC supplies a grip Transform beneath
its solved hand bone, with the retained hand basis correction inverted into the
canonical grip orientation. Raw tracking continues to drive IK independently;
the pointer ray stays in its original topology. Overextending tracking therefore
keeps the held object at the reachable avatar hand.

`Pointer.grab_attachment("[name='custom_grip']")` overrides this automatic frame
for to_hand objects. It references a Transform; author its offset and orientation
for a custom palm location. Queries resolve within the owning AVC subtree;
explicit world-root and UUID selectors also work. Without a resolved avatar or
explicit grip, the pointer/controller frame remains the fallback. The explicit
reference serializes; AVC-generated handles and grip nodes do not.

Position and orient model children relative to that moved Transform to define
its held grip pose. This does not automatically infer a handle from mesh geometry.
The Sakura pistol points down local -Z while upright in the hand. Its parent grip
Transform supplies the laying-down rotation for the workbench pose. The pistol
was moved 24 cm toward the table's front edge; its resting orientation is retained.
Its model is offset 5 cm toward local +Z (back toward the wrist). The grip root
compensates for this offset in the bench pose so the resting model does not move.

Camera/desktop pointers retain their normal surface-clearance behavior even for
flagged objects, so a camera grab does not move an object into the viewer's head.
Unflagged hand grabs also retain existing clearance/offset/rotation behavior.
The flag is resolved from the marker selected by the actual hit, including a
Grabbable.parent() handle whose moved Transform is its parent. Existing ownership
and release behavior stays in place: release restores the surviving original
parent while preserving the held world pose.

Validation covers real GrabStart/GrabEnd delivery, gradual translation and
rotation in a rotated hand frame, preserved scale, parent-mode handles, release
pose preservation, desktop fallback, default clearance, flag serialization and
Sakura's actual resting model transform. The Vulkan scene check also checks that
imported pistol primitives resolve to the authored grabbable grip root.

`Grabbable.hand_offset([x, y, z])` adds a local translation in the resolved grip
frame. Sakura uses `[0, 0, -0.105]` to place the pistol 10.5 cm forward along its
aiming axis while retaining the prior authored model offset.

Focused grab, serialization and scene checks pass, as does the release build.
The broader grabbable filter also selects xr_grab_demo's stale SpringBone count
assertion (expects 23, current fixture produces 21). The existing shared preset
already disables both bust chains; that fixture expectation is separate from this
flag change and is left unchanged here.

Implemented follow-up: automatic solved-hand grip attachment and an explicit
Pointer override. Tests cover raw-controller overextension, override priority,
AVC basis correction without moving tracking/pointer topology, idempotent setup,
and RuntimeSpec authoring alongside other pointer options without legacy fallback.
