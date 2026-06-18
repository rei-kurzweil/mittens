# Mirror Render-Pass: Status & Next Steps

## Summary

The ECS side of mirrors is wired: `MirrorComponent` exists, `MirrorSystem` runs each frame, derives reflected camera views, registers `VisualMirror` records on `VisualWorld`, overrides the mirror surface material to `MaterialHandle::MIRROR`, and wires a `TextureComponent.render_image` with the key `capture.mirror.{guid}.color`. The renderer now consumes `VisualMirror` records, allocates per-mirror offscreen targets, renders mirror views before the main window pass, and publishes mirror color outputs into runtime texture handles. The main remaining gaps are mirror self-exclusion, mirror material/shader support, and mirror-specific render-view observability.

## What's done

| Piece | File | Notes |
|---|---|---|
| `MirrorComponent` | `src/engine/ecs/component/mirror.rs` | `quality: i32` (64..=2048, default 512). MMS: `Mirror.quality(N)`. |
| `MirrorSystem` | `src/engine/ecs/system/mirror_system.rs` | Discovers mirrors, derives reflected view/proj per source-camera eye, registers `VisualMirror`, overrides material + texture. |
| `VisualMirror` | `src/engine/graphics/visual_world.rs:43-48` | `mirror_component`, `camera`, `target_key`, `source_instance`, `resolution_scale`. Stored in `Vec<VisualMirror>` on `VisualWorld`. |
| `VisualWorld` mirror API | `src/engine/graphics/visual_world.rs:1700-1709` | `register_mirror`, `clear_mirrors`, `mirrors()`. Cleared and rebuilt every frame by `MirrorSystem`. |
| `RenderViewKind::Mirror` | `src/engine/graphics/vulkano_renderer.rs:240-244` | Enum variant exists and is now constructed from `VisualMirror` records. |
| `RenderView` struct | `src/engine/graphics/vulkano_renderer.rs:246-252` | `view`, `proj`, `viewport`, `kind`. Used by XR offscreen path already. |
| `MaterialHandle::MIRROR` | `src/engine/graphics/primitives.rs:284` | Handle 6. Set on parent renderable by `MirrorSystem`. No mirror-specific shader yet. |
| `RenderTextureProducerKind::Mirror` | `src/engine/ecs/system/render_to_texture_system.rs:25` | Classification label for the render-to-texture bridge. No mirror-specific logic attached. |
| Runtime texture key | `src/engine/ecs/system/mirror_system.rs:208` | `capture.mirror.{guid}.color`. Wired into `TextureComponent.render_image`. |
| `MirrorSystem` tick in SystemWorld | `src/engine/ecs/system/system_world.rs:1846` | Runs after light system, near end of tick. |
| Example scene | `examples/vtuber-mirror-example.{rs,mms}` | Temple + hero mirror (`Mirror.quality(1024)`). Checklist mostly done, but mirror rendering not yet observable. |
| Reflected camera derivation | `src/engine/ecs/system/mirror_system.rs:147-203` | Reflects position + basis across mirror plane, reconstructs view matrix, adjusts proj aspect. Prefers XR source camera if available, else Window. |

## What's missing

| Gap | File / area | Notes |
|---|---|---|
| Mirror render-pass scheduling | `src/engine/graphics/vulkano_renderer.rs` | `visual_world.mirrors()` is now iterated, and mirror `RenderView`s are built and rendered before the main window pass. |
| Per-mirror offscreen targets | `src/engine/graphics/vulkano_renderer.rs` | Added per-mirror target allocation/reuse via `MirrorOffscreenTargets`, based on mirror resolution scale. |
| Construction of `RenderViewKind::Mirror` | `src/engine/graphics/vulkano_renderer.rs` | Now constructed from `VisualMirror` records and used for mirror passes. |
| Mirror scene pass submission | `src/engine/graphics/vulkano_renderer.rs` | Mirror command buffers are now created and submitted before the main window render. |
| Runtime texture publication | `src/engine/graphics/vulkano_renderer.rs` + `src/engine/ecs/system/render_to_texture_system.rs` | Mirror color output is copied into the runtime texture handle for `capture.mirror.{guid}.color` if the handle exists. |
| Self-exclusion during mirror pass | `src/engine/graphics/vulkano_renderer.rs` / `src/engine/graphics/visual_world.rs` | `VisualMirror.source_instance` is still recorded but not yet used to exclude the mirror surface from its own reflection pass. |
| Mirror-specific shader | `src/engine/graphics/` | `MaterialHandle::MIRROR` has no dedicated shader/pipeline. The mirror surface still needs a shader that samples the runtime texture. |
| Render-view observability | renderer stats | No counters yet for mirror render-view count per frame. Can't answer "how many mirror scene draws this frame?" from runtime. |
| Oblique clip plane | `src/engine/ecs/system/mirror_system.rs` | Reflected projection still does not apply an oblique near clip plane. v2 work. |
| Recursion guard | renderer | No mirror-in-mirror recursion policy yet. Source instance exclusion is next. |

## Ordered next steps

The existing XR offscreen path (`render_xr_eye_offscreen` → `ensure_xr_offscreen_targets` → `build_draw_batches_command_buffer`) is the template. Each step below maps directly to something XR already does.

### Step 1: Per-mirror offscreen target allocation

Add a `mirror_offscreen: HashMap<String, MirrorOffscreenTargets>` (or similar) to the renderer, keyed by mirror GUID. Each entry holds:
- color image + view (USAGE: COLOR_ATTACHMENT | SAMPLED | TRANSFER_SRC)
- depth image + view
- cached extent

Write `ensure_mirror_offscreen_targets(guid, extent)` modeled on `ensure_xr_offscreen_targets` at `vulkano_renderer.rs:1575`. Allocate on first use, recreate if extent or format changes.

**Why first**: you can't render into nothing.

### Step 2: Construct `RenderViewKind::Mirror` from `visual_world.mirrors()`

Before the main window/XR pass, iterate `visual_world.mirrors()`. For each `VisualMirror`:
- For each eye in `mirror.camera.eyes`, build a `RenderView` with `kind: RenderViewKind::Mirror { mirror_component: ... }`, using the eye's reflected `view`/`proj` and a viewport derived from `mirror.resolution_scale` and the mirror bounds aspect.
- Call `build_draw_batches_command_buffer` for each mirror `RenderView` with the per-mirror color/depth views.
- Submit the command buffer and wait for completion (or use semaphores if the GPU timeline allows).
- After submission, copy the color result into the runtime texture identified by `mirror.target_key` (the `capture.mirror.{guid}.color` key).

**Why second**: this is the main render loop change. Once this works, mirrors produce images.

### Step 3: Publish mirror color output into the runtime texture bridge

After a mirror pass finishes, the color attachment image needs to become the texture that `TextureComponent.render_image("capture.mirror.{guid}.color")` samples. Options:
- **Copy**: `vk::cmd_copy_image` from the mirror offscreen color to a persistent `SAMPLED`-usage image, then register that image with the `TextureUploader` under the mirror key.
- **Swap**: if the offscreen color image already has `SAMPLED` usage, publish it directly as the runtime texture for this frame (same pattern as XR offscreen → swapchain copy).

The `RenderToTextureSystem` + `TextureUploader` already define the publication bridge. Wire mirror output into it with `RenderTextureProducerKind::Mirror`.

**Why third**: without this, the mirror surface samples a blank/missing texture even though the offscreen pass ran.

### Step 4: Self-exclusion in mirror passes

Pass the mirror's `source_instance` into `build_draw_batches_command_buffer` (or its batch-building step) as an exclusion set. When `RenderViewKind::Mirror { .. }`, skip any draw batch whose instance handle matches `source_instance`. This prevents the mirror surface from appearing inside its own reflection.

**Why fourth**: without this, the mirror renders itself inside itself — visually wrong, but not a crash. Can ship v1 with a known cosmetic issue if needed, but it's a small change.

### Step 5: Mirror shader / material pipeline

Add a minimal shader for `MaterialHandle::MIRROR` that samples the mirror's runtime texture using standard UV mapping on the surface geometry. This can be a very simple unlit or lit textured shader. The texture binding comes from the `TextureComponent.render_image` already set up by `MirrorSystem`.

If the engine's material system already supports textured unlit/lit materials, `MIRROR` may just need to route to the same pipeline with the runtime texture bound. Check existing material → pipeline mapping before writing a new shader.

**Why fifth**: `MirrorSystem` already forces `MaterialHandle::MIRROR` on the mirror surface. Without a shader for that handle, the surface renders as a default/untextured material. But the offscreen pass (steps 2-3) is independent of this — the mirror captures the scene regardless of what the surface looks like.

### Step 6: Render-view observability

Add counters to renderer stats:
- `mirror_logical_count` — number of `VisualMirror` records discovered this frame
- `mirror_render_view_count` — number of mirror `RenderView`s actually rendered
- `total_scene_draw_count` — window + XR + mirror views combined

Expose these through the existing `RendererStats` component or equivalent. This gives a runtime answer to "are mirrors actually rendering?" without code inspection.

**Why sixth**: observability is important for validating the pipeline, but steps 2-5 can be verified visually first. This is the lowest-priority functional step but should not be deferred indefinitely.

### Step 7: Oblique clip plane (v2)

Modify the reflected camera's projection matrix to set an oblique near plane coinciding with the mirror plane. This clips geometry behind the mirror in the reflected view, preventing objects from "leaking through" the mirror surface.

This is a well-understood projection matrix modification (Lengyel method). It can be done entirely in `MirrorSystem` when building `CameraData.proj` — no renderer changes needed.

**Why deferred**: spec says v2. Visually tolerable without it; just wastes fill rate on occluded geometry.

### Step 8: Recursion / visibility policy (v2)

Decide and enforce:
- Only render mirrors visible in the main camera frustum (frustum culling for mirror passes)
- Cap simultaneous mirror render passes
- Skip all `RenderViewKind::Mirror` instances when rendering inside a mirror pass (depth-0 recursion)

These are performance and correctness concerns for multi-mirror scenes. Single-mirror v1 can skip all of them.

## Open questions

1. **XR + desktop concurrent policy** — When both window and XR cameras are active, do mirrors derive from one source camera or both? Current `MirrorSystem` picks one (prefers XR). The correct answer may be "derive per viewer family" but this needs a decision before multi-viewer mirror rendering.
2. **Mirror target extent** — `quality` → `resolution_scale = quality / 1024.0`, but how does that translate to pixel dimensions? Options: square at `quality`×`quality`, viewport-scaled with aspect correction, or mirror-surface-aspect-matched. The aspect calculation in `MirrorSystem` suggests the third. Needs a renderer-side decision when implementing step 1.
3. **Texture publication timing** — Mirror passes must complete and publish before the main pass samples the texture. If the GPU timeline uses semaphores/fences, this is natural. If the renderer synchronizes via `wait_idle` or submission order, confirm the mirror texture is available in time for the main pass's fragment shader.
4. **Material contract** — Should `MirrorSystem` force `MaterialHandle::MIRROR`, or should mirrors work with any material that happens to sample the mirror texture? Forcing simplifies the authoring contract; not forcing allows custom mirror shaders.

## Related docs

- [docs/spec/mirror-component.md](/home/rei/_/cat-engine/docs/spec/mirror-component.md) — canonical design spec. Partially stale (says mirrors are hypothetical in `src/`, but ECS plumbing now exists). Still correct on conceptual design.
- [docs/draft/mirror-implementation-plan.md](/home/rei/_/cat-engine/docs/draft/mirror-implementation-plan.md) — early implementation sketch. Partially stale. Superseded by this doc for next-steps ordering.
- [docs/task/render-view-mirror-inventory.md](/home/rei/_/cat-engine/docs/task/render-view-mirror-inventory.md) — original inventory of code state and gaps. This doc subsumes and updates it.
- [docs/task/vtuber-mirror-example-checklist.md](/home/rei/_/cat-engine/docs/task/vtuber-mirror-example-checklist.md) — example checklist. Unchecked items (observability, regression surface) depend on steps 2-6 above.
- [docs/spec/render-to-texture.md](/home/rei/_/cat-engine/docs/spec/render-to-texture.md) — runtime-texture bridge spec. Key reference for step 3.
