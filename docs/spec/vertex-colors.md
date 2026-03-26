# Vertex Colors

Per-vertex RGBA color support for toon mesh pipelines.

---

## Overview

Currently `ColorComponent` provides one flat `rgba` per renderable (per-instance). There
is no per-vertex color — `CpuVertex` has only `pos`, `uv`, and `normal`.

Per-vertex color requires a **separate pipeline** because the vertex input layout is baked
into the Vulkan pipeline object at creation time. It cannot be toggled at runtime.

---

## ECS API

```
VertexColorsComponent
  colors: Vec<[f32; 4]>      // one RGBA per vertex, linear
  handle: VertexColorsHandle  // stable identity, assigned on registration
```

`VertexColorsHandle` is a registered asset handle (opaque `u32` index into
`RenderAssets`), consistent with how `CpuMeshHandle` works:

```rust
let handle = render_assets.register_vertex_colors(colors); // -> VertexColorsHandle
```

A handle is a **stable identity for a color pattern**. In MMS, the same
`VertexColorsHandle` can be attached to multiple renderables — they will all resolve to
the same baked `CpuMeshHandle` and be instanced together automatically.

`i_color` from `ColorComponent` remains available as a per-instance tint multiplied on
top of the vertex colors in the shader.

---

## Instancing

Vertex colors are baked into a new `CpuMesh` via `clone_mesh_with_vertex_colors(base,
handle)` → new `CpuMeshHandle`. Because the batch key includes `mesh`, instancing
correctness is automatic:

- Same `VertexColorsHandle` on same base mesh → same baked `CpuMeshHandle` → instanced
- Different handle → different `CpuMeshHandle` → separate draw call

No special instancing-disable logic needed. This mirrors how `UVComponent` works.

The bake cache: `HashMap<(CpuMeshHandle, VertexColorsHandle), CpuMeshHandle>` in
`RenderableSystem`.

---

## Pipelines

Two new `MaterialHandle` constants compiled at startup (+6 pipeline objects total,
opaque/cutout/transparent × 2). No new shader source files — the existing vertex shaders
are recompiled with `#define HAS_VERTEX_COLOR` via the `vulkano_shaders::shader!` macro's
`define` parameter. Fragment shader (`toon-mesh.frag`) is unchanged and shared by all
variants.

| Handle | Vertex Shader | Skinned |
|---|---|---|
| `TOON_MESH` | `toon-mesh.vert` | no |
| `SKINNED_TOON_MESH` | `skinned-toon-mesh.vert` | yes |
| `VERTEX_COLOR_TOON_MESH` | `toon-mesh.vert` + `HAS_VERTEX_COLOR` | no |
| `SKINNED_VERTEX_COLOR_TOON_MESH` | `skinned-toon-mesh.vert` + `HAS_VERTEX_COLOR` | yes |

---

## What Does NOT Change

- `InstanceData` / instance buffer layout — unchanged
- Batch building key `(material, mesh, texture, filtering, quant_steps)` — unchanged
- `toon-mesh.frag` — unchanged
- Startup pipeline count: 6 → 12 (all pre-created at boot, no per-frame cost)

---

## Implementation Checklist

- [ ] `RenderAssets::register_vertex_colors` → `VertexColorsHandle`
- [ ] `CpuVertexColored` struct with `color: [f32; 4]`
- [ ] `VertexLayout` enum on `GpuMesh` (`Standard` / `Colored`)
- [ ] `clone_mesh_with_vertex_colors` + bake cache in `RenderableSystem`
- [ ] `VERTEX_COLOR_TOON_MESH` + `SKINNED_VERTEX_COLOR_TOON_MESH` `MaterialHandle` constants
- [ ] 2 new `shader!` invocations with `define: [("HAS_VERTEX_COLOR", "")]`
- [ ] `#ifdef HAS_VERTEX_COLOR` blocks in `toon-mesh.vert` + `skinned-toon-mesh.vert`
- [ ] 6 new pipeline objects in `PipelineBundle`
- [ ] Extend material match in `record_instanced_draws_for_batches`
- [ ] `VertexColorsComponent` ECS component + `RegisterVertexColors` intent
