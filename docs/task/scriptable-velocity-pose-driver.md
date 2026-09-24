# Task: scriptable Velocity pose driver

Status: linear XR slice implemented, revised 2026-09-23. The
[XR linear-velocity slice](mittens-corp-linear-velocity-first-slice.md) defines
the current implementation. This task also tracks later angular motion and
[broom flight](broom-flight-followup.md).

Design correction: the intended invariants are **pose-driver parent topology**
(`Velocity { T { ... } }`) and **parent-local stored velocity**. The current
child-of-target/world-space implementation and descriptions below must be
reconciled under the
[XR button/readback follow-up](xr-linear-velocity-button-click-and-readback.md)
before adding readback, gravity, or other drivers.

## Contract

`VelocityComponent` holds active linear and, later, angular velocity.
`VelocitySystem` integrates that state into one transform. Together they are
a **pose driver**; gravity, throttle, and other inputs that change velocity
are **velocity drivers** under the
[physics terminology](../spec/physics/driver-terminology.md). Descendants
inherit the resulting pose once, while authored local offsets, rotations,
and scales remain intact.

One `Velocity {}` wraps the child transform it drives, like other pose
drivers. The nearest transform ancestor supplies the parent-local frame.
Its default state is zero. There are no separate linear/angular component
flavors or implicit target searches through arbitrary descendants:

```mms
let vel = Velocity {
    T { name = "vehicle_motion_root" C3D {} }
}
T { name = "vehicle_parent_frame" vel }

// Live methods on the retained component reference:
vel.translate([0.0, 0.0, -0.25]) // add linear velocity, m/s
vel.rotate([0.0, 0.5, 0.0])      // later: add angular velocity, rad/s
```

`translate` and `rotate` name the *state being driven*. Neither immediately
translates nor rotates the transform. Each call adds a one-shot change to
velocity; neither multiplies its argument by `dt`. `rotate` means a change to
angular velocity, **not** a rotation of the linear-velocity vector. A future
persistent acceleration/throttle API will use time explicitly at fixed steps.
The first XR slice implements `translate` and linear integration; `rotate`
and angular integration remain follow-up work.

By default, `translate(delta)` interprets the delta in the nearest transform
ancestor's orientation at command time. An optional component
reference overrides that command basis:

```mms
let xr_input = InputXR.on() { /* tracked rig */ }
let vel = Velocity.rotation_basis(xr_input).horizontal() {
    T { name = "grounding_root" T { xr_input } }
}
T { name = "grounding_parent_frame" vel }
```

For an `InputXR` reference, resolve the active published XR eye orientation
belonging to that rig; `InputXR` itself is only a pose-driver marker.
`.horizontal()` projects local translation commands onto world XZ and
normalizes the heading, so HMD pitch cannot create vertical thrust. Without
the option, use the nearest transform ancestor's effective world orientation. A referenced
transform may supply its effective world rotation as a non-XR basis. If the
source is missing, disabled, ambiguous, or has no valid pose, reject the
command rather than falling back silently. This read-only descendant
reference does not change the transform hierarchy or the target Velocity
integrates.

Convert the selected basis and local delta into parent-local velocity,
ignoring translation and scale. Existing momentum does not turn when the
command source later turns, but it *does* turn if the parent transform
rotates. Expose explicit `translate_world(delta)` to bypass the configured
basis while still converting its delta into parent-local state; when angular motion
arrives, define `rotate_world(delta)` and whether `rotation_basis` also applies
to `rotate`. Rotated/scaled parents must not change commanded world speed.

Provide explicit read/set/zero access to current velocity without requiring
an inverse `translate` call. The exact MMS spelling of those accessors and
authored initial nonzero state should be specified with their first consumer;
the [XR button/readback follow-up](xr-linear-velocity-button-click-and-readback.md)
now proposes `Velocity.linear()` for the linear getter. Do not introduce
channel-specific constructors for them. Keep authored
initial configuration distinct from transient live state so serialization
does not save accidental button presses.

## Transform and update boundary

Resolve the component's child transform as its single target and its nearest
transform ancestor as the parent-local frame. Reject a missing/ambiguous
child target or competing writer to the same transform/channel; distinct
velocity layers require distinct intervening transforms. Store current
velocity in `VelocityComponent`, not in a collision system's private map.
Live component references must update that stored state, not a copied
construction value, and must work without rebuilding the tree. Define the
command's apply step so a click changes velocity exactly once before the next
fixed substep, regardless of render rate or callback replay.

Use a bounded fixed timestep and deterministic order. Integrate parent-local
velocity into the child target's local translation; compensate for parent
scale so it does not alter physical speed, while parent rotation turns the
inherited motion. Reject
non-finite deltas/state and singular bases with a clear diagnostic. The
component needs explicit enable/disable and cleanup behavior. Angular
integration, when added, must define parent-local angular state and write
the target's local rotation without scale affecting angular speed.

## Orientation helpers beyond the configured basis

The configured `rotation_basis` covers the first XR velocity button scene.
For arithmetic involving several orientations, MMS may also need a reusable
way to obtain a world-space direction from an arbitrary object's orientation:

```text
direction = rotate_vector(orientation_xyzw, local_forward_axis)
world_delta_velocity = direction * speed_change
```

This is not truncating a vec4 to vec3. Normalize valid quaternions, reject
invalid/zero ones, and document `xyzw` ordering. A transform/matrix direction
helper should ignore translation and scale and define behavior for shear,
mirrors, and degenerate bases. Prefer reusing
[transform accessors](../draft/transform-component-accessors-engine-api.md).
An orientation used by a one-shot command is a snapshot; continuous steering
needs a persistent velocity driver that refreshes its request.

## Motion authority and existing draft

The older [velocity-components WIP](wip/velocity-components.md) proposes
observed velocity/history, explicitly not active integration. Reconcile
storage/naming later without making telemetry or collision-system migration
prerequisites for this pose driver. Do not feed measured velocity back into a
second integrator. For the broom, enable the driver only after mount handoff
has removed the pointer attachment and established independent vehicle
motion; dismount disables/zeros it according to that example's policy.

## Acceptance

- The XR [two-button scene](mittens-corp-linear-velocity-first-slice.md) adds
  and cancels linear velocity by live reference without gravity or floor
  contact; one click is one change independent of `dt`.
- Nested descendants inherit displacement once, including with rotated and
  scaled parents. Equal elapsed time at different render rates produces
  equivalent displacement.
- Local and explicit world changes use their documented bases; invalid
  values and competing pose writers fail clearly.
- Disabling/removing the driver stops its writes without changing authored
  descendant locals or leaving stale motion authority.
- Later angular tests show `rotate` changes angular velocity, angular motion
  composes with linear motion in the same component, and neither channel is
  integrated twice.
- Mount/dismount and ordinary XR camera, pointer, grabbable, layout, transform
  propagation, serialization, and component-reference behavior remain intact.

## Related work

- [Velocity, forces, and pluggable physics](velocity-forces-and-pluggable-physics.md):
  longer-term authority, integration, backend, and performance boundary.
- [Global MMS keyboard events first slice](mms-keyboard-events-first-slice.md):
  input edges that can update a live `Velocity` reference.
- [MMS keyboard and regular gamepad events](mms-keyboard-and-gamepad-events.md):
  parent input task; regular desktop gamepads remain planned.
- [Velocity / AngularVelocity components WIP](wip/velocity-components.md):
  observed velocity, history, collision, and solver ownership; related
  storage, not the active integration contract here.
