# Vehicle behaviors

Status: implemented V1, 2026-09-14.

This document specifies the current behavior exports in
assets/components/vehicles/display_car.mms. It does not specify the future
normalized non-tracked locomotion event; that remains a proposal.

## Exports

~~~mms parse-only
display_car(root_name, position, yaw, mount_anchor_name, extra_children)

display_car_desktop(root_name, position, yaw, mount_anchor_name)

display_car_xr(root_name, position, yaw, mount_anchor_name, xr_gamepad)
~~~

display_car(...) is the spatial fixture. It creates the car GLB, entry zone,
mount anchor, and dismount anchor; callers may provide additional children.

display_car_desktop(...) creates the same spatial fixture plus the mounted
desktop behavior:

- W/S are forward/reverse throttle; A/D steer.
- Space fires the vehicle laser once per physical key-down edge.
- Input is observed through global keyboard signals and retained as held state.
- The wrapper drives/fires only between its car root's MountStarted and
  MountEnded events.
- It clears retained key state on either mount lifecycle event.

display_car_xr(..., xr_gamepad) creates the same spatial fixture plus the
mounted XR behavior:

- LeftStick supplies analog steering/throttle after the 0.16 radial deadzone.
- Right grip plus right trigger fires the vehicle laser.
- Input is observed from the supplied InputXRGamepad component.
- The wrapper drives/fires only between its car root's MountStarted and
  MountEnded events.
- It clears retained stick/grip state on either mount lifecycle event.

## Shared behavior

Both controlled constructors add the same car-local behavior:

- planar transform-driven motion at 5 units/second and 1.25 radians/second;
- retained vehicle position/yaw, with the authored initial position and yaw as
  the initial state;
- late GLTF-bounds measurement to place the muzzle on the car model;
- a replayable 0.22-second muzzle-flash and layered beam animation along the
  car's semantic local -Z front.

The mount system remains responsible for suppressing/restoring the rider's
automatic pedestrian movement mapping. These wrappers do not manipulate Input
or InputXRGamepad automatic-locomotion state themselves; they consume raw
keyboard/XR observations that remain available while mounted.

## Scope and limitations

The desktop keyboard adapter is global and assumes one local desktop driver.
The XR adapter is component-scoped. They are intentionally separate public
constructors in V1.

This API does not provide regular desktop-gamepad support, generic input
routing, multiplayer/device selection, physics, collision, haptics, or a live
transform-forward-direction API.

## Verification

The mittens-corp focused tests verify the two examples continue to load,
desktop mounted WASD moves the car, and the XR car retains its mounted
driving/laser behavior.

## Related design work

- [Vehicle behavior module draft](../draft/vehicle_behaviors.md)
- [Non-tracked locomotion actions and vehicle behavior assets](../task/non-tracked-locomotion-actions-and-vehicle-behaviors.md)

