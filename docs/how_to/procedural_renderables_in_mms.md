# How To: Procedural Renderables In MMS

Date: 2026-06-30

Related spec:

- [docs/spec/procedural-renderables.md](../spec/procedural-renderables.md)

## Goal

Show how to author built-in procedural renderables directly in `.mms` files.

This doc is usage-oriented and focuses on MMS authoring.
For the Rust/API surface and implementation-facing contracts, see:

- [docs/spec/procedural-renderables.md](../spec/procedural-renderables.md)

## Currently available constructors

You can currently author these procedural renderables in MMS:

- `R.icosahedron(...)`
- `R.partial_annulus_2d(...)`
- `R.star(...)`
- `R.heart(...)`

All of them can be styled the same way as other renderables, for example with:

- `C.rgba(...)`
- `EM.on()`
- `TextureFiltering.linear()`

## 1. Icosahedron

Authoring form:

```mms
R.icosahedron(tessellations, sphericalness)
```

Parameter meanings:

- `tessellations`: recursive subdivision depth
- `sphericalness`: `0.0` keeps subdivision on the original face planes, `1.0` makes an icosphere

Example:

```mms
T.position(0.0, 0.0, -4.0) {
    R.icosahedron(2, 0.85) {
        C.rgba(0.25, 0.8, 1.0, 1.0)
        EM.on()
    }
}
```

Base low-poly form:

```mms
R.icosahedron(0, 0.0) {
    C.rgba(1.0, 0.75, 0.25, 1.0)
}
```

## 2. Partial annulus

Authoring form:

```mms
R.partial_annulus_2d(
    inner_radius,
    outer_radius,
    start_angle_radians,
    sweep_angle_radians,
    segments,
)
```

Parameter meanings:

- `inner_radius`: hole radius
- `outer_radius`: outside radius
- `start_angle_radians`: where the arc starts
- `sweep_angle_radians`: how far the arc extends
- `segments`: tessellation along the sweep

Example:

```mms
T.position(-2.1, -2.1, -4.0) {
    R.partial_annulus_2d(0.55, 0.89, 0.0, 1.5707963, 48) {
        C.rgba(0.89, 0.16, 0.11, 1.0)
        EM.on()
    }
}
```

Quarter-circle reminder:

- `1.5707963` is approximately `π / 2`

## 3. Star

Authoring form:

```mms
R.star(
    points,
    inner_radius_fraction,
    outer_bevel_segments,
    inner_bevel_segments,
)
```

Parameter meanings:

- `points`: number of outer points
- `inner_radius_fraction`: how far out the inner valleys are as a fraction of the outer radius
- `outer_bevel_segments`: rounding segments for the outer tips
- `inner_bevel_segments`: rounding segments for the inner concave valleys

Example:

```mms
T.position(2.45, 0.55, -4.0).scale(1.6, 1.6, 1.0) {
    R.star(5, 0.48, 3, 2) {
        C.rgba(0.98, 0.81, 0.16, 1.0)
        EM.on()
    }
}
```

A sharp-corner variant:

```mms
R.star(5, 0.45, 0, 0) {
    C.rgba(1.0, 0.9, 0.2, 1.0)
}
```

## 4. Heart

Authoring form:

```mms
R.heart(segments)
```

Parameter meaning:

- `segments`: contour sampling density

Example:

```mms
T.position(2.55, -1.85, -4.0).scale(1.7, 1.7, 1.0) {
    R.heart(96) {
        C.rgba(0.94, 0.22, 0.42, 1.0)
        EM.on()
    }
}
```

## 5. Building the rainbow example in MMS

A concentric quarter-rainbow is just multiple `R.partial_annulus_2d(...)`
renderables with different radii:

```mms
T.position(-2.1, -2.1, -4.0) {
    R.partial_annulus_2d(0.55, 0.89, 0.0, 1.5707963, 48) { C.rgba(0.89, 0.16, 0.11, 1.0) }
    R.partial_annulus_2d(0.92, 1.26, 0.0, 1.5707963, 48) { C.rgba(0.98, 0.49, 0.10, 1.0) }
    R.partial_annulus_2d(1.29, 1.63, 0.0, 1.5707963, 48) { C.rgba(0.99, 0.84, 0.13, 1.0) }
    R.partial_annulus_2d(1.66, 2.00, 0.0, 1.5707963, 48) { C.rgba(0.16, 0.68, 0.27, 1.0) }
    R.partial_annulus_2d(2.03, 2.37, 0.0, 1.5707963, 48) { C.rgba(0.10, 0.42, 0.91, 1.0) }
}
```

If you want small gaps between bands, increase the distance between one band’s
`outer_radius` and the next band’s `inner_radius`.

## 6. Full example

See:

- [examples/pride.mms](../../examples/pride.mms)

That file includes:

- five concentric partial annuli
- one star
- one heart

all authored directly in MMS.

The same runtime path also supports `R.icosahedron(...)`.

## Notes

- These shapes are procedurally generated and registered automatically during
  normal MMS spawn.
- You do not need Rust-side `render_assets.register_mesh(...)` calls for these
  constructors.
- Save / round-trip is intended to preserve these constructor calls.
