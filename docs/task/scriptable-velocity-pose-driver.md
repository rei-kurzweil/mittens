# Task: scriptable Velocity pose driver

Status: planned, revised 2026-09-23. The
[XR linear-velocity slice](mittens-corp-linear-velocity-first-slice.md) defines
the first implementation. This task also tracks later angular motion and
[broom flight](broom-flight-followup.md).

## Contract

`VelocityComponent` holds active linear and, later, angular velocity.
`VelocitySystem` integrates that state into one transform. Together they are
a **pose driver**; gravity, throttle, and other inputs that change velocity
are **velocity drivers** under the
[physics terminology](../spec/physics/driver-terminology.md). Descendants
inherit the resulting pose once, while authored local offsets, rotations,
and scales remain intact.

One `Velocity {}` component attaches directly beneath the transform it
drives. Its default state is zero. There are no separate linear/angular
component flavors, nested velocity wrappers, or descendant-transform search:

```mms
let vel = Velocity {}
T {
    name = "vehicle_motion_root"
    vel
    T { C3D {} }
}

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

The unsuffixed methods interpret their delta in the driven transform's local
orientation at command time. They convert that delta to world space and add
it to world-space velocity state. An existing velocity vector does not turn
when the object later turns. Expose explicit `translate_world(delta)` and,
when angular motion arrives, `rotate_world(delta)` for world-axis commands.
Ignore scale when mapping a local direction; rotated/scaled parents must not
change commanded world speed. An HMD turning *inside* an outer grounding root
does not turn that root's local basis. HMD-facing thrust needs a separate
reference-orientation policy or an explicit world-space direction.

Provide explicit read/set/zero access to current velocity without requiring
an inverse `translate` call. The exact MMS spelling of those accessors and
authored initial nonzero state should be specified with their first consumer;
do not introduce channel-specific constructors for them. Keep authored
initial configuration distinct from transient live state so serialization
does not save accidental button presses.

## Transform and update boundary

Resolve the component's immediate parent transform as its single target.
Reject a missing parent or competing writer to the same transform/channel;
do not integrate a shared ancestor/descendant channel twice. Store current
velocity in `VelocityComponent`, not in a collision system's private map.
Live component references must update that stored state, not a copied
construction value, and must work without rebuilding the tree. Define the
command's apply step so a click changes velocity exactly once before the next
fixed substep, regardless of render rate or callback replay.

Use a bounded fixed timestep and deterministic order. Convert each
world-space displacement through the effective parent basis before writing
local translation; parent rotation/scale must not alter speed. Reject
non-finite deltas/state and singular bases with a clear diagnostic. The
component needs explicit enable/disable and cleanup behavior. Angular
integration, when added, must use world-space angular state and convert the
result to the target's local rotation without scale affecting angular speed.

## Orientation helpers

For steering relative to a *different* object, MMS eventually needs a way to
obtain a world-space direction from its orientation:

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
