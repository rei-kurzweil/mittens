# Vulkan multiview for XR rendering

Status: future optimization. As of 2026-10-20, final output already renders directly into OpenXR image layers; stereo draw recording is still per eye. Locked Vulkano 0.35.2 exposes multiview view masks, so wrapper support is established. Device support and engine integration still need implementation and validation.

## Current renderer

`XRSwapchain` creates a single-sample array swapchain with `array_size = view_count`. Once an image is acquired and waited, `VulkanoRenderer::begin_xr_frame` borrows it through Vulkano and caches a 2D view for each eye layer.

`submit_xr_eye` records the draw stream separately for each eye:

- Without MSAA, draw into the corresponding runtime-owned image layer.
- With MSAA, resolve directly into that layer.
- With post-processing, write the final composite into that layer.

A completion-only submission/fence follows the mirror/eye submissions on the shared graphics queue. After completion, retire Vulkano tracking and release the image to OpenXR. No final eye-image copy is needed. See [submission and ownership](../task/openxr-render-submission-pipelining.md).

## Proposed multiview path

Render both eyes in one draw sequence by using array attachments and shader `gl_ViewIndex`:

1. Query/enable `multiview` and check the maximum view count.
2. Add a runtime-output image view covering both array layers. Retain the existing per-eye views for fallback and diagnostics.
3. Use array depth/stencil and, when enabled, array multisample color attachments. Resolve directly into the runtime output array.
4. Set matching `view_mask` values in `RenderingInfo` and `PipelineRenderingCreateInfo` (normally `0b11`).
5. Upload both eyes' camera data together and select it by `gl_ViewIndex` in every participating shader.
6. Record the shared draw sequence once, retaining the existing completion and OpenXR ownership protocol.

No offscreen-copy stage is needed to introduce multiview.

## Vulkan and wrapper requirements

Multiview is core in Vulkan 1.1, but its device feature must still be enabled. For older APIs, use `VK_KHR_multiview` when supported.

The engine uses dynamic rendering. Vulkano 0.35.2 exposes:

- `RenderingInfo::view_mask` in `command_buffer/commands/render_pass.rs`.
- `PipelineRenderingCreateInfo::view_mask` in `pipeline/graphics/subpass.rs`.

An Android device without dynamic rendering needs a compatible classic render-pass path independently of the multiview optimization. See the [Focus 3 audit](../../android/docs/focus-3-port-audit.md).

## Shader and scene work

Camera buffers currently select an eye for each recorded view. Multiview needs camera arrays and shader indexing, for example:

```glsl
mat4 V = camera.view[gl_ViewIndex];
mat4 P = camera.proj[gl_ViewIndex];
```

Audit ordinary, skinned, outline, clipped UI, mirror, and fullscreen-effect pipelines. Preserve shared deformation results and correctly select camera-dependent state.

Culling must retain geometry visible in either eye. Multi-layer transparency currently sorts per eye; one shared ordering may be incorrect. Keep a per-eye transparency path or implement another correct stereo policy. Mirrors and post-processing also need explicit array/per-eye handling.

## Expected gains and validation

Multiview can reduce CPU draw recording, descriptor/state setup, and submission overhead. It still rasterizes/shades both eye views; do not assume that it halves GPU time.

Keep the current direct per-eye renderer as a fallback. Compare:

- CPU command preparation/submission time and GPU time.
- Correct eye selection, depth/stencil clipping, and transparency.
- MSAA off/on, bloom off/on, and mirrors off/on.
- Repeated image acquisition and session stop/restart.

Measure the same representative scene and display cadence before claiming a performance gain.
