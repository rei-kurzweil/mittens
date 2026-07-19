# AVC upright character capsule, collision routing, and desktop input routing

Date: 2026-07-19

Status: planned.

## Summary

Add a real vertical capsule collision shape and use it for Bisket/avatar and
first-person player colliders.  The capsule pose should follow AVC's body
alignment, remain upright when the desktop camera or XR headset pitches and
rolls, and send collision correction to the transform that owns locomotion.

This task also removes the `I.speed(0.0)` adapter from
`examples/secondary-motion-desktop.mms` by allowing one desktop `Input` to
route translation and rotation to different transforms.

The chosen AVC contract remains head-anchored for both desktop and XR.  This
task does not add a body-anchored desktop-only head topology.

## Current findings

- `CollisionShape` currently supports only centered axis-aligned cubes and
  spheres. There is no capsule constructor or narrow phase.
- AVC does not select separate desktop and XR head-splice strategies. Both
  paths move the head bone beneath `driven_t -> head_target` and run body
  follow. XR adds pose-valid gating; `Camera3D` adds a yaw-correction mount.
- AVC currently creates an empty transform stored as `splice_head`, while the
  actual head parent is a different `head_target` transform. Comments that say
  an `AimConstraint` drives `splice_head` are stale.
- Desktop `Input` writes translation and rotation to one direct transform
  child. The secondary-motion example therefore uses an outer translating
  input and an inner zero-speed rotating input to manufacture the head-level
  pose AVC expects without pitching the body collider.
- XR examples commonly put a sphere and `KineticResponse` on the camera
  wrapper. This makes the collision center follow the head, and collision
  response moves the wrapper instead of the outer transform moved by
  `InputXRGamepad`.
- `KineticResponseSystem` resolves all static penetration with AABBs, even for
  spheres. A capsule narrow phase alone would therefore not provide proper
  rounded character-controller behavior around corners.

## Chosen transform-stream design

Use the existing transform stream operators to derive the capsule pose from
the AVC model/body root:

```text
AVC
└── body_pipeline (runtime TransformForkTRS + QuatYawFollow)
    └── model_root (authored T; AVC owns world body alignment)
        ├── GLTF / armature
        └── capsule_pose (TransformForkTRS)
            ├── TransformMapTranslation { pass }
            ├── TransformMapRotation { TransformDrop }
            ├── TransformMapScale { TransformDrop }
            └── capsule_center T.position(0, center_y, 0)
                └── Collision.kinematic
                    ├── CollisionShape.capsule_y(...)
                    └── KineticResponse.slide
```

`TransformForkTRS` consumes `model_root.world` as its parent-world input:

- translation passes through, so the capsule follows AVC body-follow X/Y/Z;
- rotation drops to identity, so HMD pitch/roll and body yaw cannot tilt it;
- scale drops to one, so model/import scale cannot distort physical dimensions;
- the child `T.position(0, center_y, 0)` applies the stable world-up body-center
  offset after rotation has been removed.

This is preferable to following `driven_t` or the camera wrapper. The model
root already contains AVC's calibrated body position, including the head/eye
offset and body-follow result.

Transform streams are deliberately one-way. Collision response must not write
to the stream output because the next AVC/stream update would overwrite it.
Instead, response applies the computed world-space correction delta to an
explicit locomotion target.

## Public interfaces

### Vertical capsule

Add:

```rust
CollisionShape::CapsuleY {
    radius: f32,
    half_segment: f32,
}
```

The central line segment runs from `[0, -half_segment, 0]` to
`[0, half_segment, 0]`; hemispheres of `radius` cap both ends. Total height is:

```text
2 * (half_segment + radius)
```

Expose matching constructors:

```rust
CollisionShape::capsule_y(radius, half_segment)
CollisionShapeComponent::capsule_y(radius, half_segment)
```

```mms
CollisionShape.capsule_y(radius, half_segment)
```

Reject or clamp negative dimensions consistently at component construction and
MMS parsing. Serialization must round-trip the exact radius and half-segment.

Initial Bisket calibration:

```mms
CollisionShape.capsule_y(0.28, 0.52)
```

This preserves the current secondary-motion body collider's total height of
1.60 units while reducing horizontal width to a 0.56-unit diameter.

### Routed kinetic response

Add an optional locomotion target to `KineticResponseComponent`:

```rust
pub movement_target_source: Option<ComponentRef>
pub(crate) movement_target_id: Option<ComponentId>

with_movement_target_source(ComponentRef)
```

```mms
KineticResponse.slide() {
    movement_target("#avatar_locomotion_root")
}
```

Behavior:

- no target preserves the existing immediate-parent behavior;
- an explicit target must resolve to a `TransformComponent`;
- unresolved targets skip response for that frame and leave collision
  detection active;
- response computes penetration at the collider pose, then applies the same
  world-space correction delta to the movement target's current world pose;
- gravity and push velocity use the same routed-delta rule;
- query and GUID references serialize through the existing `ComponentRef`
  representation.

For desktop, the target is the transform translated by `Input`. For XR, it is
the outer transform selected by `InputXRGamepad::locomotion_target_transform`,
not the HMD-driven transform or camera wrapper.

### Routed desktop rotation

Add to `InputTransformModeComponent`:

```rust
pub rotation_target_source: Option<ComponentRef>

with_rotation_target_source(ComponentRef)
```

```mms
rotation_target("../#avatar_head_driver")
```

Resolution uses the same containing-`Input` subtree scope as
`translation_basis(...)`:

- translation continues to update the direct transform child;
- rotation updates the referenced transform;
- no route preserves current single-target behavior;
- an unresolved explicit route skips rotation rather than pitching the
  translation/collider root;
- when the translation basis and rotation target resolve to the same transform,
  movement uses the newly calculated rotation in the same frame.

## Collision implementation

### Bounds and broad phase

- Add capsule local/world AABBs using radius on X/Z and
  `half_segment + radius` on Y.
- Keep the existing BVH broad phase using those AABBs.

### Narrow phase

Implement all symmetric pairs:

- capsule/capsule: closest points between the two vertical segments;
- capsule/sphere: closest point on the capsule segment to the sphere center;
- capsule/cube: squared distance between the vertical segment and the AABB;
- preserve existing cube/cube, cube/sphere, and sphere/sphere behavior.

Contact at exactly the summed radius remains an intersection, matching current
sphere semantics.

### Shape-aware separation

Replace the unconditional `compute_push_out_aabb` response path with a
shape-pair penetration function returning a world-space minimum translation
vector. Implement capsule/cube first because it is the character-versus-room
case exercised by the examples, then cover all other supported pairs.

The capsule/cube result must choose a stable floor/ceiling normal for vertical
contacts and a radial horizontal normal around wall corners. Existing
`push_out_epsilon`, iteration limits, friction, and velocity handling remain in
effect after the MTV is selected.

## AVC topology cleanup

Keep the shared rigid/head-anchored behavior, but make runtime state describe
the topology that actually exists:

- remove the unused empty transform currently attached beneath the old head
  parent;
- store the real `head_target`/head mount as AVC's initialized head-splice ID,
  or rename the internal field to `head_mount`;
- keep the displaced head as a child of that mount;
- update topology comments and examples that still claim an AVC-created head
  `AimConstraint` exists;
- retain XR pose-valid initialization gating and desktop `Camera3D` correction.

This cleanup is behavior-preserving but makes capsule/body alignment tests
inspect the real topology rather than an orphan sentinel.

## Example migration

Create a reusable Bisket capsule factory/component snippet so dimensions and
stream topology do not drift across examples. Convert active player/avatar
colliders in:

- `secondary-motion-desktop.mms`;
- `input-xr-gamepad.mms`;
- `vtuber-editor-example.mms`;
- `vtuber-mirror-example.mms`;
- `bisket-desktop-demo.mms`;
- the desktop spectator/player rig in `bisket-vr-demo.mms`.

Update the commented XR collision blocks in `bisket-vr-demo.mms` and
`bisket-vr-only-example.mms` to show the correct body capsule and routed
locomotion target instead of a head-mounted sphere.

Do not add collision to unrelated first-person examples that have no collision
environment. New first-person examples should use the capsule pattern rather
than a camera-centered sphere.

For `secondary-motion-desktop.mms`, also:

- remove `desktop_head_input` and `I.speed(0.0)`;
- use one `Input` with `rotation_target` set to `avatar_head_driver`;
- translate the existing grounded locomotion root;
- use the head target as the translation basis;
- keep the capsule stream beneath AVC's model root and route its response back
  to the grounded locomotion root.

## Implementation order

1. Add capsule data model, MMS API, bounds, narrow phase, and shape-aware MTVs.
2. Add `KineticResponse.movement_target(...)` and refactor response application
   around world-space deltas.
3. Prove the model-root `TransformForkTRS` capsule follower in a focused
   synthetic AVC test for desktop and XR head poses.
4. Add `InputTransformMode.rotation_target(...)` and remove the two-input
   desktop workaround.
5. Clean up AVC's empty splice and stale topology documentation.
6. Migrate the listed examples and shared Bisket capsule configuration.

## Tests and acceptance criteria

### Shape tests

- Capsule MMS and Rust round trips preserve dimensions.
- Capsule AABBs have the expected radius and total height.
- Capsule/cube tests cover floor, ceiling, flat wall, outside corner, exact
  tangent, and separated cases.
- Capsule/sphere and capsule/capsule tests cover overlapping, tangent, and
  separated cases.
- Shape-aware response slides around a box corner without behaving like the
  capsule's AABB.

### Routing tests

- Routed kinetic response moves the target transform by the collider's MTV and
  does not mutate the capsule offset transform.
- An unresolved response target produces no movement.
- Default response behavior remains compatible.
- Routed input rotates only the head driver, translates only the locomotion
  root, and uses current-frame head yaw for movement.

### AVC alignment tests

After AVC initialization and transform propagation, for both desktop and
pose-valid XR:

- capsule world rotation is identity after arbitrary head pitch, roll, and yaw;
- capsule XZ center matches AVC `model_root` XZ;
- capsule center Y equals model-root Y plus the authored body-center offset;
- the capsule bottom remains on the expected floor for a calibrated Bisket;
- collision correction moves the desktop/XR locomotion root;
- the correction does not change the camera wrapper's authored local offset;
- subsequent AVC body-follow keeps the avatar and capsule aligned.

### Example regressions

- `secondary-motion-desktop` contains one desktop `Input`, one Bisket capsule,
  and no avatar cube collider.
- Mouse pitch does not rotate or orbit the capsule.
- WASD and XR thumbstick locomotion both collide through their owning outer
  transforms.
- Existing XR pose-valid gating, camera alignment, hand IK, secondary motion,
  and static scenery collision tests continue to pass.

