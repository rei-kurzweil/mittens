# Implicit surfaces: coarse voxels cut off thin rims, and chunks fail at shared edges

Date: 2026-11-26. Found building `examples/bliss.mms` (since moved to plain
sphere meshes).

## Symptoms

1. **A hard step around wide, gentle shapes.** A large sphere sunk into flat
   ground so only a low cap shows (rolling hills: 140 m base, 6.6 m tall) is
   drawn with a sharp vertical edge 0.5–1 m above the ground instead of
   meeting it.
2. **Missing chunks.** Splitting one field into side-by-side
   `ImplicitSurface`s (four 64 m chunks sharing inner edges, all spheres in
   each) bakes nothing for chunks whose shared edges pass through a hill, leaving
   holes. It coincided with very low frame rates, possibly from bakes being
   retried; not confirmed.

## Causes

1. `MAX_CELLS_PER_AXIS` is 128 and `voxel_size` is one value for all three
   axes, so a 280 m wide field needs voxels of about 2.3 m, also vertically.
   With `clip_floor(-0.08)` the bottom sample row (at the bounds' floor, below
   the clip) is outside. Near the rim the solid between the clip floor and the
   cap is thinner than one voxel, so no grid edge crosses it until the cap rises
   above the next sample row. Marching cubes then starts the surface there, as a
   step (`sample_and_extract_clipped`, `implicit_surface_system.rs` ~L570-620).
2. Every boundary sample must be outside the field, or the bake fails with
   "field reaches the sampling boundary" (`implicit_surface_system.rs:619`).
   A chunk edge inside a hill always violates that, so chunked terrain can't
   share edges through solid.

Separately, bakes read cached `matrix_world`s (`TransformSystem::world_model`).
If a surface bakes before its transforms first propagate, its spheres sit at
the surface origin. The fingerprint includes sphere placement, so the next frame
rebakes, but the first frame can be wrong.

## Possible fixes

- Per-axis voxel sizes (or cell counts), so wide, low fields keep fine
  vertical resolution.
- Treat the clip floor analytically, closing the surface at the clip plane
  instead of sampling it, so sub-voxel slabs near the floor still extract.
- Allow fields to touch their bounds on chosen faces (for example "open" sides
  for terrain chunks), producing an open mesh there instead of failing.
- Workaround today: keep fields small enough for fine voxels, extend the solid
  well below a clip floor hidden under other ground, and keep chunk boundaries
  outside the solid.
