# Desktop shaders tilt normals on non-uniformly scaled meshes

Date: 2026-11-26. Found with the sphere hills in `examples/bliss.mms`.

## Symptom

A curved mesh scaled non-uniformly is lit as if its surface were tilted the
wrong way. Bliss's hills (a sphere mesh scaled about 10x wider than tall) lit
like steep slopes and showed no grass tufts, which the grass shader only draws
where `N.y > 0.6`. Boxes don't show it: their face normals stay axis-aligned.

## Cause

The desktop vertex shaders transform normals by the model matrix:

```glsl
v_normal = normalize(mat3(model) * in_normal);
```

That is only correct for rotation and uniform scale. Normals transform by the
inverse-transpose; under a scale of (sx, sy, sz) the model matrix scales a
normal by (sx, sy, sz) where it should scale by (1/sx, 1/sy, 1/sz), so a
wide, low dome's normals lean toward horizontal instead of toward vertical.

Affected: `assets/shaders/toon-mesh.vert`, `mirror-mesh.vert`,
`toon-outline.vert`, `cached-skinned-toon-mesh.vert`,
`cached-skinned-toon-outline.vert`. The Android renderer's
`android/shaders/surface.vert` already uses
`transpose(inverse(mat3(model)))`.

## Workaround today

Bliss's hills use `cartoon_grass_dome(bucket, center, radii, options)`
(`assets/shaders/cartoon-grass-dome.frag`), whose fragment shader computes the
exact ellipsoid normal from world position. Custom materials have no
`.vertex(...)` hook, so per-material fixes must work from the fragment side.

## Fix options

1. **Cofactor normals in the vertex shaders.** The cofactor matrix of
   `mat3(model)` (columns `cross(m1, m2)`, `cross(m2, m0)`, `cross(m0, m1)`)
   equals the inverse-transpose up to the determinant; multiply by its sign to
   keep mirrored transforms facing out. About 20 extra multiply-adds per vertex
   (three cross products and a dot), no CPU or buffer changes, and identical
   output for uniformly scaled objects. Tried and reverted on 2026-11-26 to keep
   the shared shaders unchanged until decided; all five compiled.
2. **A normal matrix per instance.** Compute it on the CPU next to the model
   matrix and upload it. Cheaper per vertex, but adds per-instance data and
   upload work for every draw.

## Validation

Compare a uniformly scaled scene before and after (output should be
identical), then a flattened sphere and a stretched capsule lit from the side.
Measure vertex-heavy scenes (avatars, Sakura Twilight) on the GTX 1080 and
the Focus 3.
