# Grid Material Spec

Two procedural grid shaders rendered entirely in the fragment stage using
world-space derivatives. No texture lookups, no CPU-side geometry subdivision.

- **`grid-square`** — Cartesian grid of perpendicular lines
- **`grid-polar`** — Polar grid of concentric circles and radial spokes

Both support a logarithmic mode that shows 2–3 overlapping octaves simultaneously,
with coarser levels drawn with progressively thicker lines.

---

## 1. Core technique — why `fwidth` is non-negotiable

A naïve grid (hard threshold on `fract(worldPos)`) aliases catastrophically:
lines shimmer as the camera moves, disappear at oblique angles, and have no
consistent width at varying distances.

The fix is to express every threshold in **screen-space pixels** using the
screen-space derivative functions the GPU computes for free:

```glsl
// fwidth(v) = abs(dFdx(v)) + abs(dFdy(v))
// Tells you: "how many units of v change per pixel at this fragment."
```

`fwidth(gridUV)` adapts automatically to zoom level, perspective foreshortening,
and oblique viewing angles.

### The canonical antialiased line

```glsl
float grid_line_alpha(float v, float spacing, float thickness) {
    float uv   = v / spacing;
    float deriv = fwidth(uv);
    float dist  = abs(fract(uv - 0.5) - 0.5);
    float px    = dist / (deriv * thickness);   // thickness > 1 = wider line
    return 1.0 - smoothstep(0.0, 1.0, px);
}
```

For a 2D Cartesian grid (both axes at once):

```glsl
float grid_square_alpha(vec2 worldXZ, float spacing, float thickness) {
    vec2 uv    = worldXZ / spacing;
    vec2 deriv = fwidth(uv);
    vec2 dist  = abs(fract(uv - 0.5) - 0.5) / (deriv * thickness);
    return 1.0 - min(min(dist.x, dist.y), 1.0);
}
```

Lines are exactly **1 × thickness pixels** wide at all zoom levels and angles.
No MSAA needed for interior lines; MSAA still helps the mesh boundary edges.

---

## 2. Distance fade

Without fading, distant grids turn into a flat grey slab as cells go sub-pixel
and the smoothstep saturates. Fade against XZ distance (not 3D distance) so the
grid doesn't disappear when the camera is elevated:

```glsl
float camDist = length(v_world_pos.xz - camera_pos.xz);
float fade    = 1.0 - smoothstep(fade_start, fade_end, camDist);
alpha *= fade;
```

`camera_pos` is `inverse(view)[3].xyz`, or added as a field to `CameraUBO`.

---

## 3. Multi-octave logarithmic mode

A single fixed spacing is either too dense (zoomed out) or too sparse (zoomed
in). The logarithmic mode shows **2–3 simultaneous octaves**: the finest visible
level fades in as you zoom in, while coarser levels are always visible with
progressively thicker lines.

### Computing the current log level

```glsl
// world_per_px: approximate world units per screen pixel at this fragment
float world_per_px  = length(fwidth(v_world_pos.xz));
float screen_extent = world_per_px * ubo.viewport.x;

// level0: the finest octave whose spacing still covers at least target_cells cells
float log_level = log(screen_extent / target_cells) / log(log_base);
float level0    = floor(log_level);
float blend     = fract(log_level);   // 0→1 as you zoom through one octave
```

### Drawing 3 octaves simultaneously

```glsl
// Octave spacings relative to level0
float s0 = pow(log_base, level0);          // finest — fades in as blend→1
float s1 = pow(log_base, level0 + 1.0);   // mid
float s2 = pow(log_base, level0 + 2.0);   // coarsest — always prominent

// Thickness grows with spacing: coarser = thicker
float t0 = 1.0;
float t1 = mat.octave_thickness;           // e.g. 1.8
float t2 = mat.octave_thickness * mat.octave_thickness;  // e.g. 3.2

float g0 = grid_fn(worldXZ, s0, t0) * blend;   // blends in
float g1 = grid_fn(worldXZ, s1, t1);            // always on
float g2 = grid_fn(worldXZ, s2, t2);            // always on, thick

float alpha = max(max(g0, g1), g2);
```

`grid_fn` is `grid_square_alpha` or `grid_polar_alpha` depending on the shader.
The finest octave (`g0`) is multiplied by `blend` so it fades in smoothly as
you zoom in. The two coarser octaves are always fully visible, giving a stable
visual frame of reference. At base 10, `s1 = 10 × s0` and `s2 = 100 × s0`,
which matches natural unit scales (cm / dm / m, etc.).

### Axes (square mode only)

The X=0 and Z=0 axes get a distinct colour and extra thickness:

```glsl
float axis_alpha(float v) {
    float fw = fwidth(v);
    return 1.0 - smoothstep(0.0, axis_thickness * fw, abs(v));
}
float ax    = max(axis_alpha(worldXZ.x), axis_alpha(worldXZ.y));
color       = mix(color, axis_color.rgb, ax * axis_color.a);
```

---

## 4. Polar grid (`grid-polar`)

A polar grid draws **concentric circles** and **radial spokes** centred at the
world origin (or a configurable centre point). It uses the same `fwidth` +
`smoothstep` antialiasing as the square grid.

### Concentric circles

Treat the radial distance `r = length(worldXZ - centre)` as a 1D axis and
apply `grid_line_alpha` to it:

```glsl
float r      = length(worldXZ - mat.polar_centre);
float circle = grid_line_alpha(r, spacing, thickness);
```

In logarithmic mode, apply the same 3-octave compositing with `r` substituted
for the linear coordinate. The circles at `r = s0, s1, s2, 2*s0, 2*s1, ...`
are all produced automatically.

One subtlety: `fwidth(r)` is well-defined everywhere except exactly at
`r = 0` (the origin), where it collapses to zero. Guard with:

```glsl
float r = max(length(worldXZ - mat.polar_centre), 1e-6);
```

### Radial spokes

The angle `theta = atan(worldXZ.y - centre.y, worldXZ.x - centre.x)` maps
`(-π, π)` to a periodic 1D axis. With `num_spokes` equally spaced lines:

```glsl
float spoke_uv    = theta / (2.0 * PI) * float(mat.num_spokes);
float spoke_deriv = fwidth(spoke_uv);
float spoke_dist  = abs(fract(spoke_uv - 0.5) - 0.5) / (spoke_deriv * thickness);
float spoke       = 1.0 - min(spoke_dist, 1.0);
```

`atan` is discontinuous at the ±π seam (the negative X half of the X axis).
The `fwidth` approximation becomes unreliable in the two pixels that straddle
this seam. In practice the artefact is a single-pixel glitch on one radial
spoke; it can be masked by ensuring `num_spokes` places a spoke on that angle,
or simply accepted as a known limitation.

Spoke spacing does not participate in the logarithmic mode — the number of
spokes is fixed (e.g. 12 or 36). Only the concentric circles scale
logarithmically.

### Combining circles and spokes

```glsl
float alpha = max(circle_alpha, spoke_alpha) * fade;
vec3 color  = mix(bg_color.rgb, line_color.rgb, alpha);
```

Circles and spokes share one line colour. They can be given separate colours
via a second `spoke_color` UBO field and `max`-composited separately if needed.

---

## 5. Shader file layout

```
assets/shaders/
  grid.vert        — passthrough: outputs v_world_pos only, no lighting
  grid-square.frag — Cartesian grid, optional log octaves
  grid-polar.frag  — Polar grid (circles + spokes), optional log octaves
```

The vertex shader is identical to `unlit-mesh.vert` but without lighting
outputs. The mesh is a large `quad_2d` scaled to `fade_end * 2`, flat in the
XZ plane.

---

## 6. Material UBO

```glsl
layout(set = 1, binding = 0) uniform GridMaterialUBO {
    // Colors
    vec4  line_color;        // rgba — grid line color
    vec4  bg_color;          // rgba — cell background (usually transparent)
    vec4  axis_color;        // rgba — X/Z axis highlight [square only]

    // Shared
    float fade_start;        // world XZ distance where fade begins
    float fade_end;          // world XZ distance where fully faded

    // Linear mode
    float grid_spacing;      // world units per cell [linear only]
    float _pad0;

    // Log mode (grid_spacing unused when log_base > 0)
    float log_base;          // 2.0 or 10.0; 0 = linear mode
    float target_cells;      // desired cells across viewport
    float octave_thickness;  // thickness multiplier per octave step (e.g. 1.8)
    float axis_thickness;    // axis line half-width in pixels [square only]

    // Polar mode [polar only]
    vec2  polar_centre;      // XZ world position of pole
    int   num_spokes;        // number of radial lines (e.g. 12)
    float _pad1;
} mat;
```

---

## 7. `MaterialHandle` entries

```rust
impl MaterialHandle {
    pub const GRID_SQUARE: Self = Self(/* next id */);
    pub const GRID_POLAR:  Self = Self(/* next id */);
}
```

---

## 8. Antialiasing summary

| Technique | What it fixes |
|---|---|
| `fwidth` + `smoothstep` | Lines stay 1 × thickness px at all distances and angles |
| Distance fade | Prevents grey-wash where cells are sub-pixel |
| Log octave blending | Finest level fades in; no discrete pop between zoom levels |
| Coarser = thicker | Visual hierarchy; coarse grid never competes with fine grid |
| `r = max(r, 1e-6)` | Prevents `fwidth` collapse at the polar origin |
| MSAA (optional) | Helps mesh boundary edges; redundant for interior lines |
