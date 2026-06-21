# Task: Mirror Camera Projection Debug Follow-up

## Status

Open investigation.

The current mirror bug is no longer best explained as "wrong source camera family" or "wrong
reflected camera position". Recent instrumentation shows that the reflected camera pose is correct
relative to the mirror plane for both centered and off-center viewer poses. The remaining work is
to debug the projection / render-path side of the mirror view.

Primary bug note:

- [docs/bugs/mirror-camera-orientation-and-tracking.md](../bugs/mirror-camera-orientation-and-tracking.md)

## What has been proved so far

The mirror system was instrumented to log:

- mirror plane origin
- mirror local basis
- source camera world position
- reflected camera world position
- source and reflected camera positions expressed in mirror-local coordinates
- source and reflected forward/up vectors

### Observation 1: plane reflection of the camera position is correct

For a vertical mirror at:

```mms
T.position(0.0, 0.55, -4.5).scale(3.0, 3.0, 0.08) {
    R.cube() {
        C.rgba(0.82, 0.88, 0.94, 1.0)
        Mirror.quality(1024) {}
    }
}
```

the logged plane origin was:

```text
plane_pos=[0.0, 0.55, -4.46]
```

This is correct for the cube's visible `+Z` face:

- transform origin `z = -4.5`
- half-depth `= 0.08 * 0.5 = 0.04`
- visible face `z = -4.46`

### Observation 2: mirror-local position reflection is correct

Centered sample:

```text
source_local=[0.0, -0.47000003, 4.14]
reflected_local=[0.0, -0.47000003, -4.1399994]
```

Off-center sample near the top-left of the mirror:

```text
source_local=[-0.9636997, 1.0908527, 0.70652556]
reflected_local=[-0.9636997, 1.0908527, -0.7065258]
```

In both cases the reflected camera obeys the correct planar-reflection rule in mirror-local space:

- local `x` preserved
- local `y` preserved
- local `z` negated

That means the current implementation is **not** failing because the reflected camera should have a
different `y` translation, nor because the system is reflecting the wrong position relative to the
chosen plane.

### Observation 3: reflected orientation is also plausible

The logged reflected forward/up vectors behave as expected for a vertical mirror:

- the reflected forward flips the component along mirror normal
- the reflected up stays upright

So the remaining bug is unlikely to be a simple "flip the angle differently" issue.

### Observation 4: square mirror size does not fix the bug

Changing the mirror to a square world-space size did not remove the issue.

That means the current failure is not explained by mirror aspect ratio alone, even though mirror
target extent/aspect is still worth inspecting.

## Current live symptoms

These symptoms remain after the above pose-math checks:

- the reflected face/head is still missing or misplaced when the viewer stands near the mirror's
  left/right edges and looks directly at it
- blue/green quads that physically touch the mirror edge do not continue seamlessly into the
  reflection unless the viewer gets very close
- the gap grows with viewer distance
- when the viewer gets close and looks at the mirror from an oblique angle, the reflected grid/world
  alignment breaks badly
- if the live camera is vertically aligned so the world grid cuts through the middle of the live
  view, the reflected grid appears elevated within the mirror image instead of at the matching
  reflected height

These symptoms now point more strongly at:

- reflected projection construction
- mirror render-target extent / aspect coupling
- mirror-pass viewport / NDC convention
- mirror shader UV / Y-flip conventions

than at the raw reflected camera position.

## Most likely remaining bug classes

### 1. Mirror projection does not match the visible mirror surface

`MirrorSystem` currently builds a symmetric mirror projection from mirror height, aspect ratio, and
perpendicular eye-to-plane distance. That is not the correct model for a viewer who is off-center
relative to the mirror.

This is especially suspicious because the current bug is most obvious:

- at the mirror edges
- when the viewer is off-center
- when the viewer is very close to the mirror

Those are all regimes where an off-axis frustum is required. A centered frustum can look roughly
acceptable near the middle while failing to keep edge-touching geometry connected as soon as the
viewer shifts in mirror-local `x/y` or looks from an oblique angle.

### 2. Mirror render target / viewport conventions are inconsistent with the projection

The renderer allocates a mirror target extent from the mirror's aspect ratio and then uses that as
the mirror pass viewport. If the reflected projection, offscreen extent, viewport orientation, and
mirror-surface sampling all disagree even slightly, the result can look roughly right near the
center while failing badly near the edges and at oblique angles.

### 3. Mirror shader UV conventions may still disagree with the mirror pass orientation

The mirror pipeline already applies UV flips in shader space. If the offscreen pass and the mirror
sampling shader compensate differently, the image can look "almost plausible" while still not being
the correct planar reflection.

## Recommended next instrumentation

The next debugging pass should stop asking only "where is the reflected camera?" and start asking
"where do known mirror-plane points land in reflected clip space?"

### Instrument the reflected projection path

For one mirror and one chosen eye, log:

- final reflected `view`
- final reflected `proj`
- final reflected camera world basis columns

### Project known world points through `view` + `proj`

Pick mirror-surface points in world space:

- mirror center
- left edge midpoint
- right edge midpoint
- top edge midpoint
- bottom edge midpoint
- optionally the four corners

Transform each point by:

```text
clip = proj * view * world_point
ndc = clip.xyz / clip.w
```

Log the resulting NDC positions.

### What that should prove

For a correct mirror-view setup:

- the mirror center should land near NDC center
- edge points should land near the expected NDC edges in a consistent way
- left/right/top/bottom should not be compressed, shifted, or elevated unexpectedly

If those points do not map consistently, the bug is in the mirror projection / viewport /
orientation path rather than in the reflected camera pose.

## Recommended manual debug trigger

The first-frame one-shot pose log was useful to prove the basic reflection rule. The next
projection-space instrumentation should be dumpable on demand from a clearly bad viewer pose.

Recommended temporary trigger:

- left-click dumps the current mirror debug sample

That allows capturing:

- near-center pose
- left/right edge pose
- top corner pose
- oblique close-up pose

without frame-by-frame log spam.

## Suggested order of work

1. Add projection-space logging for known mirror-plane points.
2. Capture dumps from:
   - a centered pose
   - a left/right edge pose
   - a close oblique-angle pose
3. Compare the resulting NDC coordinates against expected mirror-surface coverage.
4. If NDC coverage is wrong, inspect:
   - `MirrorSystem` off-axis frustum construction
   - mirror render-target extent policy
   - mirror viewport conventions
   - mirror shader UV/Y-flip behavior
5. Only after that, revisit oblique clipping.

## What should not be treated as the primary suspect anymore

Based on the current logs, these are less likely to be the root cause:

- wrong signed reflection distance from the mirror plane
- wrong preservation of mirror-local `x` / `y`
- a need to give the reflected camera a different `y` translation for a vertical mirror
- a simple "flip the reflected angle another way" fix

Those hypotheses were reasonable earlier, but the current instrumentation has substantially reduced
their likelihood.

## Related files

- [src/engine/ecs/system/mirror_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/mirror_system.rs)
- [src/engine/graphics/vulkano_renderer.rs](/home/rei/_/cat-engine/src/engine/graphics/vulkano_renderer.rs)
- [assets/shaders/mirror-mesh.vert](/home/rei/_/cat-engine/assets/shaders/mirror-mesh.vert)
- [assets/shaders/mirror-mesh.frag](/home/rei/_/cat-engine/assets/shaders/mirror-mesh.frag)
- [docs/bugs/mirror-camera-orientation-and-tracking.md](/home/rei/_/cat-engine/docs/bugs/mirror-camera-orientation-and-tracking.md)
