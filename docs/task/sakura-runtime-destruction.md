# Sakura runtime surface destruction

Implemented 2026-10-31. This is the static-box runtime slice of
[the damage proposal](damageable-surfaces-csg-and-fragments.md). Authored general
`CSG { Subtraction { ... } }` expressions remain a separate, unimplemented API.

## Behavior and authoring

Place `Destroyable.threshold(3.0).cell_size(0.12).chunk_size(1.0)` directly under a
transform with one sibling `R.cube()` and one sibling static `Zone.cube([0.5,
0.5, 0.5])`. Use a constructor call for independent markers in repeated prefabs.
For a whole section, put the marker on its parent transform instead. Descendant
cube/box pairs inherit its settings; a piece-local marker overrides them. Any
CombineMesh batches must use keep_transforms so their sources remain available.
Imported/skinned meshes are not cut.

`Damage.at(x, y, z, radius, amount)` is a one-shot CPU impact volume. Sakura emits
one radius-0.28, amount-1 event from each accepted pistol impact callback; the
expanding visual explosion does not emit repeated damage. Every overlapping,
opted-in surface receives the event. Cans keep their existing reaction/physics
callbacks and have no Destroyable marker.

Damage is a sparse CPU map per surface, indexed in stable primitive-local cells.
Its spacing is established when first affected, including cells for thin slabs.
There is no decay or damage persistence. Successful cuts clear covered damage
entries. Each affected source uses its own state even when sharing a cube asset.
The surviving mesh records holes; clearing damage cells cannot heal geometry.

At threshold, the source is lazily partitioned into a stable local grid of roughly
one-metre cells. Intact cells share the existing cube GPU mesh and primitive
collision path. Only touched cells enter the boolean kernel. Cuts crossing cell
boundaries update all affected cells together. Detailed cells share a local
triangle BVH between rendering/ray queries and their collision zone; world
collision BVHs are retained until the transform changes.

The backend is `manifold-rust = 0.19.1`, defaults disabled, behind `csg_mesh`.
It supplies union, subtraction and intersection without the earlier csgrs
resolution blocker or a C++ toolchain. A bounded worker serializes each surface's
cuts, unions removed material across touched cells and slices it into up to three
closed debris solids. Render uploads complete before replacing visual, raycast,
bounds and collision state. Failed calculations/uploads retain previous geometry.

Debris has independent world placement, initial velocity, gravity and a four-second
lifetime. This slice uses visual falling debris: spin, bouncing and debris contact
response remain future work. Bodies and shots can pass through the static holes;
contact and downward support use the surviving triangles, including hole rims.

Each storey and roof of all nine Sakura buildings owns an aggregate Destroyable.
Its white/trim CombineMesh.keep_transforms batches initially draw the intact
floor. Derived, unsaved piece markers inherit the floor's settings; each source
cube shares its transform with its own box collider. The first successful cut
unpacks both batches in that section, revealing the intact pieces and generated
cut cells together. Sub-threshold hits, pending cuts and failed uploads leave
the intact batches visible. Untouched sections in every building stay combined.
Activated sections remain unpacked; structural support/collapse is not simulated. Both
Sakura desktop and XR scripts opt in the backing wall and layered tabletops.
Existing custom materials are reused; no renderer/material pipeline is added.
Pool tiles use world-space projection. General attribute/UV preservation is not
promised: generated mesh faces use flat normals and planar UVs.

## Resource policy

- Maximum 512 affected surfaces, 4,096 sparse damage cells per surface.
- Maximum 512 grid chunks per surface and 4,096 chunks across the scene.
- Maximum 16,384 triangles per boolean solid and 262,144 detailed surviving
  triangles across the scene. Budget failures preserve existing geometry.
- Eight queued worker jobs and eight completed results; 32 pending impacts per
  surface. Queue backpressure retries a cut without counting its damage twice;
  excess pending impacts or newly affected surfaces at capacity are dropped.
- Maximum 96 debris objects. Oldest debris retires first when capacity is reached.
- Removed owners release retained maps, geometry and render instances; complete
  destruction releases the surface state and records an exhausted runtime marker.
  Expired debris releases its meshes. Runtime CPU mesh slots are recycled; desktop
  Vulkan and the Android renderer retire owned GPU resources. Android does this
  between its existing synchronous frames, after submission completion.
- Derived chunks, debris and source suppression are excluded from filtered scene
  saves; the authored source remains intact. Reloading resets destruction.

## Validation and measurement

Focused tests cover volume conservation, repeated/disjoint/full cuts, independent
instances, thin surfaces, cross-chunk holes, three debris objects per cut, failed
upload retry, owner/debris cleanup, exhaustion without resurrection, filtered
serialization, precise rays, capsule sweeps, box contact and hole-rim support.
Existing static contact, zone, raycast and Sakura weapon tests are also exercised.

`cargo run --manifest-path android/validation/renderer-smoke/Cargo.toml -- --destruction`
checks an actual GPU render through the Android renderer. A green wall must expose
a red object through its hole while retaining the green rim. It writes before/after
images to `/tmp/mittens-destruction-{before,after}.ppm`. Use
`--destruction-groups` to check section activation, two retained material batches,
and an untouched floor that stays combined. This host GPU check is not
an XR2 device performance result.

`cargo run --release --example profile-csg` compares 128 cuts in an 8×8 slab with
one growing solid versus retained 1×1 cells. It includes booleans, two-piece debris
and mesh export; it excludes engine scheduling, upload, collision and drawing.
On the development host: whole-solid p50/p95 6.06/8.51 ms, chunked 1.16/1.26 ms;
final maximum target complexity 8,204 versus 140 triangles. Benchmark the complete
three-fragment pipeline and frame time on XR2 before claiming a device budget.
