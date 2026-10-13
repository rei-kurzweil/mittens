# Task: Shared display-car floor contact and gravity

Date: 2026-10-13.

Status: proposed after source/asset investigation; no runtime changes made.

## Outcome and scope

The reusable display car rests on actual static floor support, falls when
support ends, and carries its mounted rider with the corrected car pose.
Implement the behavior in the shared vehicle asset so XR and desktop callers
inherit the same floor policy.

The first slice concerns **the car contacting the world floor**. Making an
unmounted avatar walk on the car or collide with its cabin is a separate moving
surface/body-contact feature, discussed below. Wheels, suspension, tipping,
ramps, momentum, and a general rigid-body solver are later work.

## Current behavior

[display_car.mms](../../assets/components/vehicles/display_car.mms) creates a
GLTF, entry zone, mount anchor, and dismount anchor. The entry zone is an
interaction region; it has no `Collidable` and is not a vehicle collision hull.
There is no car-owned `Velocity`, `Gravity`, or blocking contact proxy.

The controlled wrapper retains position/yaw in a script table and writes
`car_root.update_transform(...)` during mounted driving. Every movement update
sets Y to the constructor's original `position[1]`. Merely adding contact would
allow that script to overwrite its vertical correction; retaining old X/Z can
also overwrite future wall corrections.

All current shared-asset callers are:

- [mittens-corp.mms](../../examples/mittens-corp.mms), XR;
- [mittens-corp-desktop.mms](../../examples/mittens-corp-desktop.mms);
- [mittens-corp-agc.mms](../../examples/mittens-corp-agc.mms), XR;
- [toon-outline-bisket.mms](../../examples/toon-outline-bisket.mms), XR.

The plain `display_car(...)` spatial factory is also exported. Preserve its
documented control-free use and make the new physics/default policy explicit.
`mittens-corp-agc-desktop.mms` is not a current display-car caller.

The player's existing floor collider cannot ground the car. Mounting suspends
the pedestrian Velocity and locomotion mapping, reparents the rider root, and
StaticContactSystem excludes that mounted movement root. Car contact must
target the car's own movement transform, which remains eligible while occupied.

## Existing engine seams and limitations

- `Gravity -> Velocity -> T` already supplies world-down acceleration and
  fixed-step integration. `Velocity` directly owns its immediate child mover.
- `Zone { Collidable.slide().movement_target(...) }` supplies mover geometry.
  `Collidable.static()` supplies blocking floor geometry. StaticContactSystem
  corrects the selected transform and removes inward velocity only from the
  Velocity directly driving it. Do not use the removed `CollisionResponse` API.
- The contact solver accepts upright capsules/spheres as yaw-invariant movers.
  Arbitrarily yawed moving cubes are rejected by `axis_aligned_world_shape`;
  support for yawed **static** boxes does not provide yawed vehicle-box response.
- Downward floor sweep currently supports a capsule against an upright box.
  General car hull sweeps, pitched/rolled contact, and wheel support are absent.
- Multiple independent slide proxies targeting one root need a coordination
  review: the existing pass does not propagate the root after each proxy before
  reading the next proxy. Do not bolt on four wheel responders and assume their
  independent corrections compose correctly.

See [StaticContactSystem](../../src/engine/ecs/system/static_contact_system.rs),
[collision geometry](../../src/engine/ecs/system/collision_geometry.rs), and
[zone queries](../../src/engine/ecs/system/zone_query.rs).

## Model/floor placement findings

Read-only inspection of `assets/models/car.glb`, composing node TRS and
transforming POSITION accessor bounds, gives approximate asset bounds:

```text
min = [-4.399903, 0.046352, -1.778936]
max = [ 4.399903, 5.566735,  2.745441]
```

These bounds establish approximate origin/size, not a wheel-contact hull.
Measure wheel/underbody contact landmarks explicitly; aggregate bounds can
include decorative geometry. The current Corp car origin Y=-0.75 puts its
lowest aggregate point near Y=-0.70365, about 14.6 cm above the studio-floor
top Y=-0.85. If that point were the desired support bottom, settling would put
the origin near Y=-0.89635. This is an asset-placement observation, not a
measured in-headset result or a proposed hard-coded final height.

The stage top is Y=0.12. Contact must use bounded support geometry, so driving
away from the stage can fall toward the studio floor instead of staying at one
global Y. Walls and steps require an explicit policy; floor support alone does
not imply the vehicle can climb stairs.

## Implementation slices

1. **Choose a supported first proxy.** Author/calibrate a single yaw-invariant
   proxy for a deliberately coarse floor-contact fixture, documenting footprint
   and wheel-clearance errors. An upright capsule can reuse the downward floor
   sweep, but a large radius can misrepresent this wide car. If an accurate
   rectangular footprint is required, implement a focused upright yawed-box
   mover/contact slice before using that hull. Do not silently approximate a
   rotating car with an oversized world AABB or use avatar capsule defaults.
2. **Give car Y one motion owner.** Add car-owned Velocity/Gravity and route its
   slide contact to the immediate driven car movement root. Keep GLTF, entry
   zone, anchors, laser, and mounted rider beneath that corrected root. Decide
   whether empty cars continue settling; recommended first behavior is gravity
   while unoccupied as well as occupied, with controls gated by mount state.
3. **Remove the spawn-Y reset and stale position writes.** Keep the existing
   steering/input adapter, but derive pose-driven X/Z from the latest accepted
   transform while Velocity owns Y, or migrate translation commands to the car
   Velocity with a documented speed/braking policy. Preserve contact corrections
   across the next input frame. Keep one owner per channel and explicit flush/
   propagation boundaries; expose yaw on a separate child if needed by Velocity.
4. **Bounds/readiness.** Use authored proxy dimensions or wait for usable GLTF
   CPU geometry before enabling gravity/contact. Gate car readiness independently
   of AVC's `CapsuleReady`. Record effective scale and supported transforms;
   pending geometry must not make the car fall through the floor.
5. **Mount and scheduling integration.** Verify car contact remains active while
   the rider's pedestrian contact is excluded. The corrected car must carry its
   anchors, rendered eyes, controllers, and rider in the same presentation frame.
   Keep mount/dismount authority and pedestrian Velocity restoration intact.
   Reset stale sweep history after explicit teleports/proxy replacement.
6. **Shared callers and diagnostics.** Put the policy in the shared factory or
   one shared physical wrapper rather than copying physics into each scene.
   Expose car-root Y, vertical speed, proxy/support gap, support identity,
   grounded state, and contact corrections for tuning.

## Avatar inside/on the car

Mounting currently aligns the rider anchor with the cockpit anchor; it does not
solve feet against a cabin floor or animate a seated lower body. Correcting the
car's world-floor contact will not fix cabin foot penetration or standing/kneeling
calibration. See [AVC standing/sole calibration](avc-vr-standing-height-and-sole-floor-calibration.md).

An unmounted pedestrian could be blocked by deliberately authored static zones
on a parked car, but a driving car needs relative-motion/contact and support
carry policy. Slide-versus-slide body response and moving-platform support are
not current guarantees. Decide that scope separately before treating moving
car geometry as an ordinary static floor. The existing
[surface contact and coupled motion task](surface-contact-and-coupled-motion.md)
is the related architecture work.

## Acceptance

- Drop an empty and occupied car from at least two heights; both settle with
  near-zero vertical velocity and calibrated proxy-bottom error within 1 cm on
  the studio floor, without persistent jitter or a duplicate responder.
- Hold throttle and steer through arbitrary yaw while grounded; Y corrections
  survive later FrameTick writes. Test backing up and releasing controls too.
- Leave bounded raised support and verify falling resumes. Test the actual
  selected proxy's fast-fall sweep coverage; unsupported footprint, stairs,
  walls, or slopes remain explicitly documented.
- Mounted pedestrian input/contact/Velocity stay suppressed; car physics stays
  active. Dismount/remount restores walking and grounding without stale speed,
  duplicate helpers, or resetting the car to its authored spawn height.
- Verify all four current shared-asset scenes load and retain drive/fire/mount
  behavior. Include a test that advances actual frame/contact phases during
  driving, so a stale scripted Y reset fails deterministically.
- Headset acceptance checks rider/camera motion and current wheel/floor
  clearances. This investigation did not perform an XR smoke test.

## Related work

- [Current vehicle behavior spec](../spec/vehicle_behaviors.md)
- [Mounted controls and laser first slice](mittens-corp-mounted-vehicle-controls-and-laser-first-slice.md)
- [Gravity and acceleration velocity drivers](gravity-and-acceleration-velocity-drivers.md)
- [Rider/Mountable attachment](rider-mountable-attachment-system-first-slice.md)
