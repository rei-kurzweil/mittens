# Mirror Component Camera Math

This document is the reference for how planar mirror camera matrices should be derived in
`cat-engine`.

It is intentionally narrower than [mirror-component.md](/home/rei/_/cat-engine/docs/spec/mirror-component.md):
- this doc is only about the reflected camera pose, view matrix, and projection handling
- it is written against the engine's current runtime conventions in `src/`
- it is meant to be the baseline for debugging [mirror-camera-orientation-and-tracking.md](/home/rei/_/cat-engine/docs/bugs/mirror-camera-orientation-and-tracking.md)

## Engine conventions this math must follow

From the current engine code:

- transforms are stored as column-major `[[f32; 4]; 4]`
- translation lives in column `3`, so world position is `m[3][0..3]`
- basis columns are:
  - `m[0]` = local `+X` in world space
  - `m[1]` = local `+Y` in world space
  - `m[2]` = local `+Z` in world space
- camera view matrices are the affine inverse of the camera's world matrix
- perspective projection is right-handed, Vulkan depth `z in [0, 1]`
- camera forward is `-Z`, so the camera world matrix column `m[2]` is the camera's local `+Z`,
  which is the camera's back vector, not its forward vector
- the renderer already flips Y with a negative Vulkan viewport height, so mirror math should not
  add a second vertical flip unless there is a renderer-path-specific reason

Those conventions matter because a mirror bug can come from using the correct reflection formula
with the wrong interpretation of the camera basis.

## Mirror plane definition

For the current mirror component model, the mirror plane should come from the mirror transform:

- plane origin `P` = mirror world translation
- plane normal `N` = mirror local `+Z` axis transformed to world and normalized
- the reflective surface lies in mirror local `XY`

In matrix terms, if `M_mirror` is the mirror world matrix:

```text
P = M_mirror[3].xyz
N = normalize(M_mirror[2].xyz)
```

This is a transform-defined plane, not a mesh-derived plane.

## Source camera definition

The source camera for a mirror pass must be the active viewer for that family of views:

- monoscopic: the active monoscopic camera's current world transform
- stereoscopic: each concrete stereoscopic view's current world transform independently

The important requirement is that the source must be a live per-frame world transform, not a stale
registration-time pose.

In current engine terms, the source record should provide:

- `camera_world`
- `view = inverse(camera_world)`
- `proj`

The mirror derivation should treat `camera_world` as authoritative for position and basis.

## Reflection formulas

Given a normalized plane normal `N` and any world-space point `X`, the reflected point is:

```text
reflect_point(X) = X - 2 * dot(X - P, N) * N
```

Given any world-space direction `V`, the reflected direction is:

```text
reflect_dir(V) = V - 2 * dot(V, N) * N
```

This direction formula applies to basis vectors because basis columns represent directions, not
points.

## Correct reflected camera construction

Let the source camera world matrix be:

```text
M_cam = [ R | U | B | C ]
```

Using the engine's column-major notation:

- `R = M_cam[0].xyz` is camera local `+X` in world space
- `U = M_cam[1].xyz` is camera local `+Y` in world space
- `B = M_cam[2].xyz` is camera local `+Z` in world space, which is camera back
- `C = M_cam[3].xyz` is camera world position

Then the physically reflected basis starts as:

```text
C' = reflect_point(C)
Rr = reflect_dir(R)
Ur = reflect_dir(U)
Br = reflect_dir(B)
```

At this point there is a subtle but important issue:

- a planar reflection reverses handedness
- `[Rr, Ur, Br]` is therefore generally an improper basis with determinant `-1`
- a camera world matrix used for ordinary rendering should remain a proper rigid basis with
  determinant `+1`

So the reflected basis must not be used raw unless the renderer is explicitly designed to render
from a reflected-handed camera space and also compensate culling/front-face behavior.

## Handedness correction

The mirror should preserve the reflected viewing direction, but the final camera basis should be
rebuilt into a proper orthonormal frame.

The safest construction is:

```text
F' = normalize(-Br)          // reflected forward, because forward is -Z
Utemp = normalize(Ur)
R' = normalize(cross(F', Utemp))
U' = normalize(cross(R', F'))
B' = -F'
```

Then build:

```text
M_mirror_cam = [ R' | U' | B' | C' ]
V_mirror = inverse(M_mirror_cam)
```

Why this is the preferred reconstruction:

- it preserves the reflected camera position
- it preserves the reflected viewing direction
- it produces a proper right-handed camera frame
- it avoids baking a mirror parity flip directly into the final view matrix
- it keeps the result compatible with the engine's existing culling and camera conventions

## What not to do

Do not assume that reflecting all three basis columns and stuffing them directly into the camera
world matrix is automatically valid for rendering.

That raw reflected matrix is useful as an intermediate geometric result, but it usually has
negative determinant. If the renderer still assumes an ordinary right-handed camera basis, common
failure modes are:

- image appears upside down
- left/right or front/back feel almost right but not fully
- culling/winding behaves inconsistently
- the camera appears correct in one axis but inverted in another

## Alternative equivalent construction

An equivalent way to think about the same result is:

1. reflect camera position
2. reflect the source forward vector
3. choose an up hint by reflecting source up
4. rebuild a proper camera frame from `forward + up_hint`
5. derive back as `-forward`

In engine naming:

```text
forward = normalize(-M_cam[2].xyz)
up = normalize(M_cam[1].xyz)

forward' = normalize(reflect_dir(forward))
up_hint' = normalize(reflect_dir(up))

right' = normalize(cross(forward', up_hint'))
up' = normalize(cross(right', forward'))
back' = -forward'
```

This is easier to reason about than working in terms of back vectors.

## Projection matrix rule

The mirror projection should start from the source camera projection policy, not from ad hoc mirror
basis changes.

For v1:

- preserve source vertical FOV
- preserve source near/far policy
- adjust aspect ratio to the mirror render target extent
- do not vertically flip the projection just to "fix" an upside-down image if the renderer already
  flips viewport Y

In this engine, the default perspective matrix already assumes:

- right-handed camera space
- forward `-Z`
- Vulkan depth range `[0, 1]`

So the mirror pass should use the same projection convention as the window/XR camera family.

## View matrix rule

The mirror view matrix should be:

```text
V_mirror = inverse(M_mirror_cam)
```

It should not be assembled piecemeal in camera space if the world matrix is already known.

That keeps mirror cameras consistent with:

- `CameraSystem` for window cameras
- `OpenXRSystem` for XR eyes

Both of those paths treat the world matrix as the source of truth and derive the view matrix by
affine inversion.

## Tracking rule

Mirror tracking should be evaluated every frame from the current source camera transform.

That means:

- monoscopic mirror views must use the active monoscopic camera's latest world transform
- stereoscopic mirror views must use the latest per-view world transform
- the mirror system must not depend on registration-time camera state being continuously updated

If the mirrored image appears pinned in place, the first thing to verify is that the source
`camera_world` really changes frame to frame in the mirror path being used.

## Renderer boundary

There are two separate classes of problems:

1. reflected camera math is wrong
2. reflected camera math is right, but the renderer path applies an extra orientation flip

This engine already flips Y in the Vulkan viewport for general rendering. Because of that, the
default assumption should be:

- mirror camera math should produce a normal upright camera
- if the mirror is still upside down, inspect mirror-pass-specific rendering behavior before adding
  a basis-space flip in the mirror system

In particular, avoid "fixing" an upside-down mirror by negating the reflected up vector unless it
is proven that the issue is truly in the camera basis and not in the render pass convention.

## Recommended implementation algorithm

For each mirror and each source eye:

1. Read mirror plane origin `P` from mirror transform world translation.
2. Read mirror plane normal `N` from mirror transform world `+Z`, normalized.
3. Read source camera world position `C`.
4. Read source camera forward `F = normalize(-M_cam[2].xyz)`.
5. Read source camera up hint `U = normalize(M_cam[1].xyz)`.
6. Compute `C' = reflect_point(C)`.
7. Compute `F' = normalize(reflect_dir(F))`.
8. Compute `U_hint' = normalize(reflect_dir(U))`.
9. Rebuild a proper basis:
   - `R' = normalize(cross(F', U_hint'))`
   - `U' = normalize(cross(R', F'))`
   - `B' = -F'`
10. Assemble the reflected world matrix from `[R' | U' | B' | C']`.
11. Invert it to get the view matrix.
12. Rebuild or adjust projection for the mirror render target aspect.
13. If needed later, apply oblique near-plane clipping as a projection modification, not as a pose
    modification.

## Debug invariants

Any correct mirror camera derivation should satisfy these checks:

- `dot(R', U') ~= 0`
- `dot(R', F') ~= 0`
- `dot(U', F') ~= 0`
- `|R'| ~= |U'| ~= |F'| ~= 1`
- `cross(R', U') ~= B'`
- `det([R' U' B']) > 0`
- the mirror camera position is the exact plane reflection of the source camera position
- the mirror camera forward is the plane reflection of the source camera forward

If the raw reflected basis fails only the determinant check, that is expected before
re-orthonormalization.

## Likely implications for the current bug

Relative to the current `mirror_system.rs` behavior, these are the main risk points:

- it reflects `right`, `up`, and `back` independently and uses them directly as the final basis
- it does not explicitly repair handedness after reflection
- it assumes the reflected basis itself should solve image orientation
- it reuses source projection with only an aspect change, which is correct in principle, but only
  if the renderer path does not add a mirror-specific flip elsewhere
- if the source window camera's cached `transform.matrix_world` is not live/current, the mirror
  will appear pinned even if the reflection math is otherwise correct

That makes the likely intended fix shape:

- verify the source camera transform is live for the active window path
- derive reflected forward/up from the source camera
- rebuild a proper right-handed reflected frame
- keep viewport/projection flip policy separate from mirror pose math
