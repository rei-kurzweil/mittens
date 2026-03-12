# VK_KHR_multiview for XR rendering

This note explores how cat-engine could use Vulkan multiview (`VK_KHR_multiview`, core since Vulkan 1.1) to speed up XR rendering.

## Why multiview helps

Typical stereo rendering does the same draw sequence twice:

- eye 0: bind pipeline, bind descriptors, draw scene
- eye 1: bind pipeline, bind descriptors, draw scene

Even if the GPU cost is similar, the CPU cost (command buffer building/submission, state setup, culling, draw-call overhead) often doubles.

**Multiview** renders multiple views (eyes) in a single render pass/rendering scope by targeting an **array image** with multiple layers and using `gl_ViewIndex` (SPIR-V `BuiltIn ViewIndex`) in shaders.

High-level result:

- build/submit one render sequence
- rasterize into layer 0 and layer 1 in one go
- reduce CPU overhead and pipeline state churn

## Current engine XR path (today)

The XR swapchain is already created as an array swapchain:

- [XRSwapchain::new](../../src/engine/graphics/xr_swapchain.rs) uses `array_size = view_count`.

However, the current rendering path is **not multiview**:

- OpenXR acquires a runtime-owned swapchain image.
- Renderer renders each eye into a separate offscreen image (`render_xr_eye_offscreen`).
- Engine copies offscreen eye 0 → XR swapchain layer 0 and eye 1 → layer 1 using `ash` copy commands.

Relevant code:

- XR per-eye render + copy loop: [openxr_system.rs](../../src/engine/ecs/system/openxr_system.rs)
- Offscreen eye targets: [vulkano_renderer.rs](../../src/engine/graphics/vulkano_renderer.rs)

This is a correct bring-up path, but it intentionally duplicates all draw work per eye.

## Two practical multiview options

### Option A (low-risk): multiview into an offscreen **array** image, then copy to XR

Keep the “copy into runtime swapchain images” model, but reduce the render work to **one** render instead of `view_count` renders.

1. Allocate XR offscreen targets as a single array image:
   - color: `2D array`, layers = `view_count`, usage = `COLOR_ATTACHMENT | TRANSFER_SRC`
   - depth: `2D array`, layers = `view_count`, usage = `DEPTH_STENCIL_ATTACHMENT`
   - if MSAA: a multisampled color array + resolve to single-sample color array
2. Begin rendering (dynamic rendering) or begin a render pass with multiview enabled.
3. Draw the scene once.
4. Copy array layer `i` into XR swapchain array layer `i`.

This removes:

- per-eye command buffer build/submit
- per-eye state binding, per-eye draw list iteration

It keeps:

- the final copy step (still required unless we render directly into OpenXR images)

### Option B (higher ambition): render directly into the OpenXR swapchain array image

If the runtime swapchain image can be wrapped as a Vulkano image view (or rendered to via `ash` pipelines), we can eliminate the copy.

This is more complex because OpenXR owns the images and the engine must:

- create a suitable `VkImageView` covering array layers
- ensure image usage/layout transitions are correct
- ensure the image is compatible with the engine’s rendering abstraction

Given the current architecture (Vulkano for draws, `ash` only for copies), **Option A** is the most straightforward first step.

## Vulkan mechanics (what changes in the API)

### Feature/extension availability

- Vulkan 1.1+ includes multiview in core.
- Otherwise: enable `VK_KHR_multiview`.
- Device feature: `VkPhysicalDeviceMultiviewFeatures.multiview = VK_TRUE`.

The engine already queries and passes OpenXR-required Vulkan extensions into renderer initialization (see [universe.rs](../../src/engine/universe.rs)).

Multiview enabling is *not* something OpenXR mandates; it is purely a renderer optimization. We should:

- enable it only when supported
- keep a non-multiview fallback path

### Render pass vs dynamic rendering

Multiview originally shipped as a render-pass feature:

- `VkRenderPassMultiviewCreateInfo` with `viewMasks` and `correlationMasks`

But cat-engine currently uses **dynamic rendering** (see comments in [vulkano_renderer.rs](../../src/engine/graphics/vulkano_renderer.rs)).

Dynamic rendering can support multiview in modern Vulkan via:

- `VkRenderingInfo::viewMask` (bitmask of active views)
- pipeline must be created for multiview-compatible rendering (implementation detail depends on wrapper)

Action item: verify what Vulkano 0.35 exposes for multiview dynamic rendering:

- a `view_mask` field in the begin-rendering / rendering info
- a pipeline create field for view mask / multiview

If Vulkano doesn’t expose multiview for dynamic rendering cleanly, we can:

- add an XR-only render pass path using a multiview render pass
- keep dynamic rendering for window rendering

### Shader changes: using `ViewIndex`

Multiview requires shaders to select per-eye camera data.

In Vulkan GLSL:

- use `gl_ViewIndex` (SPIR-V built-in `ViewIndex`)

Typical pattern:

- Store view/projection arrays in a uniform/SSBO:
  - `view[view_count]`, `proj[view_count]`
- Index by `gl_ViewIndex`:
  - `mat4 V = view[gl_ViewIndex];`
  - `mat4 P = proj[gl_ViewIndex];`

This implies:

- descriptor layout changes: camera data becomes an array (2 eyes)
- renderer must upload both eye matrices together

### Attachments: array images and resolves

For multiview you render into array layers.

- Color attachment: `VkImageView` type `2D_ARRAY`
- Depth attachment: also `2D_ARRAY`

If MSAA is enabled:

- MSAA color array as attachment
- resolve target is the single-sample color array

Both must match layer count.

### Copy step

Even in Option A, the copy step becomes simpler:

- copy from *one* offscreen color array image to the OpenXR swapchain array image
- region per layer still needed (or use one region per layer)

The current copy code already targets `dst_range.base_array_layer = eye`.

## What we’d change in cat-engine (concrete)

This is a practical implementation checklist tailored to the current code layout.

### 1) Add a multiview-capable XR offscreen target

Today: `XrOffscreenTargets` stores `Vec<Image>` per eye.

Proposed:

- Replace per-eye images with a single image that has `array_layers = view_count`.
- Keep per-eye views only if needed for debugging.

### 2) Add a renderer entry point: render all XR eyes in one pass

Today:

- `render_xr_eye_offscreen(eye)` renders exactly one eye.

Proposed:

- `render_xr_multiview_offscreen(view_count, extent)` which:
  - ensures array targets
  - begins rendering with `view_mask = (1 << view_count) - 1`
  - binds camera descriptor containing arrays
  - draws once

### 3) Update camera buffer upload path for XR

Today:

- `build_draw_batches_command_buffer(..., camera_target = Xr, eye_index, ...)`

Proposed:

- supply a camera buffer that contains both eyes
- shaders pick the right one via `gl_ViewIndex`

This also affects skinned meshes if bone matrices are currently indexed by “eye slot” (the code hints at per-eye bone slots to avoid contention).

### 4) Keep the OpenXRSystem mostly the same

OpenXR path becomes:

- render multiview offscreen once
- copy offscreen array layers to runtime swapchain layers

### 5) Fallback

Keep the current per-eye path for:

- devices without multiview
- platforms where wrapper limitations block multiview + dynamic rendering

## Risks and gotchas

- **Wrapper support**: Vulkano must expose view masks for dynamic rendering or we need an XR-specific render pass.
- **Shader work**: all pipelines used for XR need to be multiview-safe (camera indexing via `gl_ViewIndex`).
- **Culling**: if culling is currently eye-dependent, multiview needs either:
  - conservative culling for both eyes
  - multiview-aware culling (harder)
- **Depth precision and clip control**: ensure both eyes use consistent conventions.
- **Debuggability**: multiview makes “render just one eye” harder; keep a debug toggle.

## Expected payoff

- Lower CPU time for XR rendering (command buffer build + submission)
- Potentially improved GPU efficiency (fewer pipeline transitions, better cache locality)
- Simplifies the high-level XR render loop (one render, one copy)

Copy cost remains unless we move to direct rendering into OpenXR images.

## Suggested next steps

1. Add a capability probe in renderer init:
   - is multiview supported (`multiview` feature)?
2. Prototype Option A:
   - multiview offscreen array image
   - multiview shader variant
3. Add a runtime toggle:
   - `xr.multiview = on/off`
4. Measure:
   - CPU time in `OpenXRSystem::render_xr`
   - GPU time if available (timestamp queries)
