# Skinned-mesh grounding and static floor contact

Status: proposed.

The implementation begins with the focused
[XR avatar grounding first slices](./xr-avatar-grounding-first-slices.md).
That task establishes the outer Bisket grounding root, a dedicated vertical
velocity path, and one static floor contact before generalizing this design
to other skinned models.

## Goal

Make an opt-in skinned-model placement rig settle onto a named static floor so
models can be compared at one real ground plane. The first acceptance scene is
[`examples/capsule-stick-figure.mms`](../../examples/capsule-stick-figure.mms):
the A-pose capsule subject and the live Bisket reference must have observable,
repeatable floor contact before pose retargeting work begins.

This is grounding, not automatic visual normalization. If the capsule is twice
as tall as Bisket after both stand on the same floor, that is a genuine scale or
bounds discrepancy to diagnose; gravity must not silently rescale it or align
their heads.

## Why a new task is necessary

The existing AVC path already infers a runtime upright capsule from imported
render bounds and gives it static-overlap correction. That is useful precedent,
but not the foundation for general gravity:

- AVC creates a runtime `Collision.kinematic` capsule, centered from the
  model-root render bounds, and routes static-overlap correction to the actual
  locomotion target.
- `secondary-motion-desktop.mms` has a static physical floor, but it does not
  put the Bisket root under gravity.
- The current `CollisionResponse` implementation contains a cached ancestor
  `Gravity` coefficient and private velocity, but is planned for retirement.
  Its `slide` mode also returns before integration when it has no currently
  overlapping static collider, so it cannot reliably start a free fall from
  above a floor.

Do not add a new dependence on `CollisionResponse`, its private velocity, or
its gravity behavior. The grounding work must align with the planned explicit
velocity/force and static-nonpenetration designs.

## Scope and ownership

Grounding is **opt-in** at a model's placement root. It must not be inferred
for every imported GLB: scenery, hand props, animated set pieces, and XR
head-driven avatars may have different transform authority.

The provisional feature is a `SkinnedMeshGrounding` configuration/system (the
public name may become `ModelGrounding` if renderable-subtree support is made
generic). It owns a runtime collision proxy and, in gravity-enabled mode, a
vertical motion state for one declared movement target.

```text
placement transform / movement target     ← only transform this feature moves
  └── GLTF model root
        └── imported skeleton and skinned meshes  ← never translated per bone

runtime grounding proxy                   ← non-serialized, non-selectable
  └── inferred upright capsule
        └── pose-driven collidable/contact constraint
```

The feature must preserve the GLTF's authored local scale, root transform, and
bone pose. Settling translates the declared placement root as a whole. It must
not compensate a scale mismatch by changing scale, and must not attach physics
components to imported joints.

## Composed avatar motion layers

An avatar is not wholly "pose-driven" or wholly "velocity-driven." Those are
authorities for a particular transform (or transform channel) at a defined
point in the hierarchy. The usual composed-avatar topology has up to four
layers:

```text
physics / grounding root       ← gravity and vertical static-contact correction
  └── locomotion root          ← gamepad XZ movement and body yaw
        └── XR tracking root   ← tracked HMD/controller poses
              └── avatar rig   ← AVC, model root, skeleton, head/hand IK
```

The exact existing InputXR/AVC wrappers may differ, but the authority boundary
must preserve this composition. A physics system never moves the HMD node or
an imported head bone to make an avatar stand; it moves the ancestor grounding
root. The tracked HMD and controllers remain pose-driven descendants, while
AVC and IK continue to pose their own skeletal descendants.

The invariant is **one writer per transform degree of freedom per simulation
step**, not one writer per avatar:

| Layer | Typical owner | Channels it owns |
| --- | --- | --- |
| Grounding root | velocity/gravity plus floor constraint | vertical translation; optionally all translation for a fully dynamic body |
| Locomotion root | gamepad/input locomotion | horizontal translation and yaw |
| XR tracking root | OpenXR pose source | tracked local head/controller translation and rotation |
| Avatar rig/joints | AVC, IK, animation, pose overlays | model and joint-local transforms |

Where two sources genuinely need the same channel, introduce another explicit
wrapper or define a composition operator; never have both systems write the
same `TransformComponent` opportunistically. The focused XR slice first proves
the hierarchy with an inert outer root, then enables gravity and floor contact
on that root while leaving the tracked HMD pose on its existing descendant.

## One authoritative ground plane

Each comparison scene declares a named static floor/collidable and documents
its top surface as `ground_y`. For the capsule laboratory, choose the visible
stage deck as the comparison surface or add a dedicated invisible comparison
floor aligned to it; do not let the decorative floor and deck provide competing
contact heights.

Every groundable subject references that same floor scope/layer. A diagnostic
must report, after settling:

- movement-target world Y;
- inferred proxy bottom Y;
- floor top Y/contact normal;
- signed separation (positive gap / negative penetration);
- aggregate visual bounds min/max Y and their source;
- the model's measured standing height.

These values make it possible to distinguish a wrong floor, wrong root origin,
bad capsule inference, and a true two-times scale mismatch.

## Collision proxy inference

After the GLTF has spawned and CPU render bounds are available, measure the
aggregate visual subtree in the declared model-root frame. Reuse the AVC
upright-capsule inference policy as the first implementation:

```text
height       = bounds.max.y - bounds.min.y
center_y     = (bounds.min.y + bounds.max.y) / 2
radius       = min(authored_radius, height / 2)
half_segment = height / 2 - radius
```

AVC currently drops model-root rotation and scale when streaming its capsule.
That behavior is not automatically valid here: the capsule laboratory scales
the imported capsule instance, so blindly copying the AVC stream would give a
collision proxy at the wrong height. Grounding must infer and maintain the
proxy in the movement target's **effective scale**—either measure transformed
bounds in that frame or apply the same supported uniform scale to the proxy.
Non-uniform scale requires an explicit documented policy (initially: reject it
with a diagnostic or require an authored proxy). The proxy's effective bottom
must correspond to the effective visual-bounds bottom, so resting on `ground_y`
means the model is actually standing there.

First-slice limitations to record rather than hide:

- hair, wings, a long coat, or an A-pose can distort aggregate bounds;
- feet may not be the lowest visual vertices;
- one upright capsule cannot represent a seated, prone, or non-humanoid mesh;
- no bounds means no inferred collider: report this and require an authored
  proxy rather than falling back to an arbitrary unit shape.

An authored capsule/offset override is required before making this a general
avatar feature. The laboratory may begin only with measured Bisket and capsule
bounds plus diagnostics.

## Gravity and contact contract

Gravity and static nonpenetration have distinct jobs:

1. A named velocity/force provider adds downward acceleration to a
   gravity-enabled placement root in fixed simulation steps.
2. The grounding proxy supplies the proposed post-integration pose to a static
   floor-contact constraint.
3. The constraint applies the minimum upward world-space correction needed to
   leave static geometry and removes only velocity into the contact normal.
4. The corrected movement-target pose is propagated before cameras, mirrors,
   pose capture, and interaction queries observe it.

There is one transform writer. In gravity-enabled mode the grounder owns the
root's vertical translation; scripts/animation must not simultaneously write
that channel. Horizontal locomotion remains an explicit owner and may compose
only through a defined movement/root topology.

The initial policy has two explicit modes:

| Mode | Use | Behavior |
| --- | --- | --- |
| `settle_with_gravity` | static comparison subjects and released models | Integrate downward velocity and rest on static floors. |
| `static_contact_only` | externally pose-driven subjects | Correct static penetration but do not add gravity or alter externally authored airborne motion. |

The XR Bisket acceptance path uses `settle_with_gravity` on a dedicated outer
root after the inert-hierarchy check. It must prove how standing height,
crouching, recentering, and tracked HMD Y compose with gravity before enabling
that mode in the scene. `static_contact_only` remains useful for externally
posed subjects whose roots should not fall.

## Integration plan

1. Inspect and record Bisket and capsule aggregate bounds, root origins, mesh
   scale, capsule proxies, and deck/floor top Y in the capsule laboratory.
2. Add one static collision surface at the declared comparison ground plane and
   collision visualization for the floor and runtime proxies.
3. Extract/reuse only the bounds-to-upright-capsule inference needed by an
   opt-in grounding proxy. Keep the helper runtime-only and non-selectable.
4. Implement the explicit velocity/gravity provider and fixed-step integration
   required by [velocity, forces, and pluggable physics](./velocity-forces-and-pluggable-physics.md),
   or provide a narrowly scoped grounding integrator with the same authority
   and fixed-step contract. Do not reuse `CollisionResponse` velocity.
5. Implement/migrate the static floor constraint described in
   [retire collision response to static non-penetration](./retire-collision-response-to-static-nonpenetration.md).
6. Connect `settle_with_gravity` to a static capsule subject and verify it falls
   from several starting heights without tunneling through the floor or
   oscillating at rest.
7. Connect `settle_with_gravity` to Bisket's new outer grounding root and verify
   floor correction does not fight XR head/hand tracking, gamepad locomotion,
   or pose capture.
8. Add a comparison overlay/log with measured height and bottom-to-floor error;
   only then decide whether the capsule scene needs an authored scale change.

## Acceptance criteria

- [ ] The capsule and Bisket comparison proxies use one named ground plane and
  report bottom-to-floor error within 5 mm while resting.
- [ ] Starting the capsule above the floor in `settle_with_gravity` makes it
  descend and settle without penetration, visible jitter, or a second transform
  writer.
- [ ] The capsule's scale is unchanged by grounding; the final diagnostics make
  any height discrepancy numerically obvious.
- [ ] Static floors block groundable proxies but never move themselves.
- [ ] Bisket's outer grounding root settles under gravity without disrupting
  the XR camera, hand IK, gamepad locomotion, or pose capture.
- [ ] Removing/reloading a GLTF or floor cleans up proxy, velocity, constraint,
  and visualization state without orphan colliders.
- [ ] Unit tests cover capsule bottom inference, floor MTV/contact-normal
  handling, fixed-step settling, rest stability, movement-target routing, and
  no-double-writer authority conflicts.

## Out of scope

- Automatic equalization of model height, eye height, or scale.
- Ragdolls, rigid-body simulation, movable-versus-movable contact, slopes,
  stairs, step offsets, moving platforms, or continuous collision detection.
- Treating skinned mesh deformation as an exact physics collider.
- Changing the capsule figure into the XR avatar.

## Related work

- [Capsule stick figure XR pose-retargeting laboratory](./capsule-stick-figure-pose-retargeting.md)
- [AVC auto-calibrated upright capsule](./avc-upright-character-capsule.md)
- [Retire collision response to static non-penetration](./retire-collision-response-to-static-nonpenetration.md)
- [Velocity, forces, and pluggable physics](./velocity-forces-and-pluggable-physics.md)
