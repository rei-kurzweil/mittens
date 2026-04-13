# Stencil Clip — Current Status & Next Steps

Branch: `mittens`

Specs:
- `docs/draft/layout-clip-shaders+pipelines.md` — pipeline / attachment spec
- `docs/draft/stencil-clip-algorithm.md` — DFS algorithm detail
- `docs/draft/stencil-clip-progress.md` — session notes, gotchas, pseudocode
- `docs/draft/visualworld-phase-dfs-render-stream.md` — RenderOp data model rationale
- `docs/spec/render-phases.md` — phase list + overlay-is-gizmos-only policy

---

## Phase policy (important — read first)

UI panels, layout quads, and text belong in **opaque / transparent**, not overlay.
Overlay is **gizmos and debug only** (depth cleared → always on top).

Stencil clipping (`overflow: hidden`) therefore targets the **opaque phase** (and transparent
for semi-transparent UI), not overlay. The `overlay_stream` exists and works but gizmos don't
use stencil clips, so it degenerates to flat DrawBatch ops.

---

## Done

- `DEPTH_FORMAT` → `D32_SFLOAT_S8_UINT`, all depth image allocations updated
- Combined depth+stencil image view (`ImageAspects::DEPTH | STENCIL`) everywhere
- `stencil_attachment_format` + `stencil_attachment` wired in both `RenderingInfo` scopes
  (main window `vulkano_renderer.rs:2105`, XR `vulkano_renderer.rs:2682`) — Clear/DontCare
- Existing pipelines: `stencil: None` (test disabled). **Never use `StencilState::default()`**
  — its `compare_op: Never` discards all fragments.
- `VisualWorld` data model: `stencil_ref: u8`, `is_stencil_clip: bool` on `VisualInstance`;
  `stencil_ref` on `DrawBatch`; `stencil_clip_order` vec
- `pipeline_stencil_incr` / `pipeline_stencil_decr` — color off, depth off, EQUAL+INCR/DECR,
  `DynamicState::StencilReference`. Phase-agnostic: reused by opaque and overlay consumers.
- `pipeline_overlay_clipped` / `pipeline_emissive_overlay_clipped` — stencil EQUAL+Keep,
  `DepthState::simple()`, `DynamicState::StencilReference`. Currently unused (no clipped UI
  in overlay), but harmless.
- `RenderOp` enum + `overlay_stream` / `overlay_stream_instances` on `VisualWorld`
  — stream builder: `build_overlay_render_stream`, `build_overlay_level`, `append_stream_batches`
  — public `overlay_stream() -> (&[RenderOp], &[u32])`

Vulkano gotchas already resolved:
- `compare_op` lives in `StencilOps`, not `StencilOpState`; default is `Never` — set explicitly
- `dynamic_state` field is `foldhash::HashSet` — clone existing + insert, don't construct fresh

---

## Next 1 — Remove `OverlayComponent` from UI

Files: `src/engine/ecs/system/inspector_system.rs`, `src/engine/ecs/system/gltf_system.rs`

Remove the `OverlayComponent` nodes so UI renders in opaque (depth-tested against scene):

| Location | Variable | Action |
|---|---|---|
| `inspector_system.rs` | `world_panel_overlay` | remove component + reparent children |
| `inspector_system.rs` | `inspector_panel_overlay` | remove component + reparent children |
| `gltf_system.rs` | `viz_overlay` | remove component + reparent children |

Keep `OverlayComponent` in `gizmo_system.rs` (`gizmo_overlay`) — that's correct.

After removal, verify the inspector panel and world panel render correctly in the opaque pass
and are occluded by 3D geometry.

---

## Next 2 — `opaque_stream` on `VisualWorld`

File: `src/engine/graphics/visual_world.rs`

Add alongside the existing `overlay_stream`:

```rust
opaque_stream: Vec<RenderOp>,
opaque_stream_instances: Vec<u32>,
```

The builder is identical to the overlay stream — generalize `build_overlay_render_stream`
into `build_phase_render_stream(instances, order)` and call it for both.
`opaque_order` is sorted by `(stencil_ref, material, tex, mesh, ...)` — same as overlay.

Rebuild in `rebuild_draw_cache` after `build_draw_batches_for_order` for the opaque pass.

Public accessor:
```rust
pub fn opaque_stream(&self) -> (&[RenderOp], &[u32])
```

---

## Next 3 — `pipeline_opaque_clipped` / `pipeline_emissive_opaque_clipped`

File: `src/engine/graphics/vulkano_renderer.rs`

Clone `pipeline_toon_mesh` / `pipeline_emissive_toon_mesh` (not the overlay variants),
override `depth_stencil_state` with stencil EQUAL+Keep + `DepthState::simple()`,
add `DynamicState::StencilReference`. Identical shape to `pipeline_overlay_clipped` but
based on the opaque pipeline (depth write ON, same blend state as opaque).

Add struct fields and initialize alongside the existing stencil pipelines.

---

## Next 4 — Draw loop for opaque

File: `src/engine/graphics/vulkano_cbb.rs`, fn `record_opaque_draws`

Replace the single `record_instanced_draws_for_batches` call with a loop over `opaque_stream`.
See `stencil-clip-progress.md` §Next: Draw Loop for the full pseudocode — same pattern as
described there but targeting `pipeline_toon_mesh` / `pipeline_opaque_clipped`.

Also update the instance buffer build in `vulkano_renderer.rs`:
- Build overlay instance buffer from `visual_world.overlay_stream().1` (not `overlay_order()`)
- Build opaque instance buffer from `visual_world.opaque_stream().1` (not `draw_order()`)

Import needed (check actual path in vulkano 0.35 first):
```rust
use vulkano::command_buffer::StencilFaces;
```

---

## Next 5 — ECS + Layout

- `StencilClipComponent` — new component
- `RegisterStencilClip` / `UnregisterStencilClip` intents
- `RxIntentExecutor` handler → `VisualWorld::register_stencil_clip`
- `sync_bg_quad` in layout system — attach/detach `StencilClipComponent` based on
  `overflow: Hidden | Scroll`

See `layout-clip-shaders+pipelines.md` §ECS Changes Needed for full design.
