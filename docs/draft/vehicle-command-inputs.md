# Vehicle command inputs and separate attachment responsibilities

Status: provisional design, 2026-11-03. No new engine component, signal or MMS
API is implemented by this document. The existing implementations and gaps are
listed in the [station vehicle inventory](../review/space-station-vehicle-inventory.md).

## Responsibilities

Attachment and vehicle control are separate responsibilities:

| Responsibility | Owns | Does not imply |
| --- | --- | --- |
| Attachment / mounting | Relative placement, following, seat eligibility, occupancy and mount lifecycle | Permission to drive |
| Control ownership | Which player or NPC currently supplies commands | Physical attachment to the vehicle |
| Input adapter / driver | Collecting device state or NPC intent and producing a vehicle command | Integration of vehicle motion |
| Vehicle behavior | Allowed motion axes, acceleration limits and motion rules | Knowledge of keyboard keys, controller buttons or an NPC's decisions |
| Native motion | Integrating the resulting motion through existing Velocity | Input-device selection |

A passenger may attach without driving. An NPC, autopilot or remote player may
drive without being attached. A scene may choose to transfer control after
mounting, but that is an explicit policy between the two responsibilities.

The current AttachmentSystem still suspends/restores pedestrian input during
mounting. This proposal does not claim that implementation is already separated;
the broader [movement-authority task](../task/attachment-movement-authority-and-input-routing.md)
documents that existing coupling.

## Provisional command contract

Conceptually, an operating vehicle receives a small set of input values, like
parameters to a function component:

```text
linear_acceleration_request:  [x, y, z]
angular_acceleration_request: [x, y, z]
```

These are requests to accelerate linear and angular velocity. Direct position,
orientation and rotation overrides are deliberately excluded from this contract.
Linear axes are provisionally vehicle-local; angular axes are rotations around
those same axes. Station vehicles face local -Z, so forward acceleration has a
negative Z component. Sign conventions must be documented by the adapter.

The vehicle applies its own motion constraints. A simple ground vehicle might
accept linear Z and angular Y only. A hovercraft may also accept linear X; a
spacecraft may accept all six axes. A road vehicle can additionally apply its
own steering and traction rules: accepting yaw input does not require allowing
it to spin in place.

Units remain open. One candidate is normalized requests in [-1, 1], converted
by the vehicle into its own linear/angular acceleration limits. Another is
physical acceleration directly (metres/second squared and radians/second
squared). Choose one before implementing a shared consumer; do not silently
mix these interpretations. Per-axis clamping versus vector-length clamping is
also undecided.

## Input collection and consumption

```text
Keyboard / mouse / XR controller / regular gamepad
             -> raw device state -> driver mapping --+
                                                     +-> held vehicle command
NPC behavior ---------------------------------------+
                                                        -> vehicle behavior
                                                        -> native motion
```

Raw input stays available as device observations. The driver collects held
keys, stick axes, mouse deltas and button changes, applies its mapping and
deadzone, and writes the two command vectors. An NPC produces the same command
without synthesizing keyboard or XR events. Device bindings and NPC decisions
therefore stay outside the vehicle.

The command is held state, not an impulse applied once per device event. A
simulation step consumes the latest command and applies acceleration over its
elapsed time. The producer does not need to resend an unchanged value every
frame. The representation of this retained state and its update mechanism are
still provisional; no per-vehicle global subscription API is required.

Releasing input clears the corresponding acceleration request. Zero requested
acceleration means coasting unless the vehicle itself applies drag, friction or
another stopping rule. Braking behavior, speed limits and angular damping need
explicit vehicle rules; they are not yet an extra public command field.

Only one driver should supply the effective command at a time. Losing control,
switching driver or unloading a vehicle clears both requests so stale input
cannot continue accelerating it. Whether existing velocity should coast or be
stopped on those transitions is a separate vehicle/scene policy.

## Station experiment before any engine feature

Prototype the command state, driver mapping and vehicle behavior with ordinary
MMS objects/functions and existing Velocity. Use the same consumer first from
one player adapter and one NPC behavior. Keep the station's persistent input
controller rather than registering global callbacks on removable vehicle assets.

The current pickup throttle/steering/brake helper is an earlier prototype,
not an implementation of this two-vector contract. Its mount-gated heading
refresh and retained keyboard state must be addressed if it is adapted. The
motorcycle patrol is another producer to adapt, not a reason to invent a second
vehicle motion interface.

Before generalizing, verify that player and NPC commands produce equivalent
motion, unsupported axes are constrained, held inputs are independent of device
event frequency, and control handoff clears old requests. Verify acceleration
and damping across different update rates, plus removal while moving. Those
checks should exercise actual vehicle poses/velocities, not only script state.
