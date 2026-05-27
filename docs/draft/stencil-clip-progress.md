# Stencil Clip — Progress & Next Steps

Branch: `mittens`

Related specs/drafts:
- `docs/draft/stencil-clip-todo.md` — original step-by-step checklist
- `docs/draft/stencil-clip-algorithm.md` — correctness / DFS algorithm detail
- `docs/draft/layout-clip-shaders+pipelines.md` — pipeline / attachment spec
- `docs/draft/visualworld-phase-dfs-render-stream.md` — RenderOp data model rationale
- `docs/spec/render-phases.md` — phase list, overlay-is-gizmos-only policy

---

## Phase policy update (supersedes earlier assumptions)

UI panels, layout quads, text, and buttons belong in the **opaque / transparent** phases,
not overlay. Overlay is gizmos and debug only (depth cleared → always on top).

Consequence for stencil clipping:
- Stencil clip (overflow: hidden) applies to **opaque + transparent** phases.
- The `overlay_stream` built below is correct in structure but targets the wrong phase for
  clipped UI. The equivalent `opaque_stream` is what actually needs the RenderOp treatment.
- The `overlay_stream` still exists and is used correctly for gizmos, which don't need
  stencil clipping, so it degenerates to a flat sequence of DrawBatch ops.

Immediate code consequences:
- Remove `OverlayComponent` from `inspector_system.rs` (`world_panel_overlay`,
  `inspector_panel_overlay`) and from `gltf_system.rs` (`viz_overlay`).
- Build `opaque_stream: Vec<RenderOp>` + `opaque_stream_instances: Vec<u32>` on `VisualWorld`
  using the same `build_overlay_level` / `append_stream_batches` logic (rename or generalize).
- Add `pipeline_opaque_clipped` + `pipeline_emissive_opaque_clipped` in `vulkano_renderer.rs`:
  same as `pipeline_overlay_clipped` but based on `pipeline_toon_mesh` (opaque, depth write on).
- Update `record_opaque_draws` in `vulkano_cbb.rs` to consume `opaque_stream`.

The `stencil_incr` / `stencil_decr` pipelines are phase-agnostic (no color write, no depth)
and can be reused by both opaque and overlay stream consumers.

---

## Done This Session

### 1. New Vulkan Pipelines (`src/engine/graphics/vulkano_renderer.rs`)

Four new pipelines created in `VulkanoState::new()` (~L1170 after the skinned cutout variants):

| Pipeline | Purpose | Depth | Stencil | Color write |
|---|---|---|---|---|
| `pipeline_stencil_incr` | Enter clip region | off | EQUAL→IncrementAndClamp | off |
| `pipeline_stencil_decr` | Exit clip region | off | EQUAL→DecrementAndClamp | off |
| `pipeline_overlay_clipped` | Clipped overlay draw (non-emissive) | DepthState::simple() | EQUAL→Keep | on |
| `pipeline_emissive_overlay_clipped` | Clipped overlay draw (emissive) | DepthState::simple() | EQUAL→Keep | on |

All four have `DynamicState::StencilReference` in addition to Viewport/Scissor.

Imports added to the `depth_stencil` use block:
```rust
StencilOp, StencilOps, StencilOpState, StencilState
```

**Gotcha**: in vulkano 0.35, `compare_op` lives in `StencilOps` (not `StencilOpState`).
`StencilOps::default()` has `compare_op: CompareOp::Never` — must set explicitly.

**Gotcha**: `dynamic_state` field uses `foldhash::HashSet`, not `std::collections::HashSet`.
Fix: clone `pipeline_ci.dynamic_state` (which is already the right type) and insert.

---

### 2. `RenderOp` + Overlay Stream (`src/engine/graphics/visual_world.rs`)

#### New type

```rust
pub enum RenderOp {
    EnterClip { instance_index: u32, parent_ref: u8, new_ref: u8 },
    DrawBatch(DrawBatch),
    ExitClip  { instance_index: u32, ref_value: u8 },
}
```

`DrawBatch.stencil_ref` now means the **effective** stencil ref for the draw call
(overridden for clip-source instances, which draw at `new_ref` not their own `parent_ref`).

#### New fields on `VisualWorld`

```rust
overlay_stream: Vec<RenderOp>,
overlay_stream_instances: Vec<u32>,
```

`overlay_stream_instances` holds `VisualInstance` indices; each `DrawBatch` op references a
contiguous slice `[batch.start .. batch.start + batch.count]`.

#### Public accessor

```rust
pub fn overlay_stream(&self) -> (&[RenderOp], &[u32])
```

#### Builder — three static methods

`build_overlay_render_stream` (entry): splits `overlay_order` into per-depth groups
(`non_clip_by_depth`, `clip_sources_by_depth`), then calls `build_overlay_level(0, ...)`.

`build_overlay_level` (recursive):
```
at depth D:
  DrawBatch for non-clip instances at D          (stencil_ref = D)
  for each clip source at D:
      EnterClip { src, parent_ref=D, new_ref=D+1 }
  DrawBatch for clip sources at D                (stencil_ref = D+1, visual draw inside own region)
  build_overlay_level(D+1, ...)
  for each clip source at D (reversed):
      ExitClip { src, ref_value=D+1 }
```

`append_stream_batches`: batches a pre-sorted slice of indices into `DrawBatch` ops,
using an explicit `effective_ref` argument.

Called from `rebuild_draw_cache` after `build_draw_batches_for_order` for overlay.

#### Algorithm notes / limitations

- `overlay_order` is currently sorted by `(stencil_ref, material, tex, mesh, ...)`.
  Within each depth group the material-sort order is preserved → good batch locality.
- Multiple sibling clips at the same depth all EnterClip before any content draws.
  This is correct for non-overlapping spatial regions (typical in UI layouts).
- For full sibling-clip correctness when regions overlap, `overlay_order` would need to
  be populated in true ECS DFS order rather than just stencil_ref-sorted.
  That's deferred — see `visualworld-phase-dfs-render-stream.md` §Dirty-flag Strategy.

---

## Next: Draw Loop (`src/engine/graphics/vulkano_cbb.rs`)

Function: `record_overlay_draws` (~L234)

Replace the current single `record_instanced_draws_for_batches` call with a loop over
`visual_world.overlay_stream()`:

```rust
let (ops, stream_instances) = visual_world.overlay_stream();

for op in ops {
    match op {
        RenderOp::EnterClip { instance_index, parent_ref, .. } => {
            // bind pipeline_stencil_incr
            // set_stencil_reference(StencilFaces::FrontAndBack, parent_ref as u32)
            // draw the clip source mesh (single instance at instance_index)
        }
        RenderOp::DrawBatch(batch) => {
            let pipeline = if batch.stencil_ref > 0 {
                if emissive { pipeline_emissive_overlay_clipped } else { pipeline_overlay_clipped }
            } else {
                if emissive { pipeline_emissive_toon_mesh } else { pipeline_toon_mesh }
            };
            // bind pipeline, descriptor sets
            // set_stencil_reference(StencilFaces::FrontAndBack, batch.stencil_ref as u32)
            // draw instances stream_instances[batch.start..batch.start+batch.count]
        }
        RenderOp::ExitClip { instance_index, ref_value } => {
            // bind pipeline_stencil_decr
            // set_stencil_reference(StencilFaces::FrontAndBack, ref_value as u32)
            // draw the clip source mesh
        }
    }
}
```

Import needed (not yet in vulkano_cbb.rs):
```rust
use vulkano::command_buffer::StencilFaces;
```

Check where `StencilFaces` is actually exported in vulkano 0.35 — it may be under
`vulkano::pipeline::graphics::depth_stencil` or a command buffer module.

For EnterClip/ExitClip draws: need to build a one-instance buffer for the clip source mesh.
Options:
- Reuse the existing overlay instance buffer (instance at `instance_index` is already in it)
  and draw with `instance_count=1` at the right buffer offset.
- Or build a tiny temporary buffer. The reuse approach is cheaper.

The existing instance buffer is indexed by position in `overlay_stream_instances`.
EnterClip/ExitClip reference `instance_index` directly (an index into `visual_world.instances()`),
not into `stream_instances`. Need to locate the right buffer slot.

Simplest approach: for stencil write draws, look up which slot in the uploaded instance buffer
corresponds to `instance_index`, then do a `draw_indexed` with `instance_start` = that slot,
`instance_count` = 1.

The overlay instance buffer is built from `overlay_stream_instances` (the new stream). So
`stream_instances.iter().position(|&i| i == instance_index)` gives the slot. Could precompute
a lookup table alongside the stream.

---

## After Draw Loop: ECS + Layout

- `StencilClipComponent` — new component
- `RegisterStencilClip` / `UnregisterStencilClip` intents
- `RxIntentExecutor` handler → `VisualWorld::register_stencil_clip`
- `sync_bg_quad` in layout system — attach/detach `StencilClipComponent` based on `overflow: Hidden | Scroll`

See `stencil-clip-todo.md` §After Draw Loop: ECS + Layout for full design.
