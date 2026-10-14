# Draft: `RefractionSpecularMaterial`

Status: draft / design exploration

2026-10-13: [Water surface optics and refracted lighting](../task/water-surface-and-refracted-lighting.md)
is the current two-slice plan, aligned with Materials v3. It includes planar
reflection and receiver caustics, which this earlier surface-only draft excludes.
The snapshot, material-handle and authoring proposals below are historical;
check that task's source audit before treating them as current implementation.

## Purpose

Define `RefractionSpecularMaterial`: a reusable transparent material that refracts an opaque-scene
snapshot and adds view/light-dependent specular highlights. Its first presentation use case is a
visually richer water surface in `planar-auto-transparency-optimization.mms`, but the material is
not water-specific.

The current ocean is a large translucent `R.plane()` using the ordinary toon material and a fixed
purple-blue RGBA color. It is useful as a placement reference, but it cannot refract scene content
and has no distinct liquid specular response.

## Goals

- Give a planar liquid surface a bright blue/aqua appearance that remains readable behind the
  layout benchmark.
- Refract the opaque scene behind the liquid through a subtle animated screen-space distortion.
- Add an explicit view- and light-dependent specular response.
- Add inexpensive, deterministic animated surface detail to drive those optical effects.
- Support a small, explicit set of author-controlled parameters.
- Preserve the existing transparent render phases: blended liquid uses depth test with depth write
  off.
- Keep the first implementation usable as a built-in material on planes and simple meshes.

## Non-goals

- Physically accurate fluid simulation, shoreline interaction, buoyancy, or mesh displacement.
- Planar reflections, caustics, physically accurate volumetric transmission, or fluid simulation.
- Correct compositing of arbitrary overlapping liquid volumes or transparent geometry.
- Replacing the planar-transparency optimizer or changing its layout-background policy.

## Proposed authoring shape

The exact MMS component/API is deferred, but the intended surface is conceptually:

```text
R.plane() {
  M.refraction_specular({
    transmission_tint: [0.18, 0.66, 0.86],
    opacity: 0.62,
    distortion_scale: 1.0,
    distortion_speed: 0.35,
    distortion_strength: 0.18,
    fresnel_strength: 0.32,
    specular_color: [0.92, 0.98, 1.0],
    specular_strength: 0.48,
  })
}
```

Names, component nesting, and defaults must follow the material-authoring API chosen for the
engine. This is intentionally not a commitment to an MMS spelling.

## First rendering model

The first `RefractionSpecularMaterial` implementation is a dedicated transparent shader/pipeline
variant. It samples a
single-sample snapshot of the opaque scene color, adds a specular highlight, and composites into
the foreground before ordinary transparent content is drawn.

Its vertex shader should retain the existing mesh attributes and instance layout. It should
produce the ordinary world position and transformed normal, plus stable world-unit surface
coordinates derived from the transformed local plane axes and projected `v_screen_uv` coordinates
for sampling the captured opaque scene. It does **not** displace vertices in v1: the ordinary plane
is too low-density for convincing geometric waves, so animated wave detail remains fragment-stage
normal/distortion only.

For each fragment, the material shader should:

1. derive two scrolling procedural wave signals from surface coordinates and shared frame time;
2. combine them into a small perturbed normal and refraction offset;
3. sample the opaque-scene snapshot at the clamped, distorted `v_screen_uv`;
4. tint that refracted color with the selected transmission color;
5. add a restrained Fresnel/specular term from the perturbed normal, camera direction, and scene
   lights; and
6. composite the result using the engine's documented transparent-alpha convention.

Depth-aware absorption, water thickness, and shoreline fades are explicitly deferred. The first
version refracts opaque scene color only; it does not sample depth.

## Render-graph placement

The current renderer writes opaque, cutout, and transparent phases directly into its active color
attachment. A liquid fragment shader cannot safely sample that same active attachment. The render
graph therefore needs this ordering:

```text
opaque + cutout
  -> resolve/copy opaque scene color to a sampleable refraction source
  -> refraction/specular material pass
  -> existing transparent single-layer and multilayer phases
  -> overlay / post-processing final work
```

The refraction source is an opaque-only snapshot. Other transparent objects are deliberately not
included: refracting already-composited arbitrary transparency needs a different ordering and
compositing contract. The liquid pass depth-tests against opaque geometry and keeps depth writes
off.

### Relationship to the mirror architecture

`RefractionSpecularMaterial` should reuse the *architectural pattern* of the renderer's mirror
support: the renderer
creates an auxiliary image and binds it to a dedicated material pass. The image source differs:

```text
mirror: reflected-camera scene capture -> mirror material samples it
liquid: current-camera opaque-scene snapshot -> liquid material samples it with distortion
```

A mirror capture requires rendering the scene again from a reflected camera and may be unique to a
mirror. `RefractionSpecularMaterial` should instead share one opaque-scene snapshot per window
frame or XR eye across all of its instances. The existing mirror path is therefore the reference for dynamic texture binding,
per-view target ownership, and special-material pipeline routing; it is not the source of the
liquid image or its pass ordering.

## Transparency and ordering contract

`RefractionSpecularMaterial` is ordinary transparent content, not layout-owned planar
transparency metadata.

- The initial blended material uses depth testing with depth writes off.
- It receives its own pass before the existing transparent single-layer stream because it needs the
  opaque-scene refraction source. It does not silently join the batched generic transparent material
  path.
- An author who needs correct blending with other transparent surfaces must select the existing
  multilayer transparent policy; refraction of those surfaces is not supported in v1.
- The material must not claim that a water plane is globally isolated merely because it is planar.

This keeps liquid independent from the layout optimizer, which only classifies declared layout
background scopes.

## Engine implications

Today, built-in material handles and shader paths are static, and `MaterialUBO` only contains base
color, quantization, and emissive fields. A liquid implementation therefore needs:

- one `MaterialHandle` and shader registration;
- a `RefractionSpecularMaterial` parameter block and a shared per-frame `time_seconds` source;
- a sampleable, resolved opaque-scene-color snapshot for each window/XR eye and the descriptor
  layout needed to bind it during the liquid pass;
- a dedicated liquid pass inserted after opaque/cutout and before generic transparent rendering;
- transparent pipeline selection with depth write off; and
- test coverage that verifies uniform layout and material-to-pipeline routing.

The first version should use analytic/procedural waves rather than a normal-map asset so it has no
new asset-loading dependency. Normal-map support can be evaluated after the material parameter
path exists.

## Visual direction for the benchmark scene

- Move the clear/background color toward a pale neutral or slightly blue-white value.
- Use a brighter aqua-to-blue liquid palette with restrained alpha, rather than the current dark
  purple surface.
- Keep wave contrast subtle enough that it does not obscure the pink layout-background quads or
  their opaque white cubes.
- Favor broad, slow highlights over visually busy noise; this is a renderer/transparency benchmark
  first.

## Delivery stages

1. Brighten the benchmark environment and tune the current flat ocean color; no new material.
2. Add an opaque-scene snapshot and a dedicated pass with a pass-through liquid test shader.
3. Add `RefractionSpecularMaterial` parameter/time plumbing and its refraction/specular shader.
4. Verify desktop and XR eye-local snapshots, depth testing, and transparent-pass ordering.
5. Expose the authoring API and provide a small standalone water example.
6. Consider depth tint, normal maps, reflection captures, and geometric waves only after measuring
   the first path's cost and compositing behavior.

## Open questions

- Does material authoring need a general parameter block before this, or can the initial liquid
  material own a narrowly scoped component?
- Is the renderer's current alpha convention fully documented for a dedicated liquid shader?
- Should the opaque-scene snapshot be allocated only when a liquid instance is visible, or kept as
  a reusable render-graph target whenever the liquid material is registered?
- Which rendering targets must animated liquid support initially: desktop only, or desktop and XR?
