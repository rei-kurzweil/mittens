# Ambient occlusion baking

Status: slices 2–4 implemented for case B (untextured `Texcoord0`) with the
built-in UV generator, unbuilt and unvalidated in the engine. The pickup truck
body enables it. Nothing is cached on disk yet, and bakes run synchronously.

## Implementation (2026-11-21)

- `src/engine/graphics/ao_bake/`: `surface` (weld, adjacency), `atlas`
  (`UvGenerator` trait, `BuiltinUvGenerator`), `texels` (rasterize, dilate),
  `occlusion` (proximity + cavity with uniform grids), `tests`.
- `AmbientOcclusionComponent` and its MMS builders: `texcoord0`, `resolution`,
  `strength`, `proximity_radius`, `cavity_radius`, `cavity_angle`,
  `one_sided`, `debug_checker` / `debug_proximity` / `debug_cavity`.
- `CombineMeshSystem` bakes when the combined mesh rebuilds, through
  `combine_mesh_occlusion.rs`. Bakes are shared by content and settings, and
  the texture is bound after `reconcile_and_build`.
- `xatlas` is not bound yet. The built-in generator is the only one, so it
  came first, because it could be prototyped and checked here
  (`ao_proto.py`-style NumPy prototype on the real truck: 187 charts, no
  overlap, ~44 texels/m at 512²).

### Findings from the truck prototype

- The cabin is an open single-sheet shell with inward normals, while about
  half of the body is solidified panels (a second layer 1–2 cm behind).
  Two-sided bakes therefore measure both sides of every texel. A side with a
  parallel facing layer within 0.4 × `proximity_radius` is inside the
  material and is ignored; otherwise the more open side wins. Consequence:
  single-sheet surfaces (and a lone ground plane) get no AO on their enclosed
  side. `one_sided()` trusts the winding instead.
- Proximity uses face normals. Smoothed normals tilt at fan centres and made
  coplanar neighbours self-occlude (dark dots).
- A crease darkens only as far as its walls extend, so a 1 cm lip does not
  shade 7 cm of floor.
- Default radii are absolute mesh-space units (0.15 proximity, 0.07 cavity),
  tuned for the truck in metres.

## Goal

Bake a per-mesh ambient occlusion (AO) texture, at runtime or ahead of time,
and cache the result on disk. The darkening comes from two terms, both baked
per texel into a unique UV atlas:

- **Proximity:** two surfaces of the same primitive close together (folds,
  gaps, a seat against the floor) darken each other.
- **Cavity:** concave curvature sharp enough to count as a crease darkens
  nearby surface, with a falloff measured across the surface.

The atlas comes from automatic UV generation. `xatlas` is the default
generator; a built-in generator is a later, optional fallback.

The bake is configured with an `AmbientOcclusionComponent` (builder methods,
MMS-constructible) attached under a renderable. The renderable draws normally
until the bake finishes, then switches to the baked mesh and texture.

## Non-goals (for now)

- AO between different objects (scene AO / lightmaps). AO is per mesh and
  shared by every instance of it.
- Per-frame or deforming AO. Skinned and morphed meshes bake in bind pose.
- Screen-space AO.

## Design

### Component

```rust
pub struct AmbientOcclusionComponent {
    pub uv: AoUvChannel,            // Texcoord1 (default) | Texcoord0
    pub merge: AoMerge,             // None | BaseColor | Texture | TextureAndBaseColor
    pub generator: UvGeneratorKind, // Auto (default) | Xatlas | Builtin
    pub resolution: u32,
    pub strength: f32,
    pub proximity_radius: f32,      // fraction of mesh bounds diagonal
    pub cavity_angle_deg: f32,      // dihedral angle that starts to darken
    pub cavity_radius: f32,         // fraction of mesh bounds diagonal
    pub samples: u32,
    pub cache: AoCache,             // Disk (default) | Memory | None
    pub debug: AoDebug,             // Off | Checker | Proximity | Cavity
}
```

Builder methods follow the existing component idiom (`new()`, `with_*`), as
in `LightQuantizationComponent`. `init` sends a registration intent.
`to_mms_ast` round-trips the non-default settings.

### UV channel cases

`uv` selects the channel. With `Texcoord0`, whether the renderable already
has a texture decides between cases B and C. No extra setting is needed.

| | A: `Texcoord1` (default) | B: `Texcoord0`, no texture | C: `Texcoord0`, textured |
|---|---|---|---|
| Mesh | adds optional `CpuMesh.uv1` stream; `uv0` untouched | `uv0` replaced with atlas UVs | `uv0` replaced with atlas UVs |
| Texture | existing albedo kept, sampled through `uv0`; AO is a second texture | the AO texture becomes the only texture | albedo transferred into the atlas and multiplied by AO |
| Base colour | material multiplier | multiplier by default (`AoMerge::None`); baked in only with `BaseColor` | multiplier, or baked in per `merge` |
| Runtime | 2 textures, 2 UV sets | 1 texture, 1 UV set | 1 texture, 1 UV set |
| Material change | needs AO input and `uv1` | none | none |
| Destructive | no | no; nothing used `uv0` | yes; the albedo loses tiling and mirroring |

Case B is the truck body. Keeping base colour as a multiplier means
differently painted instances (`set_color`) share one bake and one cache
entry. Colour is not part of the cache key unless `merge` bakes it in.

`uv1` is an optional side stream on `CpuMesh`, like `joints0`/`weights0`, not
a `CpuVertex` field. That keeps the vertex layout for every other pipeline
unchanged (see `docs/analysis/vertex-color-pipeline-variants.md`).

`Texcoord0` must be rejected, with a reported error, when a consumer of the
renderable still needs the original `uv0`: a procedural `uv` shader, a UV
override clone, or a texture with `merge` not including it. It must never
silently fall back.

### UV generation

```rust
pub trait UvGenerator {
    fn generate(&self, mesh: &CpuMesh, opts: &UvGenOptions) -> Result<UvAtlas, UvGenError>;
}
// UvAtlas { xref: Vec<u32>, uvs: Vec<[f32; 2]>, indices: Vec<u32>, width: u32, height: u32 }
```

- `xref[i]` is the source vertex for output vertex `i`. Seams duplicate
  vertices; `joints0`, `weights0`, and morph target deltas are remapped
  through `xref`.
- The `xatlas` implementation is a thin `cc`-built FFI over upstream
  `xatlas.cpp`, behind cargo feature `xatlas`. Desktop enables it by default.
  Android builds it with the NDK only if runtime bakes are wanted there.
- `Auto` uses `xatlas` when it is compiled in, otherwise `Builtin`, and logs
  the choice once per mesh. Explicit `Xatlas` without the feature is an error
  and the renderable draws without AO.
- Every atlas, from either generator, passes the same validation: no overlap,
  minimum gutter, per-triangle stretch bound, and all UVs within [0,1].

### Bake pipeline

1. **Weld** by position for adjacency. glTF splits vertices at normal and UV
   seams.
2. **Generate the atlas** (above).
3. **Rasterize** each texel to its triangle, barycentrics, world position and
   normal, with conservative coverage so slivers get texels.
4. **Proximity:** for each texel, find the nearest front-facing surface within
   `proximity_radius`, skipping triangles that share a vertex with the
   texel's triangle. The first version uses a nearest-point query; hemisphere
   rays are a follow-up and reuse the same triangle BVH.
5. **Cavity:** compute signed dihedral angles over welded edges. Concave edges
   over `cavity_angle_deg` are darkening sources, with falloff over surface
   distance up to `cavity_radius`.
6. **Combine:** `occlusion = max(proximity, cavity)`,
   `ao = 1 - strength * occlusion`.
7. **Merge** per `merge` and `uv`: multiply into base colour or into the
   transferred albedo. When transferring albedo, take 2×2 to 4×4 samples per
   texel and respect the source texture's wrap mode.
8. **Dilate** into the gutters.
9. **Swap:** register a cloned mesh, keeping `base_mesh` as the original for
   raycasts and collision inference, then upload the texture and update the
   renderable. This is the same pattern as UV overrides.

Bakes run on a worker thread, never the render thread. `xatlas`'s progress
callback cancels the bake if the renderable is despawned first.

### Cache

- Key: hash of vertex positions and normals, indices, the albedo texture
  contents if merging, all AO settings, generator kind and version, and a
  `BAKER_VERSION` constant.
- Desktop: `assets/data/ao_cache/<key>.mesh` and `<key>.png` (KTX2 later).
- Android: assets first, then app internal storage. Desktop bakes can be
  copied into the Android assets, so Android never needs `xatlas` for shipped
  content.
- A miss with no generator available draws without AO and logs once.

## First target: pickup truck

Facts from `assets/models/mitten_pickup.glb` and
`assets/components/vehicles/pickup_truck.mms` (2026-11-21):

- The body uses no textures, so it is case B. Materials are base colour
  factors only, and the truck uses built-in Anime (`pickup_anime_material`).
  Paint colour stays an instance multiplier, so one bake serves every truck.
  `Texcoord0` mode
  needs no shader change, if built-in Anime samples an attached texture
  through `uv0` the way the tyres' `tire-tread.png` already is. **Verify this
  in slice 2.**
- Existing `uv0` cannot be reused. Several parts overlap heavily (default
  cube unwraps), and the `CombineMesh` body batch puts every part into the
  same 0–1 range. One primitive has no `TEXCOORD_0` at all. The atlas must be
  generated.
- AO goes on the `pickup_painted_body` `CombineMesh` output, not on the
  individual glTF parts. That way proximity between parts (seat to floor, bed
  to wheel wells, steering wheel to dash) counts as the same primitive. The
  bake must wait until the combine is complete.
- The tyres use `uv0` for the tread texture. They need `Texcoord1`, or
  `Texcoord0` with albedo transfer. They are out of scope until slice 6.
- The body is mostly **double-sided single sheets**, and the shader flips
  backfacing normals. Both sides of a sheet share one texel, so the inside
  and outside of a door get the same AO. Options: bake only the authored
  normal's side, take the darker of both hemispheres, or duplicate
  triangles at bake time to give each side its own chart. This needs a
  visual review in slice 3.
- Destroyable parts can change the combined mesh. A rebake follows from the
  cache key; the first version may simply drop AO when the mesh changes.
- Consider moving retained batch appearance into the AO module. Keep its
  bake and texture lookups off the intact render path, and check that CSG
  outputs and unpacked sources sample only textures that match their UVs.

## Slices

1. **Tracker and design** (this file).
2. **UV generation and checker look.** (done, built-in generator only) Add the `UvGenerator` trait and the
   `xatlas` FFI. Add `AmbientOcclusionComponent` with `Texcoord0` and
   `AoDebug::Checker` only; it bakes a checkerboard into the atlas and swaps
   it in. Add a runtime-generated texture path (today's `TextureComponent`
   loads from a URI only). Attach it to the truck body batch.
   *Validate:* atlas validation tests; the truck shows an even checker with
   no stretching smears and no overlap; verify built-in Anime samples the
   texture.
3. **Cavity AO on the truck.** (done) Weld, rasterize texels, add the signed
   dihedral cavity term, dilate. No BVH is needed yet. *Validate:* panel
   seams, wheel-well lips and the bed corners darken, and there are no dark
   lines along convex edges. Decide the double-sided sheet policy.
   `AoDebug::Cavity` view.
4. **Proximity AO.** (done; grid instead of a BVH) Add a per-primitive triangle BVH and the nearest-point
   query. *Validate:* seat-to-floor, bed-to-wheel-well and the cabin interior
   darken. `AoDebug::Proximity` view. Record bake time for the truck body
   on desktop.
5. **Disk cache.** Key, `assets/data/ao_cache/`, load-before-bake, worker thread,
   cancellation. *Validate:* a second launch uses the cache with no bake;
   changing any setting rebakes.
6. **Case C and merge modes.** `AoMerge::BaseColor`, and albedo transfer for
   textured `Texcoord0` surfaces; then the tyres.
7. **`Texcoord1` mode.** Add the `CpuMesh.uv1` stream and a material input
   for the AO texture. This is a renderer scope change, so it gets its own
   ticket first.
8. **Android.** Cache-only loading from assets. Decide whether to build
   `xatlas` with the NDK for runtime bakes.
9. **Optional:** built-in UV generator, hemisphere-ray proximity, skinned
   bind-pose policy.

## Performance and validation

- The truck in `Texcoord0` mode adds one texture sample per body fragment at
  most. Measure with the ship profile fixture used in
  `docs/task/lambert-toon-truck-material-experiment.md` (walking and driving,
  GTX 1080, 1920×1920 per eye, MSAA 4x), comparing AO off and on.
- Bakes never run on the render thread. Record bake time per slice for the
  truck body (2,287 triangles).
- AO must not split the body batch into more draws. One baked mesh plus one
  texture replaces the existing combined mesh.
