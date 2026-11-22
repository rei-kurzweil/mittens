# Billboard material bases

Status: first slice implemented 2026-11-22, not yet run on hardware. Approved
as an explicit renderer scope change (engine-owned vertex stages) instead of
author-supplied vertex shaders.

## What exists

- `MaterialBase.billboard()`: a local-XY quad turned square to each view,
  sized by the model's X/Y scale and centred on its origin. For smoke puffs
  and sprites.
- `MaterialBase.axial_billboard()`: a local-XY quad that keeps its local Y
  axis (direction and length) and turns about it to face each view, as wide
  as its X scale. For trails and beams.
- Both reuse the custom-material path: `MaterialDefinition(base)
  .fragment(...)`. The vertex stages (`assets/shaders/billboard-mesh.vert`,
  `axial-billboard-mesh.vert`) keep `toon-mesh.vert`'s inputs and outputs,
  so existing custom fragments run unchanged. Opaque, transparent and
  emission-extraction pipelines all get the billboard stage.
- Each XR eye has its own camera UBO, so each eye faces its own quad.
- The STA rocket trail is now one axial-billboard quad instead of two crossed
  planes, which roughly halves its transparent overdraw.

Choice over `.vertex(path)` on MaterialDefinition: a fixed base keeps the
vertex/instance layout private, keeps stereo handling in one place, and is
one explicit feature for the Android renderer to add, rather than accepting
arbitrary vertex SPIR-V there.

## Limits

- No cached-deformation variant: billboards on skinned meshes get the usual
  "no cached-deformation program" message and are not drawn.
- `opaque_bucket` is still Surface-only.
- CPU bounds come from the flat quad, so culling near the frustum edge can be
  off by up to half the quad's width (axial) or its half-diagonal (camera
  facing). A billboard-aware bounds expansion is a follow-up if popping shows.
- Raycasts and BVH picking see the authored, unrotated quad. The rocket trail
  is ray-disabled; give billboards `Raycastable.disabled()` until picking
  follows the vertex stage.
- The Android renderer still accepts only `MaterialBase()` and rejects these
  bases with its existing explicit error.

## Follow-ups

- Smoke puffs (`smoke.mms`) and the curved ribbons in
  [curved smoke ribbons](curved-smoke-ribbons-and-spin-blur.md) can use these
  bases instead of crossed cards.
- Editor gizmos (handles, icons) are a candidate for camera-facing
  billboards; that needs its own pass over gizmo picking and depth.
- Validation: the rocket test checks one emissive cone and one billboard trail
  instance. A GPU capture should confirm the trail faces both eyes and stays
  clipped at the launch point.
