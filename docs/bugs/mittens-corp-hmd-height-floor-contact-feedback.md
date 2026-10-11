# HMD height is cancelled by pedestrian floor-contact correction

## Status

Investigating. Reported on pre-cleanup main `4796e0e9`; attachment topology
cleanup `4f6b2915` has since been fast-forwarded into main. This is not a
regression introduced by that cleanup.

## Observed behavior

In `examples/mittens-corp.mms`, physical HMD movement along Y feels constrained
before entering a vehicle. Mounting frees that movement; dismounting restores
the constraint. X/Z movement remains available. The report describes both up
and down movement feeling locked; the isolated probe below only reproduces
downward cancellation and accumulated tracking-origin displacement.

## Evidence and likely mechanism

The XR pose remains enabled while mounted and unmounted. Vehicle attachment
suspends pedestrian gamepad locomotion, not head tracking. The gamepad
locomotion implementation changes X/Z only.

The more relevant difference is static-contact correction:

1. `src/engine/ecs/system/ik/head_pose_body_xz_follow.rs` sets body world Y to
   tracked head world Y plus the body's local offset.
2. `src/engine/ecs/system/avatar_control_system.rs::try_init_or_route_capsule`
   places the avatar capsule under a translation fork of that model root.
   Rotation and scale are dropped, but all three translation axes pass through.
3. The capsule's collision response targets the authored outer locomotion root,
   `rei_mu_locomotion_root`. That root also supplies the XR tracking origin.
4. `src/engine/ecs/system/static_contact_system.rs` raises that root to resolve
   penetration of the floor. A head-driven capsule moving down can therefore
   cause an equal upward movement of the camera's parent, cancelling the HMD's
   world-space movement.
5. The same system skips correction when `is_movement_root_mounted` is true.
   Dismounting re-enables correction.

The scene's `Gravity.enabled(false)` does not disable collision correction.
This explains why turning gravity off is insufficient.

## Isolated probe

A temporary source copy at `/tmp/mittens-hmd-y-probe` used the real World,
TransformSystem, and StaticContactSystem with a floor, a tracked driver, a
head-relative capsule, and an outer movement target. No repository source was
changed. The capsule initially rested on the floor with head Y = 1.6 m.

Successive tracking changes produced:

| Tracked local head Y | World head Y before correction | World head Y after correction | Outer root Y after correction |
| --- | --- | --- | --- |
| 1.4 m | 1.4 m | 1.6 m | 0.2 m |
| 1.2 m | 1.4 m | 1.6 m | 0.4 m |
| 1.8 m | 2.2 m | 2.2 m | 0.4 m |

The focused test passed. Output is in `/tmp/mittens-hmd-y-probe.log` for this
session. This confirms the downward feedback mechanism, including an origin
that remains raised after the user stands up. It does not reproduce the full
avatar initialization, frame scheduling, actual stage geometry, or hardware
pose stream, so it does not yet establish the complete reported Y lock.

Follow-up hardware report: the user tested normal `mittens-corp`, not the
collision-disabled diagnostic. Button-triggered jumping works and the avatar
appears grounded while traversing stairs/the stage edge, but physical HMD Y
still feels constrained in both directions except while mounted. A button jump
changes Velocity on the outer root, so it does not test physical head movement.
The apparent stair following has not been traced; it is not yet evidence of a
particular snap-to-ground implementation or a successful diagnostic result.

## Hardware isolation check

A temporary copy of the scene is available for this session at
`/tmp/mittens-corp-hmd-y-collision-disabled.mms`. It adds
`.collision_disabled()` to the player's AVC constructor and uses absolute asset
imports so it can load outside the examples directory. Run from the repository:

```sh
cargo run --release -- load /tmp/mittens-corp-hmd-y-collision-disabled.mms
```

Check physical head movement before mounting, while mounted, and after
dismounting. This disables the avatar's generated collision capsule, including
wall contact, solely to isolate the issue. It is not the proposed fix. This
hardware check is pending.

If the constraint remains, trace tracked local head Y, XR origin Y, body Y,
capsule bottom Y, and each static-contact correction across those transitions.
That will distinguish other transform effects from this confirmed feedback.

## Tentative fix direction

### Intended motion boundary

Grounding and static contact constrain gamepad locomotion, scripted movement,
and velocity/physics. Physical HMD movement is tracking input composed inside
that resolved locomotion frame. Ordinary contact must not translate the frame
to cancel a change in the tracked pose.

The current rig already has the two transform layers: the outer
`rei_mu_locomotion_root` and its descendant `rei_mu_xr_driver`. The coupling is
in the proxy: its pose follows the head-driven avatar body while its response
targets the outer root. Merely adding another parent transform or changing the
response target would leave that dependency intact.

Separate the collision proxy's vertical grounding frame from head-driven body
translation. Give it a calibrated bottom/height relative to the locomotion
frame. The movement root should still receive genuine locomotion, gravity,
and floor-contact correction. Crouching may change the rendered body's pose
or, under a separate explicit policy, capsule height without lifting the
tracking origin to cancel the crouch.

Start by fixing the vertical dependency. Room-scale X/Z contribution to proxy
placement and contact needs an explicit policy too; retaining current X/Z
correction does not by itself satisfy unrestricted physical HMD locomotion.
Do not silently settle that broader policy while making the height fix.

Do not make the generic static-contact solver ignore vertical corrections:
it must still resolve falls and land button jumps. Likewise, disabling contact
immediately after an XR update is insufficient; later passes can still read a
head-driven proxy. The proxy's grounding reference must be independent of
physical head Y across every contact pass.

Decide that policy before changing capsule parenting or filtering translation;
simply discarding Y could break floor placement or scale calibration. Verify
physical crouch/stand, wall contact, floor grounding, and mount/dismount
transitions together.

Focused verification before a headset smoke check:

- With no simulated movement, vary raw HMD Y up and down. The locomotion
  root and grounding proxy bottom must stay unchanged; the world-space head
  must inherit the full tracking delta. Repeat to detect accumulated origin
  drift. HMD Y alone must not manufacture a contact impulse or grounded change.
- Move the same frame with gamepad/scripted displacement and Velocity.
  Floor/wall penetration still resolves, falls still land, and genuine support
  still updates the corresponding Velocity's grounded state.
- Test combined simulated movement and changing HMD height through the real
  frame phases, not only an isolated collision tick.
- Mount, dismount, and remount without losing the motion separation or applying
  a tracking-origin offset twice. Check initial placement and recentering too.

Related plans:

- [XR avatar grounding first slices](../task/xr-avatar-grounding-first-slices.md)
- [AVC upright character capsule](../task/avc-upright-character-capsule.md)
- [Attachment topology cleanup](../task/attachment-system-world-owned-topology-cleanup.md)
