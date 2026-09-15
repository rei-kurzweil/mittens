# Vehicle behavior module design draft

Status: the one-module/two-export V1 extraction is implemented, 2026-09-14.
This draft now records the remaining question of whether those two public
constructors should converge on one export.

## Question

`mittens-corp` and `mittens-corp-desktop` implement the same mounted display
car: it drives, fires a visual laser, and only responds while mounted. Their
input adapters differ, but the mounted lifecycle, transform integration, muzzle
placement, and animation are substantially the same.

Should V1 create separate desktop/XR behavior files, or keep the spatial prefab
and two explicit input-specific behavior exports in one car module? This is not
a request to solve movement authority or general input routing yet.

## What is shared today

```text
MountStarted / MountEnded
        |
        +-- reset mounted state and active command
        |
FrameTick
        |
        +-- integrate steering + throttle into the car transform
        +-- keep motion planar and retain the car's yaw state
        |
vehicle fire action
        |
        +-- play the bounds-positioned muzzle flash + beam animation
```

The car-root `MountStarted`/`MountEnded` handlers, motion update, model-relative
muzzle placement, and laser animation do not differ between desktop and XR.

Only command acquisition differs:

| Concern | Desktop | XR |
| --- | --- | --- |
| Continuous command | retain W/A/S/D down/up state | retain `LeftStick` value |
| Fire edge | global `KeyDown`, `code == "Space"` | XR button/chord event |
| Signal scope | scene-global keyboard signal | supplied `InputXRGamepad` |
| Command values | synthesized digital steering/throttle | native analog `Vec2` |

So yes: extracting shared code should mainly change the examples to import a
module, pass the car root, and select an input adapter. Mount lifecycle events
do not force two behavior modules.

## MMS constraint

MMS modules may own global and component-scoped handlers, but one subscription
cannot be polymorphic across these unrelated signals:

```text
desktop: on_global("KeyDown", ...)
XR:      on(xr_gamepad, "XrAxisChanged", ...)
```

They have different scope, payload, and lifetime. A one-file solution must
make the distinction visible as separate exports or an explicit control-mode
branch; it cannot claim that `Input` already emits XR-style axis events.

## Candidate V1 layouts

### A. Two files, one adapter per file

```text
assets/components/vehicle_behaviors/car.mms
assets/components/vehicle_behaviors/car_xr.mms
```

This makes the source difference explicit but duplicates shared controller and
laser setup unless a third private module is introduced. Keep it as a fallback;
do not create this directory yet.

### B. One behavior module with two exports

```mms parse-only
// assets/components/vehicle_behaviors/car.mms
export fn attach_desktop_car_behavior(car_root) { /* ... */ }
export fn attach_xr_car_behavior(car_root, xr_gamepad) { /* ... */ }
```

Each export owns only its adapter, then invokes the same private car setup:
mount lifecycle, persistent state, `FrameTick` integration, muzzle placement,
and laser animation.

```mms parse-only
let car_root = display_car(...)
attach_desktop_car_behavior(car_root)

// or
attach_xr_car_behavior(car_root, vehicle_controls)
```

This is the strongest V1 candidate if behavior extraction happens before a
normalized input event exists. It is honest about the adapters without
duplicating the car implementation.

The exact attach/return shape needs a small feasibility prototype: the module
must attach its laser subtree to `car_root`, retain references to the animation,
and share state among its handlers. MMS has those ingredients individually, but
this API has not yet been proven as a module boundary.

### C. `display_car.mms` exports optional behavior attachments

The car's laser depends on its semantic local `-Z` front and `car_model` child,
so this single-module alternative is coherent:

```mms parse-only
// assets/components/vehicles/display_car.mms
export fn display_car(...) { /* spatial prefab only */ }
export fn attach_desktop_behavior(car_root) { /* ... */ }
export fn attach_xr_behavior(car_root, xr_gamepad) { /* ... */ }
```

`display_car()` remains usable without behavior attached. The downside is that
a spatial prefab module now contains desktop and OpenXR policy. That is
acceptable only while the exports remain explicit and optional; a separate
behavior module will age better if more vehicle types gain controllers.

### D. Input-neutral behavior core plus scene adapters

```mms parse-only
let commands = attach_car_behavior(car_root)
on_global("KeyDown", fn(event) { /* write commands */ })
on(xr_gamepad, "XrAxisChanged", fn(event) { /* write commands */ })
```

This is the cleanest core but leaves adapters in scenes until a common action
surface exists. It also needs a deliberate MMS API for returning/exposing a
retained command-state handle. Treat it as a later refinement.

## V1 result and next question

The implementation selected candidate C's module location with explicit
constructors rather than attachment exports:

```mms parse-only
display_car(...)
display_car_desktop(...)
display_car_xr(..., xr_gamepad)
```

The shared implementation now lives in `display_car.mms`; each public wrapper
only selects the desktop or XR adapter. Its current contract is recorded in
[Vehicle behaviors](../spec/vehicle_behaviors.md).

The next design question is whether one public export can replace the two
controlled constructors without hiding their different signal scopes or
inventing a weak stringly-typed control-mode API. A normalized non-tracked
action source is the preferred route for that later convergence.

## Future normalized actions

A future non-tracked `Locomotion2DChanged` event can collapse the two adapters
to one public behavior entry point. That is valuable but not required for the
V1 experiment: the explicit adapters can write the same internal steering,
throttle, and fire command state.

That future action should use a real `Vec2`, rather than overloaded array
arithmetic. See [Non-tracked locomotion actions and vehicle behavior assets](../task/non-tracked-locomotion-actions-and-vehicle-behaviors.md).

## Non-goals

- no generic movement-authority stack or input routing;
- no regular desktop gamepad provider;
- no change to tracked-pose input;
- no vehicle physics, collision, force integration, or haptics;
- no claim that global desktop keyboard routing is solved;
- no change to `display_car` or either example in this draft.
