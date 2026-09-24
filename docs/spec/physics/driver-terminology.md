# Pose and velocity driver terminology

Status: proposed vocabulary. The linear `VelocityComponent`/`VelocitySystem`
and one-shot `translate` commands are implemented; gravity, angular motion,
and general force inputs described here remain proposed.

## The boundary

A **pose driver** writes a transform's position, rotation, or scale. A
**velocity driver** writes or requests a change to linear or angular velocity.
The velocity driver does not move the transform directly.

```text
input / gravity / throttle
            ↓
   velocity drivers
            ↓
 linear or angular velocity state
            ↓
   velocity integration              ← pose driver
            ↓
     transform pose
            ↓
     descendants
```

`VelocityComponent` (MMS `Velocity {}`) is the proposed outer-root motion
state and active pose driver: its system integrates velocity into a transform
at a fixed step. It may have linear velocity, angular velocity, or both. If
the implementation splits these into `Velocity` and `AngularVelocity`, both
are pose drivers when their respective integration is enabled. A component
that only observes a transform's measured velocity is motion telemetry, not
an active pose driver.

The word *driver* describes which quantity a component changes, not whether
it receives input from a person. `InputXRGamepad` currently drives a
locomotion transform directly and is therefore a pose driver on that path.
A future vehicle throttle can instead be a velocity driver if it changes the
vehicle's velocity and leaves transform integration to `VelocitySystem`.

## Transform inheritance versus velocity

Transform parenting is **pose composition**, not a velocity driver. For
example, while riding a vehicle, the rider's XR rig inherits the vehicle
mount's effective transform; the HMD's tracked left/right offset remains a
child pose relative to that rig. Moving one's head sideways does not add
linear velocity to the vehicle or require a `Velocity` on the HMD. Roughly,
`head_world = vehicle_mount_world × rider_local × tracked_head_local`.

A `Velocity` has no pose of its own. As a pose driver, its intended topology
is `Velocity { T { ... } }`: it drives its **child** transform, as `InputXR`
does. The nearest transform **ancestor** of `Velocity` supplies the
parent-local coordinate frame for that child's translation and the default
orientation basis for `translate(delta)`. An explicit
`rotation_basis(source)` can interpret a command using another orientation,
but converts the result into that same stored parent-local velocity; it does
not change the target or reparent the transform. A parent rotation then
turns the inherited motion, while a physical metres-per-second
interpretation needs explicit compensation for parent scale. If there is
no transform ancestor, use the world frame as the parent frame. The current
implementation follows this topology and stores parent-local linear
velocity; the [XR button/readback task](../../task/xr-linear-velocity-button-click-and-readback.md)
tracks headset interaction verification.

Separate velocity layers need separate transform targets:

```text
T.basis                    ← vehicle / grounding parent frame
  Velocity.outer
    T.outer_motion         ← driven relative to T.basis
      Velocity.inner
        T.inner_motion     ← driven relative to T.outer_motion
          XR rig           ← inherits both poses
```

The inner layer's world displacement composes with the outer layer's
transform; it is not a second write to `T.outer_motion`. Two `Velocity`
components can share a transform ancestor if each owns a different child
transform. Nesting `Velocity { Velocity { T { ... } } }` does **not** create
two independent pose layers: both drivers would target the same nearest
child transform unless a distinct transform is inserted between them.
The implementation requires one immediate child transform and rejects a
missing or ambiguous target. Each driver owns one target/channel.

## Velocity driver kinds

| Kind | Example | Effect on velocity |
| --- | --- | --- |
| Persistent acceleration | Gravity | Adds `a × dt` to linear velocity each fixed step while enabled. |
| Controlled acceleration or force | Engine throttle, thruster | Adds acceleration, or `F / mass`, each fixed step while commanded. |
| One-shot impulse | Jump, impact | Adds one velocity change at a defined step, then clears the request. |
| Velocity command | Cruise control, scripted speed | Sets or approaches a target velocity under an explicit priority/rate policy. |
| Angular counterpart | Steering torque, spin impulse | Changes angular velocity in radians per second. |

A throttle is usually an acceleration/force request, not a direct position
change and not necessarily a command to set speed. Releasing it stops adding
acceleration; it does not automatically set velocity to zero. Braking, drag,
friction, and contact impulses are separate velocity-changing mechanisms.

The proposed live `vel.translate(delta_mps)` is a **one-shot velocity change**,
not a pose translation and not acceleration integrated over time. A click
therefore supplies no `dt`. Its unsuffixed delta should use the nearest
transform ancestor's orientation by default, or an explicit `rotation_basis(component_ref)` such
as the inner `InputXR` rig's active eye orientation; optional `.horizontal()`
removes vertical thrust from that command. The basis is sampled at command
time, and the resulting delta should be converted into parent-local
velocity state. `vel.translate_world(delta_mps)` bypasses the command basis
but still converts the world-axis change into that state. The current
linear slice uses this parent-local stored state.
The analogous proposed `vel.rotate(delta_radps)` would change angular
velocity, not rotate the linear-velocity vector. These names are defined in
the [XR linear-velocity task](../../task/mittens-corp-linear-velocity-first-slice.md);
`translate` and `translate_world` are implemented; `rotate` is proposed.

Gravity is a persistent **linear acceleration** provider in the first slice.
For a mass-bearing dynamic body it can be represented as force `mass × g`,
which produces the same acceleration. This avoids requiring a mass value just
to make the XR grounding root fall. General forces, by contrast, need a mass
or inverse-mass policy before they can change linear velocity. Torque needs an
angular inertia policy before it can change angular velocity.

## Composition and authority

Velocity drivers submit typed requests at the fixed-step boundary. Compatible
acceleration and force requests accumulate for that step; a persistent provider
such as gravity contributes once to every substep. One-shot impulses apply
once. Velocity-setting commands need explicit precedence and must not be
silently added to accelerations. Reject two incompatible authorities that both
claim to set the same velocity channel.

The integration system consumes the resulting velocity once and writes its
owned transform channels once per substep. A contact constraint may correct
the proposed pose and change the velocity along the contact normal; the next
substep begins with that corrected velocity. Systems must not hide a second
velocity accumulator inside collision response.

Authority is per transform/channel, not per whole object. The XR humanoid can
compose layers:

```text
grounding root       Velocity + gravity → outer pose
  └── locomotion     XR gamepad → XZ pose
        └── tracking OpenXR → tracked head and hand poses
              └── rig AVC / IK / animation → joint poses
```

Gravity changes the outer root's linear velocity. The HMD, controller, and
joint transforms do not receive that velocity directly; they inherit the
outer-root displacement through transform propagation. This is why a skinned
mesh does not need a velocity component on each bone.

## Component ownership

- `VelocityComponent` owns the current velocity used by integration and the
  explicit transform/channel it drives. `VelocitySystem` integrates it.
- Velocity-driving components or request sources own their acceleration,
  force, impulse, or target-velocity settings and lifetime. They do not write
  the pose.
- A future `PhysicsBodyComponent` defines mass, inertia, and simulation
  authority when general forces or moving-body contact need them. It is not
  required for uniform gravity acceleration in the first XR grounding slice.
- Contact/collision components provide geometry and contact policy. Contact
  may correct velocity and the final pose, but collision geometry alone is
  neither a pose driver nor a velocity driver.

The existing `GravityComponent` currently feeds the deprecated
`CollisionResponseSystem`, which has its own private velocity. The new gravity
velocity driver must use the first-class velocity state; migrate or retire the
old path explicitly before using the same `Gravity` surface syntax.

## Related work

- [XR avatar grounding first slices](../../task/xr-avatar-grounding-first-slices.md)
- [Scriptable Velocity pose driver](../../task/scriptable-velocity-pose-driver.md)
- [Velocity, forces, and pluggable physics](../../task/velocity-forces-and-pluggable-physics.md)
- [Spatial, collision, and physics naming](../../task/spatial-collision-and-physics-naming.md)
