# Vehicle cutter trial: car-8.blend

Run the splitter with Blender 4.5.5 LTS:

```sh
blender --background --factory-startup --disable-autoexec --python-exit-code 1 \
  /home/rei/assets/3D/car-8.blend \
  --python scripts/blender_split_vehicle.py -- \
  --output /tmp/car-8-split.blend --export-glb
```

For Blender's Text Editor, open `scripts/blender_split_vehicle.py` and Run
Script. With no command-line arguments, it saves a sibling `*-split.blend`
and a JSON report. Save the source first. Existing output files are rejected
rather than overwritten. Open the original source again for another run.

The checked trial is in `data/car-8-split/` (local, Git-ignored), with the
Blender file, candidate GLB, JSON reports and assembled/exploded PNG previews.
The original `/home/rei/assets/3D/car-8.blend` is intact. The splitter was then
rerun from that source into `data/car-8-production-export/`, and its GLB replaced
`assets/models/car.glb` at the user's request. The previous production asset is
backed up as `data/car-8-production-export/car-before-split.glb`; `installed.json`
records the old and new SHA-256 hashes. Production includes all five body pieces
and the original accessories, with no cutter meshes or hidden unsplit body.
The shared display-car factory now marks the two validated wing primitives
destroyable; the other pieces remain excluded pending geometry repair.

## Cutter convention

The source object is `vehicle_body`. Each cutter is a separate, unmodified
quad lying in its object's local XY plane. The displayed local +Z axis points
toward the extracted piece. Quad size is a visual guide; cuts use an infinite
plane through the quad's center. Object translation, rotation and negative
scale are supported. A zero scale, tilted quad within the local coordinates,
or a warped quad is rejected. Under shear, the plane normal is calculated with
the inverse transpose so that it stays perpendicular to the plane while still
choosing the local +Z half-space.

Default extraction order:

1. `cut_roof` -> `vehicle_body_roof`
2. `cut_legs` -> `vehicle_body_legs`
3. `cut_wing_left` -> `vehicle_body_wing_left`
4. `cut_wing_right` -> `vehicle_body_wing_right`
5. Remaining geometry -> `vehicle_body_center`

All cuts operate on the remaining center. Use `--cuts NAME NAME ...` to change
the order or provide another list. Every `cut_` object must be accounted for.
The original source remains hidden in the output, and generated objects live
in `VEHICLE_PARTS`. Wheels, seat, dashboard and steering wheel stay separate.
Generated object colors distinguish regions in Solid viewport mode; material
slots are preserved for export.

The script welds exact duplicate source positions before cutting, retaining
per-loop UV seams; it does not weld nearby vertices across real gaps. This
trial welded 320 source vertices. Both sides of each cut get triangulated caps
from cut-boundary edges, using Blender's
[BMesh bisect and triangle-fill operations](https://docs.blender.org/api/4.5/bmesh.ops.html).
Nested contours can preserve through-holes. Cap UVs are planar in world units.
Caps use the original material, because another material would export them as
separate open GLB primitives. Multiple source materials currently require an
explicit policy and are rejected.

## Actual result

| Piece | Exported triangles | Exact-weld boundary edges | Non-manifold edges | Engine solid import |
| --- | ---: | ---: | ---: | --- |
| Roof | 4,384 | 74 | 120 | Fails: degenerate triangle |
| Legs | 1,895 | 8 | 58 | Fails: degenerate triangle |
| Left wing | 6,382 | 0 | 0 | Passes; boolean subtraction verified |
| Right wing | 6,244 | 0 | 0 | Passes; boolean subtraction verified |
| Center | 8,102 | 128 | 180 | Fails: degenerate triangle |

All five pieces meet the 16,384-triangle source budget. Both wings also have
consistent winding and positive closed volume. Native Solid import accepted
them, and subtracting a box reduced their volumes from 8.785294 to 8.725803
and from 8.634218 to 8.570218 respectively.

The roof cut intersects broken source contours: 56 cut-boundary edges remain
unsealed on each initial half. The other cuts seal their new boundaries, but
legs and center still contain defects from the source and problematic cap
triangulation around those junctions. Existing degenerate triangles and
non-manifold/open geometry are not silently repaired. The script reports
partial capping and failed topology checks as warnings and saves them in JSON.
`passes_engine_topology_checks` is a preflight result, not a replacement for
the engine's solid-kernel validation.

## Verification

```sh
blender --background --factory-startup --python-exit-code 1 \
  --python scripts/test_blender_split_vehicle.py
python3 scripts/inspect_car_geometry.py data/car-8-split/car-8-split.glb
```

Four Blender tests cover the five-region split, closed caps and volume
conservation; local +Z under negative scale and offset quad geometry; a cap
with a nested hole; and exact seam welding that preserves UV discontinuities
without merging near-coincident geometry. The actual exported GLB was also
audited per primitive and checked with the engine's `Solid::from_mesh` and
boolean subtraction. Cycles CPU renders verify assembled and exploded layout.
