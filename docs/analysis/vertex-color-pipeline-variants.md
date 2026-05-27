# Vertex Color Pipeline Variants

## Question

Do we support per-vertex colors, or only per-instance? If we wanted per-vertex color,
would that require a new pipeline (new `MaterialHandle`, different vertex input layout)?

---

## Current State: Per-Instance Color Only

`ColorComponent` provides one `rgba: [f32; 4]` per renderable. This flows into
`InstanceData.i_color` in the instance buffer and is applied as a flat multiplier in the
fragment shader — same color across every vertex of the mesh.

`CpuVertex` has `pos`, `uv`, and `normal` only. There is no vertex-level color attribute.

---

## Why Per-Vertex Color Needs Its Own Pipeline

In Vulkan, the vertex input layout is baked into the pipeline at creation time. It is not
a runtime switch. Adding `color: [f32; 4]` to `CpuVertex` produces a different vertex
input description — different attribute count, different locations, different strides —
which is incompatible with the existing `toon-mesh` and `skinned-toon-mesh` pipelines.

Therefore: **yes, per-vertex color requires new `MaterialHandle` values and new compiled
pipeline objects.** It is not a descriptor-set or push-constant change.

---

## Instancing Correctness: The UV Precedent

A concern: if vertex colors are baked into the mesh, two instances of "the same cube" but
with different per-vertex color patterns would incorrectly share a `MeshHandle` and get
batched together, rendering the wrong colors on one of them.

This is already solved by the UV override mechanism. `UVComponent` works by calling
`clone_mesh_with_uv_overrides` — it bakes the UV data into a **new `CpuMesh`**, registers
it as a new asset, and gets a new `CpuMeshHandle`. Because the batch key includes `mesh`,
two renderables with different UV patterns naturally land in different batches (different
`MeshHandle`s). Two renderables with *identical* UV patterns share a `MeshHandle` and get
instanced for free.

Per-vertex color would follow exactly the same pattern:

- `VertexColorComponent` triggers `clone_mesh_with_vertex_colors`
- Bakes color data into a new `CpuMesh` → new `CpuMeshHandle` → new `MeshHandle`
- Different color patterns → different `MeshHandle`s → different batches → correct draw
- Identical color patterns → same `MeshHandle` → instanced together automatically

No special "disable instancing" logic needed. The mesh identity system handles it.

The UV cache (`uv_mesh_cache`, keyed by `{base_mesh, uv_bits[8]}`) optimises the 4-vertex
text glyph case, but the fixed-size key only works because quads always have exactly 4
vertices. For vertex colors the vertex count is arbitrary, so a different key strategy is
needed. Two options:

**Option A — content hash (`VertexColorsHandle(u64)`)**

Hash the raw bytes of all color values into a `u64` at registration time:

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct VertexColorsHandle(u64);
```

Cache: `HashMap<(CpuMeshHandle, VertexColorsHandle), CpuMeshHandle>`. Fixed-size key
regardless of vertex count, no heap allocation, consistent with how the UV cache avoids
allocs. Tiny theoretical collision risk at u64 that is negligible in practice.

**Option B — registered asset handle (like `CpuMeshHandle`)**

Register color data into `RenderAssets`, which hands back an opaque index handle:

```rust
let handle = render_assets.register_vertex_colors(colors); // -> VertexColorsHandle(u32)
```

Cache: same map shape. Zero collision risk, supports eviction/GC, consistent with how
meshes are managed. More boilerplate.

Option A is probably sufficient — vertex colors don't need lifecycle management, the hash
is computed once and discarded, and the UV cache already accepts the same implicit
tradeoff by using raw bit values as keys.

There is a second reason to make `VertexColorsHandle` a first-class value: in MMS, the
same color pattern can be expressed once and referenced by multiple renderables. If
`VertexColorsComponent` carries a `VertexColorsHandle`, two renderables that attach the
same component (or manually specify the same handle) will resolve to the same baked
`CpuMeshHandle` and be instanced together — without the cache having to re-hash the color
data on each registration. The handle becomes a stable identity for the color pattern that
MMS code can hold onto and reuse across many spawned renderables.

---

## What Does NOT Change

**The instance buffer layout is unchanged.**

`InstanceData` is per-*instance*, not per-vertex. Each instance still has one model
matrix, one `i_color` (which remains as a per-instance tint multiplied on top of the
vertex color), opacity, and bone indices. None of that is affected.

The instance buffer binding slot (binding 1 in the non-skinned case, binding 2 in the
skinned case) stays the same. The batching key `(material, mesh, texture, filtering,
quant_steps)` also stays the same — different color patterns are differentiated by having
different `MeshHandle`s, not by adding a new batch key dimension.

---

## Pipeline Count

All pipelines are **pre-created eagerly at startup** in `VulkanoState::new()` — not on demand.
Currently there are 6: toon-mesh × 3 opacity variants + skinned-toon-mesh × 3 opacity variants.
Adding vertex-color variants brings the total to **12**, all compiled at boot.

This is purely a startup cost. Per-frame draw overhead is unchanged.

---

## Proposed Variants

Two new `MaterialHandle` constants, parallel to the existing two:

| Handle | Vertex Shader | Fragment Shader | Extra Vertex Buffer |
|---|---|---|---|
| `TOON_MESH` (existing) | `toon-mesh.vert` | `toon-mesh.frag` | — |
| `SKINNED_TOON_MESH` (existing) | `skinned-toon-mesh.vert` | `toon-mesh.frag` | skin buffer |
| `VERTEX_COLOR_TOON_MESH` (new) | `toon-mesh.vert` + `HAS_VERTEX_COLOR` | `toon-mesh.frag` | — |
| `SKINNED_VERTEX_COLOR_TOON_MESH` (new) | `skinned-toon-mesh.vert` + `HAS_VERTEX_COLOR` | `toon-mesh.frag` | skin buffer |

Each new handle also gets the same three opacity variants (opaque / cutout / transparent),
so that's +6 compiled pipeline objects total.

---

## Required Changes

### 1. New CPU vertex type + bake path

A new vertex struct (keep `CpuVertex` as-is for non-colored meshes):

```rust
pub struct CpuVertexColored {
    pub pos:    [f32; 3],
    pub uv:     [f32; 2],
    pub normal: [f32; 3],
    pub color:  [f32; 4],   // linear RGBA, per vertex
}
```

A `clone_mesh_with_vertex_colors(base_mesh, colors)` function mirrors
`clone_mesh_with_uv_overrides`: clone the CPU mesh, write colors into vertices, register
as a new asset → new `CpuMeshHandle`. For small fixed-vertex-count meshes a
`vertex_color_mesh_cache` (keyed by `{base_mesh, color_bits_per_vertex}`) avoids
re-registering identical color patterns.

`CpuMesh` could either gain an enum over its vertex type, or a separate `CpuColoredMesh`
wrapper. The GPU upload path (`RenderAssets`) needs to produce a separately-typed buffer
for these meshes — a `VertexLayout` enum on `GpuMesh` is the minimal approach:

```rust
pub enum VertexLayout { Standard, Colored }
```

### 2. Vertex shader variants — no new source files needed

Shaders are compiled by the `vulkano_shaders::shader!` proc-macro during `cargo build`
(no separate build step). The macro accepts a `define` parameter, so the colored variants
can be compiled from the *same* source files:

```rust
// in vulkano_renderer.rs

mod vertex_color_toon_mesh_vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "assets/shaders/toon-mesh.vert",
        define: [("HAS_VERTEX_COLOR", "")],
    }
}

mod skinned_vertex_color_toon_mesh_vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "assets/shaders/skinned-toon-mesh.vert",
        define: [("HAS_VERTEX_COLOR", "")],
    }
}
```

The fragment shader (`toon-mesh.frag`) is already shared by all variants and needs no changes.

The vertex shader sources (`toon-mesh.vert`, `skinned-toon-mesh.vert`) gain a guarded block:

```glsl
#ifdef HAS_VERTEX_COLOR
layout(location = 9) in vec4 in_color;
#endif

// ...

#ifdef HAS_VERTEX_COLOR
    v_color = in_color * i_color;   // per-vertex × per-instance tint
#else
    v_color = i_color;
#endif
```

So: **0 new shader source files**, 2 new `shader!` macro invocations, 6 new pipeline objects.

### 3. Pipeline objects

Six new pipelines in `PipelineBundle` (opaque/cutout/transparent × 2 new handles),
compiled with the new vertex input description.

### 4. Dispatch in `record_instanced_draws_for_batches`

The `if batch.material == SKINNED_TOON_MESH` branch becomes a match:

```rust
match batch.material {
    SKINNED_TOON_MESH | SKINNED_VERTEX_COLOR_TOON_MESH => {
        cbb.bind_vertex_buffers(0, (mesh.vertices, skin.vertices, instance_buffer))?;
    }
    _ => {
        cbb.bind_vertex_buffers(0, (mesh.vertices, instance_buffer))?;
    }
}
```

Pipeline selection similarly becomes a match over the four (× 3 opacity) handles.

### 5. Mesh component & MeshHandle

`MeshComponent` and `RenderAssets` need to know which vertex layout a given handle was
uploaded with, so the draw path can bind the right vertex buffer type. The simplest approach
is a `VertexLayout` enum on `GpuMesh`:

```rust
pub enum VertexLayout { Standard, Colored }
```

---

## Summary

| Concern | Changes needed |
|---|---|
| `InstanceData` / instance buffer | **None** |
| `CpuVertex` | New parallel struct `CpuVertexColored` |
| `MaterialHandle` | +2 constants |
| Compiled pipelines | +6 objects |
| Vertex shaders | +0 new files (2 new `shader!` macro invocations with `define`) |
| Fragment shader | **None** (reuse `toon-mesh.frag`) |
| GPU mesh upload | New upload path for colored vertex buffer |
| Draw dispatch | Extend material match arms |
| Batch building | **None** (key unchanged) |

The instance buffer is untouched. The split is entirely in the vertex buffer layout and the
pipeline objects derived from it.
