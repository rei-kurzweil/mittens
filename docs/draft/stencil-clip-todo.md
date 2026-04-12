# Stencil Clip — Current Status & Next Steps

Spec: `docs/draft/layout-clip-shaders+pipelines.md`
Algorithm detail: `docs/draft/stencil-clip-algorithm.md`
Branch: `mittens`

---

## Done

- `DEPTH_FORMAT` → `D32_SFLOAT_S8_UINT`, all depth image allocations updated
  (`vulkano_swapchain.rs`, `post_processing.rs`)
- Combined depth+stencil image view (`ImageAspects::DEPTH | STENCIL`) everywhere
- `stencil_attachment_format = Some(DEPTH_FORMAT)` on the shared `PipelineRenderingCreateInfo`
  in `vulkano_renderer.rs:988` — all 15 existing pipeline variants inherit it via `.clone()`
- `stencil_attachment: Some(...)` wired in both `RenderingInfo` scopes:
  `vulkano_renderer.rs:2105` (main window) and `vulkano_renderer.rs:2682` (XR/deferred overlay)
  — load Clear (stencil=0), store DontCare
- Existing pipelines: `stencil: None` in `DepthStencilState` (test disabled = all pass,
  no writes). **Do not change to `StencilState::default()`** — its `compare_op: Never`
  would discard all fragments.
- `VisualWorld` data model: `stencil_ref: u8`, `is_stencil_clip: bool` on `VisualInstance`;
  `stencil_ref` on `DrawBatch`; `stencil_clip_order` vec; new public API

---

## Done: New Vulkan Pipelines ✓

- `pipeline_stencil_incr` / `pipeline_stencil_decr` — color off, depth off, EQUAL+INCR/DECR, DynamicState::StencilReference
- `pipeline_overlay_clipped` / `pipeline_emissive_overlay_clipped` — EQUAL+Keep, DepthState::simple(), StencilReference dynamic
- Imports: `StencilOp, StencilOps, StencilOpState, StencilState` added to `depth_stencil` use
- Note: `compare_op` lives in `StencilOps` (not `StencilOpState`) in vulkano 0.35

---

## Next: Draw Loop

File: `src/engine/graphics/vulkano_renderer.rs`
All new pipelines go in the same `VulkanoState::new()` block as existing ones (~L985).
All need `stencil_attachment_format = Some(DEPTH_FORMAT)` in `PipelineRenderingCreateInfo`
(same as the shared one — or inherit by cloning the base and overriding `depth_stencil_state`).

### `pipeline_stencil_incr` and `pipeline_stencil_decr`

Used to enter/exit clip regions. Color write off, depth off, stencil EQUAL+INCR / EQUAL+DECR.
Both use `DynamicState::StencilReference`.

Key `DepthStencilState`:
```rust
DepthStencilState {
    depth: None,   // depth test disabled
    stencil: Some(StencilState {
        front: StencilOpState {
            ops: StencilOps {
                compare_op: CompareOp::Equal,
                pass_op: StencilOp::IncrementAndClamp,  // or DecrementAndClamp
                fail_op: StencilOp::Keep,
                depth_fail_op: StencilOp::Keep,
            },
            ..Default::default()
        },
        back: /* same */,
    }),
    ..Default::default()
}
```

Color write off:
```rust
ColorBlendAttachmentState {
    blend: None,
    color_write_enable: true,
    color_write_mask: ColorComponents::empty(),
}
```

Dynamic state must include `DynamicState::StencilReference` (in addition to Viewport/Scissor).

These pipelines only need a trivial vertex shader (position only — no lighting, no texture).
Can reuse the existing toon-mesh vertex shader stages or make a minimal one.
Fragment shader output is irrelevant (masked), use any existing fs.

### `pipeline_overlay_clipped` / `pipeline_emissive_overlay_clipped`

Clone `pipeline_toon_mesh` / `pipeline_emissive_toon_mesh`, override `depth_stencil_state`:

```rust
DepthStencilState {
    depth: Some(DepthState::simple()),  // same as overlay
    stencil: Some(StencilState {
        front: StencilOpState {
            ops: StencilOps {
                compare_op: CompareOp::Equal,
                pass_op: StencilOp::Keep,
                fail_op: StencilOp::Keep,
                depth_fail_op: StencilOp::Keep,
            },
            ..Default::default()
        },
        back: /* same */,
    }),
    ..Default::default()
}
```

Also add `DynamicState::StencilReference` to these pipelines.

The non-emissive/emissive split mirrors the existing overlay pipelines exactly.
Skinned variants can wait — add `pipeline_skinned_overlay_clipped` later if needed.

---

## After Draw Loop: Draw Loop (formerly "After Pipelines")

File: `src/engine/graphics/vulkano_cbb.rs`, fn `record_overlay_draws`

See `stencil-clip-algorithm.md` §4 for the DFS clip-stack pseudocode.
Short version: iterate overlay instances in order; maintain a clip stack;
on `is_stencil_clip=true`, INCR before drawing the instance's content draw,
DECR after its subtree is done. Use `set_stencil_reference` per group.

The existing `overlay_order` is already in DFS-compatible order (built from ECS tree).
Check that `stencil_clip_order` lookup works with it before writing the full loop.

---

## After Draw Loop: ECS + Layout

- `StencilClipComponent` — new component, `RegisterStencilClip` / `UnregisterStencilClip`
  intents, `RxIntentExecutor` handler → `VisualWorld::register_stencil_clip`
- `sync_bg_quad` in layout system — attach/detach `StencilClipComponent` based on
  `overflow: Hidden | Scroll`

See spec §ECS Changes Needed for full design.

---

## Vulkano type imports needed (not yet in vulkano_renderer.rs)

```rust
use vulkano::pipeline::graphics::depth_stencil::{
    StencilState, StencilOpState, StencilOps, StencilOp,
    // CompareOp, DepthState, DepthStencilState already imported
};
use vulkano::pipeline::graphics::color_blend::ColorComponents;
use vulkano::command_buffer::StencilFaces;  // for set_stencil_reference calls
```

Check `StencilOp` variants: `IncrementAndClamp`, `DecrementAndClamp` (safe),
or `IncrementAndWrap` / `DecrementAndWrap` (wraps at 255/0 — don't use these).
