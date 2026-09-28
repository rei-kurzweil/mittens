# Task tracker: name the annulus, then add a filled circle

Status: renderable refactor implemented; visual inspection of the updated
showcase and rhythm cue integration remain pending.
Parent: [VR rhythm game prototype](epic/vr-rhythm-game-prototype.md).

## Why

The current MMS constructor `R.circle2d()` renders a narrow annulus, not a
filled circle. Its built-in mesh is `MeshFactory::circle_2d(0.45, 0.5, 64)`.
This makes circle target authoring misleading: the head and hand cues need an
outer annulus and a same-color filled disk that grows from the center until
it visually joins the annulus. Keep `R.partial_annulus_2d(...)` for authored
arcs; it is a separate constructor with a different purpose.

The public names are `R.annulus_2d()` for the existing ring and
`R.circle_2d()` for the new filled disk. The former `R.circle2d()` spelling
remains a legacy parsing alias for the annulus; in-repo scenes and serialized
output use `R.annulus_2d()`.

## Phase 1 — rename the existing ring

- Make `R.annulus_2d()` the canonical constructor for the existing XY-plane
  ring, preserving its +Z normal, radii 0.45/0.5, 64 segments, color/material
  behavior, and mesh-handle identity where possible.
- Rename the matching Rust entry points and built-in mesh labels that still
  say “circle” but construct an annulus. This includes
  `MeshFactory::circle_2d`, `RenderableComponent::circle2d`, and the
  `BuiltinMeshType::Circle2D`/`CpuMeshHandle::CIRCLE_2D` naming path. Keep
  numeric handle stability if any runtime or serialized representation relies
  on it.
- Change the MMS runtime signature, component registry dispatch, authored
  shape/serialization spelling, and the asset-panel primitive export. Search
  all examples, assets, tests, and relevant docs for `R.circle2d()`,
  `circle2d`, and the internal `circle_2d` spelling; migrate ring usage to
  the new annulus name.
- Decide explicitly how older saved MMS using `R.circle2d()` loads during the
  migration. If a temporary alias is needed, it must retain ring semantics
  and serialize back as `R.annulus_2d()` with a clear deprecation path. It
  must never become a filled disk by accident.
- Keep `R.partial_annulus_2d(...)` and its parameter meanings unchanged.

Phase 1 is complete when all in-repo ring authoring uses `R.annulus_2d()`,
round-tripping produces that name, and the ring still renders identically.

## Phase 2 — add the actual filled circle

After phase 1 lands, add `R.circle_2d()` as a filled XY-plane disk. Give it
the same outer radius 0.5 and segment count 64 by default so a unit-scale
disk can meet the renamed annulus's outer edge. The mesh should have one
center vertex and a perimeter triangle fan with consistent +Z winding,
normals, UVs, and bounds. This avoids relying on a zero-width inner annulus
with degenerate triangles. Parameterized radius/segment variants can follow
later if the prototype needs them; the first constructor may take no args.

Wire the new shape through the built-in mesh registry, `RenderableComponent`,
MMS runtime signature and dispatch, authored-shape serialization, asset-panel
primitive shelf, and focused mesh/scene tests. Do not change
`R.partial_annulus_2d` to implement the public disk API.

## Rhythm target use

The reusable target factory in `assets/components/rhythm_game/` should use
`R.annulus_2d()` for the outline and `R.circle_2d()` for the expanding fill,
with one authored color and a common center. Animate the disk's scale from
small to its intended radius over the beat before judgment. Keep the two
surfaces separated just enough in depth to avoid coplanar flicker, without a
visible color or position gap at the due beat.

The earlier `R.partial_annulus_2d(0.0, 0.5, 0.0, 6.283185, 64)` disk workaround
is no longer needed for a full circle.

## Acceptance

1. Phase 1: existing ring examples look the same, use `R.annulus_2d()`, and
   round-trip to that constructor. `R.partial_annulus_2d()` still renders and
   serializes as before.
2. Phase 2: `R.circle_2d()` is visibly filled, with no center hole or seam;
   its mesh has valid triangles, UVs, +Z normals, and bounds, and survives MMS
   serialization/round-trip.
3. At equal final radii, the expanding disk and same-color annulus form one
   continuous cue for head, left hand, and right hand in the minimal rhythm
   example.
