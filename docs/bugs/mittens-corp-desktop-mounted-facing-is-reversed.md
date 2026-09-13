# `mittens-corp-desktop` mounted camera faces toward instead of away from the mount point

Status: open, reproduced by user on 2026-09-12.

## Observed behavior

In `examples/mittens-corp-desktop.mms`, entering the car successfully applies
the vehicle mount point's yaw to the Rider/camera, but the resulting camera
faces toward the mount/vehicle when it should face away in the intended seated
view direction.

Position attachment and yaw transfer both commit.  This is not a missing-yaw
or stale-pre-mount-yaw report.

## Expected behavior

On a successful desktop Rider-to-Mountable transition:

- the rider is placed at the vehicle's authored mount point;
- the destination mount point is authored with its horizontal forward in the
  desired seated viewing direction;
- the effective desktop camera adopts that authored horizontal heading;
- the result is independent of the camera's pre-mount yaw;
- pitch behavior remains explicitly defined and independent from the yaw snap;
- repeated mount/dismount cycles produce the same facing result.

The desktop requirement is not automatically the XR requirement.  XR must
continue to distinguish locomotion/body yaw from the user's live tracked head
orientation; fixing desktop virtual look must not forcibly overwrite HMD pose.

## Current fixture

The desktop Rider references:

- movement root: `bisket_desktop_locomotion_root`;
- rider-side mount point: `bisket_first_person_camera_slot`;
- pedestrian input: `bisket_desktop_input`.

The car's `Mountable` references:

- eligibility zone: `car_entry_zone`;
- destination mount point: `left_display_car_desktop_mount`;
- dismount point: `car_dismount`.

The destination point is an ordinary authored Transform today.  Its meaning
comes from `Mountable.mount_anchor(...)`; there is no first-class mount-point
component yet.

## Relevant implementation evidence

`AttachmentSystem::horizontal_anchor_alignment` extracts yaw from the
destination mount transform and applies that yaw directly to the Rider's outer
movement root.  It uses the Rider-side point's translation offset but does not
remove that point's relative yaw when solving the root pose.

That matters in this desktop fixture:

- `Camera3D` views along local `-Z`;
- OpenXR view poses also use local `-Z`, so the renderer backends agree;
- Bisket's authored head-facing direction is local `+Z`;
- `bisket_first_person_camera_slot` therefore carries an authored π yaw between
  the head and desktop camera.

The current mount helper assumes an identity-oriented Rider-side frame, so the
desktop half-turn survives on top of the copied destination yaw.  This is a
source/destination endpoint-frame alignment bug, not evidence that the
destination mount needs an opposite-facing flag or compensating rotation.

The XR fixture appearing correct is explained by an important path asymmetry,
not by a different OpenXR forward convention:

- XR selects `bisket_rider_cxr_anchor`, which has no authored yaw; desktop
  selects the π-rotated `bisket_first_person_camera_slot`;
- the named XR and desktop car-side targets are distinct nodes, although both
  currently have the same local position and identity rotation;
- for a `CameraXR` below `InputXR`, rendering composes the locomotion rig origin
  with the live OpenXR eye pose instead of using the CXR node's inherited
  avatar/head orientation as the eye basis;
- desktop `Camera3D` does inherit the camera slot's π correction.

Thus the existing root-yaw assignment happens to produce the intended XR view,
while desktop reveals that the selected Rider-side frame was never included in
the alignment solve.

The desktop camera is below additional authored/runtime transforms:

```text
bisket_desktop_locomotion_root
  Input
    bisket_desktop_driver            // FPS/mouse yaw is retained here
      AVC / imported Bisket head
        bisket_first_person_camera_slot
          bisket_desktop_camera_rig
            Camera3D
```

`InputSystem` also retains FPS yaw/pitch/roll state keyed by the controlled
transform.  Correcting the authored base heading must keep that state coherent
so the first subsequent mouse or arrow-look update does not jump back.  That
continuity concern is secondary to the incorrect effective facing.

## Mount-point terminology

Use **mount point** for the specific authored attachment endpoint whose
position and orientation participate in alignment.  A mount point is a kind
of socket in the broader terminology, but it is more precise for
Rider-to-Mountable placement.

Attachment valence belongs to mount points/endpoint roles, not to eligibility
zones.  A zone answers whether an attempted relation is allowed in a spatial
region.  Valence answers endpoint compatibility and direction; it does not
replace either point's authored coordinate frame.  The edge selects alignment
and follow channels, while the endpoint Transforms encode orientation.

For the current car edge:

- the rider-side mount point has neutral/`0` valence;
- the vehicle destination mount point has positive/`+1` valence;
- the entry zone is referenced by that mounting policy but does not itself
  need an attachment valence.

## Investigation plan

1. Change horizontal alignment to solve the Rider root from both endpoint
   frames, including the Rider-side point's relative yaw.  Do not add a hidden
   axis negation or compensate by misauthoring the destination point.
2. Add a focused desktop test that mounts from several initial camera yaws and
   compares projected camera forward with the destination mount-point forward.
3. Define a system boundary for synchronizing desktop look state when an
   attachment alignment intentionally changes facing.  Do not reach into an
   unrelated system's private cache.
4. Keep XR coverage proving that mount alignment changes locomotion/root yaw
   without destroying live tracked head orientation.

## Acceptance criteria

- Mounting from at least three different initial desktop yaws produces a camera
  horizontal forward aligned with the correctly authored mount-point forward
  within a small tolerance.
- The rider-side and destination mount-point orientation offsets are honored;
  the test includes the desktop camera slot's non-identity π yaw.
- The first mouse-look update after mounting continues smoothly from the
  snapped orientation and does not jump back to cached pre-mount yaw.
- Mounting does not introduce pitch or roll into the pedestrian locomotion
  root.
- Existing XR mount behavior and tracked-head orientation remain intact.
- Dismount/remount and vehicle motion preserve the authored mount-frame
  contract.

## Related work

- [Desktop occupancy mount pose and input-authority first slice](../task/desktop-occupancy-mount-pose-and-input-authority-first-slice.md)
- [Vehicle mounting disables desktop look together with locomotion](vehicle-mount-disables-desktop-look-with-locomotion.md)
- [Attachment valence and Grabbable unification](../task/attachment-valence-and-grabbable-unification.md)
- [Rider + Mountable attachment-system first slice](../task/rider-mountable-attachment-system-first-slice.md)
- [Default desktop Input arrow-key camera look](../task/desktop-input-default-arrow-camera-look.md)
