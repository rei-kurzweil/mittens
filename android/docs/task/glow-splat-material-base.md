# Glow splat material base

Status: proposed, not implemented. Adding a material base to the Android
renderer is an explicit scope change (AGENTS.md); this ticket asks for that
approval and records the design.

## Problem

The Android renderer is forward-only and accepts just the opaque/albedo
`MaterialBase()`. There is no emission extraction or bloom pass, and adding
one would mean an intermediate target and a blur, which the XR2 tiling
guidance rules out. Bright lights and windows therefore clip to the same white
as a white wall: nothing says "brighter than white".

Glow is wanted for authored light sources:

| Source | Primitive |
|---|---|
| Lamp, point light | One Gaussian halo |
| Neon tube, light strip | One segment |
| Lit window | The window stays an opaque bright surface; one rectangle halo in front of it |
| Glowing triangle / panel edge | One triangle, expanded by the glow radius |

These are one primitive per light, so a night scene with a few thousand lit
windows is a few thousand quads. Reconstructed Gaussian-splat scenes
(hundreds of thousands of splats, spherical harmonics, sorted alpha) are a
different workload and out of scope here.

## Design

Glow is drawn as geometry in the existing eye pass. The shader knows each
light's shape, so the falloff is computed analytically instead of extracting
and blurring bright pixels.

### Base and authoring

`MaterialBase.splat(kind)` with `kind` one of `"gaussian"`, `"segment"`,
`"rect"`, `"triangle"`. Like `MaterialBase.billboard()`
([billboard bases](../../../docs/task/billboard-material-base.md)), the base
owns the vertex stage; authors supply only a fragment through the existing
`MaterialDefinition(base).fragment(...)` path. Each kind ships a default
fragment, so a plain glow needs no custom shader.

The primitive kind is a parameter of one base, not a base per kind: only the
bounding-geometry code differs.

### Data

- Per-splat records live in a storage buffer, std430, 16-byte aligned:
  centre or endpoints/corners, glow radius, colour, intensity, and an index
  into custom per-splat inputs.
- No vertex buffer. The vertex stage pulls the record by instance index and
  builds the quad corners from `gl_VertexIndex` (vertex pulling).
- All splats sharing a definition draw as one instanced draw. The buffer
  keeps its capacity and uploads dirty ranges, following the deformation
  arenas (`android/src/renderer/deformation.rs`). No per-draw allocations or
  descriptor churn.
- Authoring granularity (one scene node per light, gathered into the shared
  buffer, versus a component owning many records) is an open question below.

### Vertex stage

- Each splat is two triangles, so the hardware binner places it in tiles.
  Bounds must be tight: an oriented quad around a segment or rectangle, the
  triangle enlarged by its radius, and an ellipse-aligned quad for Gaussians.
  The axis-aligned squares used by reference 3DGS waste tiles on thin shapes.
- Rect and triangle halos sit in their surface's plane, offset a few
  centimetres toward the viewer to avoid z-fighting with the wall behind.
- Clamp the on-screen radius and fade halos out very close to the eye: fill,
  not splat count, is the expected cost.
- Keep position computation cheap and separate from colour work. Adreno runs
  a position-only binning variant of the vertex stage, so the record fetch and
  projection run twice per vertex.
- Varyings passed to the fragment: a primitive-local coordinate (Gaussian
  sigma units, or distance along/across the segment or shape), colour,
  intensity, splat id.

### Blending and pass placement

- Additive blending by default. Additive glow commutes, so no sort is needed.
- Premultiplied "over" blending would need a per-view sort; reject it with an
  explicit error in this slice.
- Drawn in the same eye pass after opaques: depth test on, depth write off,
  culling off. No new attachments, loads, stores or copies; 4x MSAA resolve
  and foveation apply as today.
- Each XR eye uses its own camera UBO. A later multiview path selects the view
  with `gl_ViewIndex`, as for the other bases.

### Occlusion semantics

A halo is depth-tested at its source's depth, so an occluder in front hides
it. It reads as scattering in the air around the light, not lens bloom, and
does not bleed over occluder edges. Glare that bleeds over occluders, or hides
entirely when the source is hidden, needs an occlusion test and is out of
scope.

## Out of scope

See [transmissive splats](transmissive-splat-material-base.md) for colour captured
from beyond an opening (follow-up ideas, not part of this ticket).

- Reconstructed Gaussian-splat scenes and sorted over-blending.
- Light cast onto surrounding geometry. A faint wide halo can fake a wall
  wash, but real spill needs lights or baked vertex colour.
- HDR targets and tone mapping.
- Lens-style glare and occlusion queries.

## Open questions

- **Desktop parity.** Desktop implements `MaterialBase.splat` too (same
  authoring on both), or rejects it explicitly. Desktop bloom for
  `ExtractedEmissive` is unaffected either way.
- **Authoring granularity.** A `T {}` node per light is simplest to author and
  edit; a record-list component scales better for generated content (city
  windows). Both can feed the same buffer.
- **Custom fragment interface.** Fix the varying locations and the set/binding
  for per-splat custom inputs so the existing descriptor check in
  `android/src/renderer/materials.rs` can validate them.
- **Culling.** CPU bounds per splat or per batch, accounting for the glow
  radius.

## Validation

- Renderer smoke (`android/validation/renderer-smoke`): one rect halo and one
  Gaussian render in both eyes; a box in front occludes a halo; two
  overlapping additive halos sum; an over-blend definition is rejected with
  its error; pipeline and buffer counts stay stable across frames.
- Focus 3 `FRAME_PROFILE` on a test scene at matched extent, MSAA and
  foveation:
  - 0, 100, 1000 and 5000 small, distant halos (vertex and binning cost);
  - one halo filling most of the view (fill cost, with and without the
    radius clamp).
- Confirm no new attachments or render passes in a GPU capture.
