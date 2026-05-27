# Blend Shapes (Morph Targets) — Draft Spec

Status: **draft**. No code changes yet — this doc is for design alignment.

This spec covers adding glTF morph targets ("blend shapes") to cat-engine alongside
the existing static and skinned toon pipelines. The headline deliverable is a
**third toon mesh shader** that combines bones **and** morph targets, so that
distant / non-emotive characters can stay on the leaner skin-only pipeline and
not pay the morph cost.

## ૮₍ ˃ ⤙ ˂ ₎ა  TL;DR

- **Two new vertex shaders** (storage-format peers), not one:
  - `skinned-morph-dense-toon-mesh.vert`  — dense per-vertex × per-target deltas.
  - `skinned-morph-sparse-toon-mesh.vert` — CSR inverted list (per-vertex
    short list of `(target_idx, delta)` entries).
- **Four peer skinned-tree pipelines** total, picked per renderable:
  1. `toon-mesh.vert`                          — static
  2. `skinned-toon-mesh.vert`                  — bones only
  3. `skinned-morph-dense-toon-mesh.vert`      — bones + dense morphs   (**new**)
  4. `skinned-morph-sparse-toon-mesh.vert`     — bones + sparse morphs  (**new**)
- **Selection is mesh-driven, not authored**: an offline-style analyzer runs at
  GLTF/VRM import, scores the morph data, and picks Dense or Sparse storage per
  primitive. The result is cached to disk (asset cache) **and** in-memory
  (`MeshAssets`), so re-loads are free.
- Both morph pipelines iterate a per-instance **sparse active list** of
  `(target_index, weight)` — the active-list is sparse regardless of storage
  format. The two formats differ only in *how vertex deltas are stored*.
- Selection between morph (3/4) and non-morph (2) is a renderable property
  (`MorphTargetsComponent` present / absent). LOD policy can downgrade
  morph → no-morph at distance, and `base_mesh` → lower-res LOD independently.
- **`base_mesh` becomes authorable** so a 10k-vert head can declare an explicit
  3k-vert LOD1 / 800-vert LOD2 chain. Morph data is keyed per-LOD because the
  vertex indices change between LODs.

## 1. Terminology

| Term | Meaning |
|---|---|
| **Morph target** | A delta vertex buffer that, when added to base vertices with weight `w∈[0,1]`, produces a deformation. Often called a "blend shape" or "shape key". |
| **Target index** | Stable per-mesh index identifying one morph target (e.g. `0 = browInnerUp`, `1 = jawOpen`). |
| **Weight** | Scalar per active target, typically `[0,1]`, but glTF allows arbitrary. |
| **Active target** | A target whose weight is non-zero this frame. |
| **VRM / ARKit blendshape clip** | A named bundle mapping standard expression names → (target_index, value) pairs. Out of scope for v1; v1 exposes raw targets. |
| **Morph palette** | Per-instance list of `(target_index, weight)` consumed by the vertex shader (analogue of the skin matrices palette). |

## 2. Why a dedicated 3rd shader (vs. ubershader)

The cat-engine renderer already runs both **static** and **skinned** toon
pipelines side-by-side instead of one über-pipeline gated by `i_bones_count == 0`.
We extend the same pattern. Reasons:

- **Vertex-stage cost is per-vertex**, not per-instance. A `for` loop over a
  morph palette runs even when the palette is empty unless the driver compiles
  out the branch — which it generally can't with a dynamic SSBO length.
- **Per-vertex SSBO indirection** (4× random reads per active target) is far
  more expensive than reading from a static attribute. We want characters who
  don't need facial morphs (background NPCs, mirrors, far LODs) to skip it
  entirely.
- **Pipeline cost is fixed** at startup; we already maintain ~10 toon pipeline
  variants for transparency × cutout × emissive × clipped combos. Adding one
  more vertex shader doubles those — addressed in §7.

## 3. Vertex-attribute slot situation

The current `skinned-toon-mesh.vert` uses locations: `0, 5, 8` (pos/uv/normal),
`12, 13` (joints/weights), and `1–4, 6, 7, 9, 10, 11` (instance attributes) —
**locations 0 through 13 are committed**. Adding morph deltas as vertex
attributes would require 14+ per active target, and Vulkan only guarantees 16
total vertex input locations.

Therefore: morph deltas **must not** be vertex attributes. They go in an SSBO
indexed by `gl_VertexIndex + base_vertex_offset`.

## 4. Data layout

### 4.1 Per-mesh: morph target SSBO

```glsl
// set = 2, binding = 2  (alongside bones SSBO at binding 1)
layout(set = 2, binding = 2) readonly buffer MorphDeltasSSBO {
    // Packed: for each target t in [0, target_count):
    //   for each vertex v in [0, vertex_count):
    //     vec3 delta_pos
    //     vec3 delta_normal    // optional, half-precision encoded
    //     (vec3 delta_tangent) // optional, only if material needs it
    float morph_deltas[];
} morph_ssbo;
```

`MeshAssets` records `(target_count, vertex_count, has_normals, has_tangents,
base_offset_into_global_ssbo)`. A single global SSBO holds deltas for all
morph-bearing meshes; per-instance push constants select the base offset.

**Memory budget:**
- Position-only: `12 B × verts × targets`. A 5k-vertex face mesh with 52
  ARKit blendshapes = `12 × 5000 × 52 = 3.12 MB`. Manageable.
- Position + normal: 2× → 6.24 MB.
- Position + normal + tangent: 3× → 9.4 MB. Probably skip tangents for v1.

### 4.2 Per-instance: morph palette SSBO

```glsl
// set = 2, binding = 3
struct MorphActive {
    uint  target_index;
    float weight;
};
layout(set = 2, binding = 3) readonly buffer MorphPaletteSSBO {
    MorphActive entries[];
} morph_palette;
```

Per-instance attributes:
- `i_morph_base : uint`   — first entry index for this instance
- `i_morph_count : uint`  — number of active morphs (capped, e.g. 16)

This is the **sparse** layout. The shader iterates `i_morph_count` entries
rather than scanning all targets, so an instance with one mouth-open morph
costs ~1 SSBO read per vertex per frame, not 52.

### 4.3 Vertex shader sketches

**Dense path** (`skinned-morph-dense-toon-mesh.vert`):

```glsl
vec3 morphed_pos    = in_pos;
vec3 morphed_normal = in_normal;
uint per_vert_stride = 6u; // pos.xyz + normal.xyz
uint mesh_base = i_morph_mesh_base; // start of this mesh's deltas in global SSBO
uint vert_count = i_morph_vert_count;

for (uint k = 0u; k < i_morph_count; ++k) {
    MorphActive a = morph_palette.entries[i_morph_base + k];
    uint base = mesh_base + a.target_index * vert_count * per_vert_stride
              + uint(gl_VertexIndex) * per_vert_stride;
    morphed_pos    += a.weight * vec3(morph_dense.deltas[base+0],
                                       morph_dense.deltas[base+1],
                                       morph_dense.deltas[base+2]);
    morphed_normal += a.weight * vec3(morph_dense.deltas[base+3],
                                       morph_dense.deltas[base+4],
                                       morph_dense.deltas[base+5]);
}
```

**Sparse path** (`skinned-morph-sparse-toon-mesh.vert`):

```glsl
// CSR layout per mesh:
//   vert_entry_offset[v]   -> index into morph_entries[]
//   vert_entry_offset[v+1] -> end index (so count = end - start)
//   morph_entries[i]       = { uint target_idx; vec3 dpos; vec3 dnormal; }
//
// Per-instance dense weight array (small: target_count floats):
//   weights_dense[t] is 0 for inactive targets

uint v = uint(gl_VertexIndex);
uint start = vert_entry_offset[i_mesh_csr_base + v];
uint end   = vert_entry_offset[i_mesh_csr_base + v + 1u];

vec3 morphed_pos    = in_pos;
vec3 morphed_normal = in_normal;
for (uint k = start; k < end; ++k) {
    MorphEntry e = morph_entries[k];
    float w = weights_dense[i_weights_base + e.target_idx];
    morphed_pos    += w * e.dpos;
    morphed_normal += w * e.dnormal;
}
```

Morphs are applied **before** skinning so deltas live in bind-pose space —
matches glTF semantics.

### 4.4 Sparse storage (CSR inverted list)

The sparse format stores morphs as one flat array of `(target_idx, dpos, dnormal)`
entries, with per-vertex offsets that say "vertex `v` owns entries
`[offset[v], offset[v+1])`." This is identical to CSR sparse matrix storage.

Per mesh:
- `vert_entry_offset[vertex_count + 1] : uint`  — prefix sum, ~`verts × 4 B`
- `morph_entries[total_nonzero_entries] : MorphEntry` — `~28 B` per entry

Per instance:
- `weights_dense[target_count] : float` — small (e.g. `67 × 4 B = 268 B`)
- `i_mesh_csr_base : uint`   — start of this mesh's offset table in the global CSR buffer
- `i_weights_base : uint`    — start of this instance's weights in the global weights buffer

Trade vs dense (recap from `§8.2`):
- Bandwidth (theoretical): sparse wins; only reads entries that actually exist for this vertex.
- Bandwidth (effective): partially eaten by **gather access**; warp threads
  read from different regions of `morph_entries[]`. Triangles are spatially
  coherent so this hurts less than it sounds, but is real.
- Warp utilization: divergent — the warp's loop count = max `entries_per_vertex` in the warp.
- VRAM: scales with **actual sparsity**, not `verts × targets`.

### 4.5 Mesh analyzer + storage selection

#### 4.5.1 What the analyzer computes

At import (post-decode, pre-upload), for each primitive that carries morph targets:

```rust
pub struct MorphAnalysis {
    pub vertex_count: u32,
    pub target_count: u32,
    pub nonzero_entries: u32,         // |delta_pos| > eps_pos OR |delta_normal| > eps_normal
    pub density: f32,                 // nonzero_entries / (vertex_count * target_count)
    pub max_entries_per_vertex: u32,
    pub p95_entries_per_vertex: u32,  // 95th percentile — drives warp-divergence estimate
    pub estimated_dense_bytes: u64,
    pub estimated_sparse_bytes: u64,
    pub chosen: MorphStorage,         // Dense | Sparse
    pub reason: &'static str,         // short audit string for the inspector
}
```

`eps_pos` defaults to `1e-5 × mesh_bbox_diagonal`, `eps_normal` to `1e-3`. These
are tunable in `config/morph_analyzer.toml`.

#### 4.5.2 Selection heuristic (v1)

```rust
fn select_storage(a: &MorphAnalysis) -> (MorphStorage, &'static str) {
    if a.density > 0.35              { return (Dense,  "density > 35%"); }
    if a.p95_entries_per_vertex > 24 { return (Dense,  "warp divergence risk"); }
    if a.estimated_dense_bytes < 2 * 1024 * 1024 { return (Dense, "small enough"); }
    (Sparse, "sparse wins on VRAM")
}
```

Heuristic constants are version-stamped in the cache (see §4.5.4); a knob change
invalidates the cached choice.

#### 4.5.3 Authoring override

`MorphTargetsComponent` carries `storage_override: Option<MorphStorage>` for cases
where the heuristic picks wrong. MMS exposes `.dense()` / `.sparse()` builders.

#### 4.5.4 Disk + in-memory cache

Two-tier cache, keyed by mesh content hash:

| Tier | Lives in | Lifetime | Purpose |
|---|---|---|---|
| In-memory | `MeshAssets::morph_analysis: HashMap<MeshContentHash, Arc<MorphAnalysis>>` | until process exit / asset eviction | Avoid re-running the analyzer when the same mesh is re-instanced (multi-character scenes sharing a face base). |
| On disk | `assets/.cache/morph/<mesh_hash>.<analyzer_version>.bin` | until cleared | Avoid re-running on cold load; survives restarts. Bincode-serialized `MorphAnalysis`. |

Hash inputs:
- mesh vertex positions + indices (canonical content hash)
- morph delta arrays (positions + normals) in target-index order
- analyzer version constant (bumped when heuristic changes)
- eps_pos / eps_normal (so a tuning change invalidates)

Cache lookup flow at import:

```text
mesh content hash → 
  in-memory hit?  → use it
  else disk hit?  → load + populate in-memory
  else            → run analyzer; write disk; populate in-memory
```

Same primitive shows up across multiple `.glb` files? Same hash → shared
analysis. Same primitive at different LODs? Different hash (vertex indices
differ) → separate analyses. That's correct: storage choice is per-LOD.

#### 4.5.5 Analyzer cost

`O(verts × targets)` linear scan with one `|delta| > eps` check per cell.
For 10k verts × 67 targets = 670k cells, ~5 ms cold on a typical CPU.
Trivial vs. mesh decode + GPU upload time.

### 4.6 Memory variants (orthogonal)

- **f16 deltas**: `VK_KHR_shader_float16_int8` is widely available. Halves
  storage for both Dense and Sparse. Quality loss invisible for head-sized
  models. Per-mesh flag, picked by analyzer based on bbox diagonal.
- **No-normal-delta**: position-only deltas, recompute normals from skin.
  Acceptable for toon shading (mostly diffuse + rim), wrong for
  normal-mapped or PBR faces. Per-mesh flag, opt-in via authoring or material.
- These compose: a mesh can be Sparse + f16 + no-normal, which is the
  minimum-VRAM corner.

## 5. ECS surface

### 5.1 New components

- `MorphTargetsComponent` — sibling of `SkinnedMeshComponent`. Holds:
  - `targets: Vec<MorphTargetDef { name: String }>` (names from glTF / VRM)
  - `weights: Vec<f32>` (one per target, dense host-side; sparsified at upload)
  - `dirty: bool`
- `MorphWeightComponent` (optional, for animation routing) — references
  `MorphTargetsComponent` by `ComponentRef` and exposes a single
  `(target_name_or_index, value)` so it can be a child of an animation
  keyframe the way `MusicNoteComponent` is for audio.

### 5.2 Intents

- `IntentValue::SetMorphWeight { target: ComponentRef, index: u32, value: f32 }`
- `IntentValue::SetMorphWeights { target: ComponentRef, weights: Vec<f32> }`
  (bulk path for animation lookahead)

Both are routable through the signal pipeline like `UpdateTransform`, so
gestures / expression mappers can intercept and remap.

### 5.3 System: `MorphWeightSystem`

- Owns per-instance dirty flags.
- On tick: walks dirty `MorphTargetsComponent`s, sparsifies (`|w| > epsilon`),
  caps to `MAX_ACTIVE_MORPHS` (largest |weight| wins), writes into a
  per-frame-slot host-visible buffer, hands `(base, count)` to
  `VisualWorld::set_morph_palette(handle, ...)`.

### 5.4 Renderable pipeline selection

`RenderableSystem` picks the pipeline from the combo of `(has_skin,
has_morph, MorphAnalysis.chosen)`:

| has_skin | has_morph | storage | pipeline                                  |
|---|---|---|---|
| no  | —   | —       | `pipeline_toon_mesh` |
| yes | no  | —       | `pipeline_skinned_toon_mesh` |
| yes | yes | Dense   | `pipeline_skinned_morph_dense_toon_mesh`  |
| yes | yes | Sparse  | `pipeline_skinned_morph_sparse_toon_mesh` |

The storage choice is **per-mesh, not per-instance** — all instances of the
same mesh use the same pipeline. Different meshes on the same character (face
sparse, hair no-morph, body no-morph) get their own pipelines as usual.

A renderable can move between (2) and (3/4) at runtime by adding / removing
`MorphTargetsComponent` — useful for LOD policies (see §11).

## 6. glTF / VRM import

- glTF 2.0 morph targets attach to primitives as `targets[i].POSITION /
  NORMAL / TANGENT`. `GLTFSystem` already walks primitives — extend it to
  upload deltas into the global morph SSBO and emit a `MorphTargetsComponent`
  attached to the same node as the `SkinnedMeshComponent`.
- VRM 0.x / 1.0 expression presets map names → (mesh, target_index, value).
  Out of scope for v1; deliver as a separate `VrmExpressionComponent`
  later that emits `SetMorphWeight` intents.

## 7. Pipeline-variant explosion

Existing toon pipelines fan out across: `{static, skinned}` × `{opaque,
transparent, cutout}` × `{normal, clipped}` × `{lit, emissive}` × `{prepass,
main}` — already non-trivial. Adding **two** morph variants (dense + sparse)
to the skinned half multiplies further.

Mitigation options:

| Strategy | Pipeline count | Compile time | Memory | Notes |
|---|---|---|---|---|
| Full matrix × dense + sparse | ~30 → ~60 | +100% | bigger PSO cache | Cleanest; matches existing pattern. |
| **Morph only for opaque+cutout, lit + emissive** | +8 pipelines | minor | small | Transparent face meshes are rare; restrict surface area. **Recommended v1.** |
| Specialization constants `HAS_MORPH` + `MORPH_SPARSE` | same as existing | two extra constants per skinned variant | none | Driver may not actually DCE the SSBO loop / branch; risk benchmark required. |

The recommended option lights up morph on the {opaque, cutout} × {lit,
emissive} = 4 subsets × {dense, sparse} = **8 new pipelines** in the skinned
tree. Transparent face meshes (e.g. eyelash overlays) fall back to
non-morph; this is consistent with how VRM face primitives are typically
authored.

## 8. Performance vs ergonomics tradeoffs

Beyond the obvious "morph mesh uses more memory than non-morph mesh":

### 8.1 Bandwidth, not flops, is the bottleneck

Each active morph adds 2 random-pattern `vec3` reads per vertex (pos + normal)
from a buffer that's typically too large to stay in L1. At 5k verts × 60 fps ×
4 active morphs × 24 B = ~28 MB/s/instance — fine for one head, painful for
crowd shots. **Implication:** the per-instance LOD downgrade matters more
than the pipeline split alone. A crowd of 50 background characters all on
`skinned_morph` with `morph_count == 0` *still* pays the per-vertex loop
overhead (the loop runs zero times but the attribute fetch + branch happens) —
which is exactly why pipeline (2) exists as a peer.

### 8.2 Sparse vs dense — two independent axes

Don't conflate the two "sparse" decisions:

1. **Per-instance active-list sparsity** (§4.2). Shader iterates only
   non-zero-weight targets per instance. Always sparse — both Dense and Sparse
   storage pipelines do this. The only knob is `MAX_ACTIVE_MORPHS` (proposed: 16).
2. **Per-vertex storage sparsity** (§4.4 / §4.5). Per-mesh choice driven by
   the analyzer. Dense pays VRAM for coalesced reads + zero warp divergence;
   Sparse trades coalescing + warp uniformity for VRAM.

### 8.3 Weight upload frequency

Morph weights change every frame during animation but most frames change only
a few. Upload strategies:

- **Full rewrite per frame** (simple). Cheap if `total_active_across_all_instances`
  stays in the low thousands.
- **Per-instance dirty flags** (current pattern for transforms). Saves CPU but
  needs stable SSBO addressing; suballocator + free-list complexity.
- **Persistent mapped ring** (per-frame-slot, like the bones palette). Best of
  both. Matches how skin matrices already work — recommended.

### 8.4 LOD popping

If an animation-driven morph fades to zero and the LOD policy then drops the
`MorphTargetsComponent`, the mesh snaps back to bind pose. Mitigations:

- Hysteresis: only drop the component after `weight_sum < ε` for N frames.
- Or, drop based on screen-space size alone and clamp tiny morphs to zero
  inside the morph system regardless of LOD.

### 8.5 Normal correctness vs cost

See §4.6 for the f16 / no-normal storage variants. Summary: position-only is
cheapest but shades wrong on big deformations; position + normal is the
recommended default for toon; tangent deltas are unnecessary unless faces use
normal maps.

### 8.6 Animation pipeline interaction

`AnimationSystem` currently dispatches `UpdateTransform` for joints; morphs
need an analogous channel. Two options:

- **Direct**: `AnimationSystem` emits `SetMorphWeight` per channel per frame
  (mirrors current bone path).
- **Component-as-keyframe-child**: like `MusicNoteComponent`, treat a
  `MorphWeightComponent` as a child of a keyframe and have it auto-fire on
  beat. Lets MMS authors hand-place expressions on the timeline without an
  animation clip. **Recommend supporting both.**

### 8.7 Editor / inspector ergonomics

- Sliders for each named morph target are the obvious inspector surface.
- VRM faces typically have 52 ARKit + ~10 viseme + ~5 custom morphs = ~67
  sliders. Group by prefix (`brow*`, `mouth*`, `eye*`) to avoid a wall of
  controls.
- Live preview cost is one `SetMorphWeight` intent per slider drag — already
  cheap.

### 8.8 Serialization

- Morph weights are runtime state, not authoring data → exclude from MMS
  dump (like cached `ComponentId` fields).
- Target *names* are authoring data → keep.
- Default rest-state weights (non-zero "always on" expressions) → keep.

## 9. Open questions

1. **GPU compute pre-pass** (Option C from design discussion): Bake
   `(morph + skin)` into a transient per-instance vertex buffer once, then a
   single static-vertex pipeline reads it. Reduces pipeline count to 1 but
   adds a compute pass + a VB per instance per frame. Defer until pipeline
   count or sparse-path warp divergence becomes a measured problem.
2. **`MAX_ACTIVE_MORPHS` value**: 16 covers most ARKit faces simultaneously
   active during speech. 8 might be enough; benchmark.
3. **Re-targeting morphs** (avatar A's blendshape names driving avatar B):
   defer to a `MorphRetargetComponent` analogous to `BoneMappingComponent`.
4. **Interaction with `SkinnedMeshSystem` dirty propagation**: morph weight
   changes don't dirty any bone matrices, but they do dirty the renderable's
   morph palette upload. Need a parallel `dirty_morphs` set in `VisualWorld`.
5. **Analyzer heuristic constants** (`0.35`, `24`, `2 MB`) are guesses. Need
   to validate against real VRM face meshes once data is in hand.
6. **VRM face often splits across multiple primitives** (eyes, mouth bag,
   eyelashes). Analyzer runs per-primitive, so each gets its own storage
   choice — confirm this is what we want (vs. forcing whole-character
   coherence for inspector simplicity).

## 10. LOD and authorable `base_mesh`

The existing `RenderableComponent` already carries a `base_mesh:
CpuMeshHandle` — currently treated as a static, single-LOD asset (see
`src/engine/ecs/system/renderable_system.rs`). For 10k+ vert face meshes
(typical VRM head primitive), LOD is required for any scene with more than a
couple of full-detail characters. Morph storage decisions and LOD selection
interact, so they're specified together.

### 10.1 Authorable LOD chain

Extend `RenderableComponent` (or a sibling `MeshLODComponent`) to carry an
ordered list of mesh handles:

```rust
struct MeshLOD {
    mesh: CpuMeshHandle,
    // World-space distance at which this LOD becomes active.
    activate_at_distance: f32,
    // Optional: screen-space coverage threshold (alternative to distance).
    activate_at_screen_fraction: Option<f32>,
    // Optional: drop morphs at this LOD even if the mesh has them.
    disable_morphs: bool,
}

struct MeshLODComponent {
    lods: Vec<MeshLOD>,    // sorted: closest-first (LOD0 → LODn)
    hysteresis: f32,       // distance / fraction the camera must cross past
                           // the threshold before switching (avoids popping).
    current: u32,          // runtime: which LOD is bound right now.
}
```

MMS authoring:

```mms
let head = Mesh.gltf("avatars/rei/head.glb")
let head_lod1 = Mesh.gltf("avatars/rei/head_lod1.glb")
let head_lod2 = Mesh.gltf("avatars/rei/head_lod2.glb")

Renderable {
    base_mesh: head,
    lods: [
        (head,      0.0, disable_morphs: false),
        (head_lod1, 3.0, disable_morphs: false),
        (head_lod2, 8.0, disable_morphs: true ),  // drop morphs at LOD2
    ],
    hysteresis: 0.25,
}
```

For glTF assets that already pack LODs (the `MSFT_lod` extension or sibling
primitives by naming convention), `GLTFSystem` should auto-populate this on
import; the MMS form above is the manual override / authoring path for assets
that don't.

### 10.2 Per-LOD morph data

Morph targets are keyed by vertex index. A decimated LOD has different
vertices, so it needs **its own morph deltas** (typically retargeted from
LOD0 in the DCC tool that produced the LODs, or auto-decimated alongside the
mesh). Each LOD therefore:

- Has its own `MorphAnalysis` (separate cache entry; different content hash).
- May independently pick Dense vs Sparse storage (LOD2 with 800 verts × 67
  targets = 156 KB dense — always picks Dense; the analyzer handles this).
- May drop morphs entirely via `disable_morphs: true` on the LOD entry — the
  renderable switches to pipeline (2) `skinned_toon` for that LOD.

This is the **primary memory lever** for the 20-character / 8 GB target.
Numbers:

| Scenario | Per-character morph VRAM | × 20 chars |
|---|---|---|
| 10k-vert head, all-LOD dense, pos+normal f32     | ~16 MB | 320 MB |
| Same + f16 deltas                                  | ~8 MB  | 160 MB |
| Same + analyzer picks Sparse for LOD0 (15% density) | ~2.5 MB | 50 MB |
| Above + LOD1/LOD2 with morphs disabled at distance | ~2.5 MB only when nearby | ≤ 50 MB |

Combined: realistically 50–80 MB resident for all 20 character faces,
leaving plenty of 8 GB headroom for textures and the rest of the scene.

### 10.3 LOD selection system

Add `LODSelectionSystem`, ticked after camera but before renderable build:

1. For each `MeshLODComponent`, compute distance from camera to renderable's
   `matrix_world.translation` (or AABB center).
2. Walk `lods[]` to find the active band, applying `hysteresis` to avoid
   per-frame thrash.
3. If the chosen LOD differs from `current`:
   - Swap `RenderableComponent.base_mesh` to the chosen LOD's handle.
   - If the new LOD has `disable_morphs: true` and a `MorphTargetsComponent`
     is currently attached, detach it (or set an `enabled` flag — TBD which
     is cleaner with the rest of the system).
   - Mark the renderable as needing pipeline re-selection (same flow as
     §5.4).

### 10.4 Authoring `base_mesh` for VRM specifically

VRM 0.x / 1.0 doesn't standardize LOD packs — most authored content ships
LOD0 only. So the VRM importer should:

- Always populate LOD0 from the VRM's primary mesh primitives.
- Optionally accept a sidecar `<vrm_name>.lods.toml` that points at additional
  decimated meshes + their distance thresholds. This keeps the source `.vrm`
  unmodified and lets `cat-engine` provide LODs as an engine-side asset
  augmentation rather than a VRM-format extension.
- If no sidecar exists, `LODSelectionSystem` is a no-op and the character
  always renders at LOD0 (current behavior).

Mid-term: an offline `cargo run --bin morphmesh-decimate <input.vrm>` could
auto-generate the LOD chain + retargeted morphs and write the sidecar.

### 10.5 Interaction with `MeshAssets` morph cache

The analyzer cache (§4.5.4) is keyed by mesh content hash, which naturally
keys per-LOD. So:

- Cold load of a 3-LOD character runs the analyzer 3 times (once per LOD)
  and writes 3 cache entries.
- Reload (after restart) reads 3 entries; no work re-done.
- Sharing avatar templates across characters dedups all 3 entries.

## 11. Implementation phases

Phases 1–8 are sized to ship dense-only first, then add sparse + LOD as a
self-contained second milestone. The 20-character target is acceptance
criteria for the second milestone, not the first.

**Milestone 1: Dense morphs, single LOD**

1. Extend `CpuMesh` with optional `morph_targets: Vec<MorphTargetData>`;
   `GLTFSystem` populates from `primitive.targets`.
2. GPU upload: global dense morph SSBO in `VisualWorld` + per-mesh
   `(base, vertex_count, target_count)` lookup.
3. `skinned-morph-dense-toon-mesh.vert` + pipelines (opaque + cutout, lit + emissive).
4. `MorphTargetsComponent` + `MorphWeightSystem` + sparse active-list palette upload.
5. `SetMorphWeight(s)` intents through `RxIntentExecutor`.
6. `MorphWeightComponent` (keyframe-child sugar) + animation channel hookup.
7. Inspector sliders, prefix-grouped.

**Milestone 2: Sparse storage + analyzer + LOD**

8. Analyzer (§4.5.1–.2): produces `MorphAnalysis` per primitive at import.
9. Cache (§4.5.4): in-memory `MeshAssets::morph_analysis` + disk
   `assets/.cache/morph/<hash>.<ver>.bin`.
10. `skinned-morph-sparse-toon-mesh.vert` + matching pipelines.
11. Pipeline-selection logic in `RenderableSystem` (table in §5.4).
12. `MeshLODComponent` + `LODSelectionSystem` (§10.1–.3).
13. f16 delta variant (§4.6) gated on `VK_KHR_shader_float16_int8`.
14. VRM LOD sidecar loader (§10.4).
15. 20-character acceptance benchmark on 8 GB target.

**Milestone 3: Polish + ecosystem**

16. VRM expression mapping (separate spec, references this one).
17. `MorphRetargetComponent` (cross-avatar blendshape transfer).
18. Optional: compute-scatter pipeline (Option C from design discussion) if
    pipeline count or sparse divergence becomes a measured problem.
