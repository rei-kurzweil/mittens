# Display car / mech physics and destruction

2026-11-09

The shared display-car factory now returns a native Velocity wrapper around
the named body transform. It owns always-active gravity and eight rounded slide
zones targeting that body. All examples using car.glb now use this factory,
including both subjects in the studio car example; the studio surfaces also
have static collision. Plain fixtures have physics without driving callbacks.

Paper-craft uses the XR driving wrapper. Grip the entry region to mount, then
click the cockpit POWER switch. Left stick drives/steers, right grip + trigger
fires. Desktop wrappers use WASD and Space. Power starts off. Power and mount
state create control callbacks only while powered and occupied; switching off
or dismounting removes those handlers and clears input latches while preserving
physical velocity. The standalone studio uses the same desktop controller, with its cockpit
considered occupied from scene startup.

Translation now belongs to native Velocity rather than FrameTick pose writes.
Steering refreshes the heading-relative drive command and preserves vertical
velocity. No spawn-height reset can overwrite floor or wall correction. The
native solver already accumulates corrections for compound slide zones sharing
a movement target; no solver changes were required.

The hull is deliberately coarse and upright: two wheel-bottom spheres, five
overlapping body spheres, and a roof sphere. Wheel bottoms are calibrated at
asset-local Y=0.04635. These proxies support yaw, bounded floors and walls;
they do not supply suspension, roll/pitch, vehicle-to-vehicle rigid-body
response, pedestrian support carry, or a detailed triangle hull. Thin-wall
horizontal tunnelling at high speeds remains an existing solver limitation.

## Original unsplit asset: destruction audit

This table records the original asset, now backed up at
`data/car-8-production-export/car-before-split.glb`. Reproduce with
`python3 scripts/inspect_car_geometry.py data/car-8-production-export/car-before-split.glb`.
Counts use exact
position welding, matching the solid import boundary, without approximate
repairs. Every primitive has POSITION, NORMAL and TEXCOORD_0, and all are rigid
with no skinning or morph targets.

| Node | Triangles | Degenerate triangles | Boundary edges | Non-manifold edges | Inconsistent two-face edges |
| --- | ---: | ---: | ---: | ---: | ---: |
| vehicle_body | 24,632 | 2 | 90 | 328 | 2 |
| wheel_right | 640 | 0 | 128 | 0 | 0 |
| wheel_left | 640 | 0 | 128 | 0 | 0 |
| dashboard | 60 | 0 | 0 | 2 | 0 |
| seat | 24 | 0 | 0 | 0 | 0 |
| steering_wheel | 512 | 0 | 128 | 0 | 0 |

The original unsplit main body cannot be destroyable. It exceeds both the imported
destruction adapter and CSG importer's 16,384-triangle limit, and its topology
also fails solid import. Raising the cap alone will not fix it. Author closed,
outward-wound body sections below the budget, removing degenerate faces,
closing intended volumes and splitting non-manifold junctions. Wheels and
steering wheel need closed volumes; the dashboard needs manifold repair.
The seat passes this topology audit, but enabling only seat destruction would
not make the mech body destroyable.

## Current split export

At the user's request, `assets/models/car.glb` now contains the five-piece
split of `/home/rei/assets/3D/car-8.blend`. Both wings pass solid import and
boolean subtraction; roof, legs and center remain invalid despite meeting the
triangle budget. See the [cutter report](blender-vehicle-cutters.md) for geometry
counts and reproduction instructions. Existing material overrides, slide
proxies, controls and mount anchors continue to apply to the shared GLTF.

The shared factory now attaches `Destroyable.threshold(3.0)` directly to each
wing's imported renderable after `GLTFInitialized`. Roof, legs, center and
accessories are excluded. Damage is independent per wing and per vehicle.
Existing paper-craft pistol and mounted-car laser hit callbacks emit one damage
unit per hit with a 0.28 m radius. Reload the scene and land three shots in the
same wing area to cut it. The vehicle need not be powered or occupied to take
damage. The coarse movement hull is separate from the damaged visual surface.

## Verification

Automated tests cover power/mount gating, momentum and falling while off,
wall sliding with tangent velocity retained, four headings at two drop heights,
support removal, scene-wide car collision coverage, and existing mounted
desktop/XR driving and laser behavior. Headset testing is still needed for
switch reachability and subjective collision-hull fit.

`cargo test --test display_car_imported_destruction` runs the real laser ray and
MMS `Damage.at` path against both wings, checking the damage threshold, precise
holes, per-wing/per-instance isolation, and replacement meshes following motion.
It also checks that surviving vertices in the uploaded cut-wing mesh retain
their original atlas UVs. The solid kernel carries UVs across splits without
welding UV seams; new cut faces and debris interiors use a stable texture
sample from the source primitive. Existing procedural solids retain their
generated planar mapping.
