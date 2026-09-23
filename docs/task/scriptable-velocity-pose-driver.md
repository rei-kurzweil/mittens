# Task: scriptable Velocity pose driver

Status: planned, 2026-09-07. Supports [broom flight](broom-flight-followup.md).

## Contract

Introduce `Velocity` as a pose driver that does not require user input. It
integrates configured linear velocity in world units per second and angular
velocity in radians per second using frame elapsed time, then applies the
resulting motion to descendant transforms through the existing pose/transform
pipeline. Descendants inherit motion once, not once per ancestor. Preserve
authored offsets, rotation, and scale.

Terminology follow-up: [pose and velocity drivers](../spec/physics/driver-terminology.md)
calls the active `Velocity` integrator a **pose driver**. Gravity, throttle,
and other inputs that change its velocity are **velocity drivers**. The older
uses of "velocity driver" below mean the active `Velocity` pose driver and
should be reconciled when this task is implemented.

## Authoring shape and driven-transform boundary

Prefer the pose-driver wrapper form. It composes naturally with the existing
`Input` authoring style and makes motion authority visible from the tree:

```mms
Velocity.linear(0.0, 0.0, 1.0) {
    T {
        C3D {}
    }
}

Velocity.angular(0.01, 0.0, 0.0, 1.0) {
    T {
        C3D {}
    }
}
```

For `angular(speed_radians_per_second, axis_x, axis_y, axis_z)`, the four
arguments are axis-angle velocity, not a quaternion: the first is angular speed
and the remaining three form the rotation axis. Normalize a finite non-zero
axis and reject invalid values. This exact spelling remains subject to the MMS
component registration pass, but it is the target API for this task.

Do not switch to `T { Velocity... }` merely because some existing systems assume
an immediate transform relationship. Instead, make the wrapper topology work
deliberately and test it. A velocity driver resolves exactly one first
descendant `Transform` boundary, traversing only explicitly documented
transparent pose-driver/configuration nodes. It stops descending after reaching
that transform, so nested transforms inherit the result normally and are not
integrated again.

Nested linear and angular drivers must compose around one transform:

```mms
Velocity.linear(0.0, 0.0, 1.0) {
    Velocity.angular(0.01, 0.0, 0.0, 1.0) {
        T { C3D {} }
    }
}
```

This requires an explicit resolver rather than an unrestricted descendant
search. Reject an ambiguous driver branch with multiple first transforms unless
multi-target behavior is intentionally added later. Detect two drivers trying
to integrate the same degree of freedom and report the authority conflict
instead of silently applying motion twice.

The compatibility audit must cover systems that encode structural assumptions.
In particular, desktop `InputSystem` currently searches for a direct
`Transform` child of `Input`, whereas XR locomotion searches upward for an
ancestor transform. Adding a velocity wrapper must not silently break camera,
pointer, grabbable, layout, transform propagation, serialization, or component
reference resolution. Where a system genuinely requires a direct relationship,
document the valid nesting order or migrate it to the shared pose-driver
boundary resolver rather than adding one-off recursive searches.

Expose construction and live get/set through an MMS component reference.
Live component references update either mode without rebuilding the tree:

```mms
let flight = Velocity.linear(0.0, 0.0, 0.0) {
    T { name = "vehicle" }
}
flight.set_linear([0.0, 0.0, 2.0])
flight.set_linear([0.0, 0.0, 0.0])
```

Updates change the live component, not a copied value or just its construction
configuration. Specify update ordering and whether a handler's update applies
this tick or next. Default to zero velocity; provide explicit enable/disable
semantics, finite-value validation, and cleanup when the driver is removed.
Serialize authored initial configuration separately from transient scripted state.

Start with explicitly world-space linear velocity. Correctly convert displacement
through the effective parent basis; parent rotation/scale must not silently
change commanded world speed. Angular integration is also world-space for the
first slice and must be converted into the driven transform's local rotation
without scale affecting angular speed. Document handling of singular parent
transforms. If local-space velocity is added, expose its space rather than infer
it.

## Orientation helpers

MMS needs a reusable way to rotate a chosen local axis by a quaternion and
obtain a direction vector. This is not truncating a vec4 to a vec3:

```text
direction = rotate_vector(orientation_xyzw, local_forward_axis)
velocity = direction * speed
```

Names above are provisional. Normalize valid quaternions, reject invalid/zero
quaternions, and document `xyzw` ordering and the selected forward-axis convention.
Allow reading an object's local or world orientation through existing transform
accessors. A transform/matrix direction convenience should ignore translation
and yield a unit direction without scale changing speed; specify behavior for
shear, mirrored, and degenerate bases. Prefer reusing
[transform accessors](../draft/transform-component-accessors-engine-api.md).

Provide an easy direction-and-speed setter or a helper composition from MMS;
avoid requiring authors to implement quaternion math themselves. A setter using
an orientation snapshots it unless explicitly documented as a live binding.
Continuous steering must refresh the velocity when orientation changes.

## Motion authority and existing draft

The older [velocity-components WIP](wip/velocity-components.md) proposes storage,
history, and derived motion observations, explicitly not integration. This task
defines the requested active driver. Reconcile naming/shared storage during
implementation without making observed-velocity history or collision-system
migration prerequisites. Commanded angular integration in this ticket likewise
must not depend on migrating the WIP's observed `AngularVelocityComponent`.
Observed velocity and commanded velocity must not be confused or fed back into
two integrators.

Exactly one owner integrates a driven transform. For the broom, enable the
driver only after the mount handoff has removed the pointer attachment and
established independent vehicle motion. Dismount disables/zeros it according
to the flight example's policy.

## Acceptance

- An input-free MMS scene moves or rotates a wrapped descendant at the configured
  linear or angular speed.
- Changing the component by reference changes motion; zero stops it.
- Nested descendants inherit displacement once, including rotated/scaled parents.
- Nested linear and angular wrappers compose without double integration.
- Wrapper insertion does not break camera, pointer, grabbable, layout,
  serialization, transform propagation, or component-reference behavior that
  previously relied on an immediate transform relationship.
- Ambiguous transform boundaries and competing motion authorities fail clearly.
- Equal elapsed time at different frame rates yields equivalent displacement.
- Identity and quarter-turn orientations produce the documented directions;
  invalid values fail clearly, and scale does not change commanded speed.
- Disable/removal and mount/dismount leave no stale motion or competing driver.

## Related work

- [Velocity, forces, and pluggable physics](velocity-forces-and-pluggable-physics.md):
  longer-term authority, integration, backend, and performance boundary.
- [Global MMS keyboard events first slice](mms-keyboard-events-first-slice.md):
  implemented input edges that can update a live `Velocity` reference.
- [MMS keyboard and regular gamepad events](mms-keyboard-and-gamepad-events.md):
  parent input task; regular desktop gamepads remain planned.
- [Broom flight follow-up](broom-flight-followup.md): first intended consumer of
  scripted velocity and the place to verify mount/dismount motion authority.
- [Velocity / AngularVelocity components WIP](wip/velocity-components.md):
  observed velocity, history, collision, and solver ownership; related storage,
  but not the active integration contract defined here.
- [Transform component accessors](../draft/transform-component-accessors-engine-api.md):
  orientation/direction helpers needed for steering-relative velocity.
