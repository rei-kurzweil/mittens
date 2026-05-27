# Task: OpenXR multiview + direct render

Date: 2026-05-27
Branch: `mittens`

This task tracks the refactor of cat-engine's OpenXR rendering path from
per-eye-pass + copy to single-pass multiview (`VK_KHR_multiview`) rendering,
ultimately landing directly in OpenXR swapchain images (no intermediate copy).

Motivation: cat-engine over ALVR feels choppier than SteamVR / wlx-overlay-s.
The bulk of the gap is duplicated CPU command-buffer work per eye, plus
~2.9 GB/s of unnecessary GPU copy bandwidth at 2×2000² @ 90 Hz.

Related docs:

- `docs/analysis/VK_KHR_multiview.md` — original investigation
- `docs/spec/render-phases.md` — render graph details
- `docs/spec/vr-input.md` — adjacent XR context

Related code:

- `src/engine/graphics/vulkano_renderer.rs` — pipelines, offscreen targets, render entry points
- `src/engine/graphics/xr_swapchain.rs` — wraps OpenXR runtime swapchain
- `src/engine/graphics/xr_renderer.rs` — `copy_offscreen_to_xr_layers`
- `src/engine/ecs/system/openxr_system.rs` — XR frame loop, layer submission
- `assets/shaders/*.vert`, `*.frag` — GLSL sources

---

## 1. Problem statement (◕‿◕✿)

The current XR path renders each eye in a separate Vulkano pass into a per-eye
offscreen image, then `vkCmdCopyImage`s eye 0 → swapchain layer 0 and
eye 1 → swapchain layer 1 before submitting to the runtime.

Cost breakdown per frame:

- **CPU**: 2× command-buffer build, 2× pipeline state binding, 2× draw-call
  iteration, 2× descriptor set rebuild.
- **GPU bandwidth**: ~16 MB / eye copy at 2000² RGBA8 → ~2.9 GB/s over 90 Hz.
  ALVR's encoder competes for that bandwidth.

ALVR's frame budget is tight; CPU dominance is the smoothest-feel killer.

---

## 2. Goal

Render both eyes in a **single pass** using `VK_KHR_multiview` (core in
Vulkan 1.1), with view selection in shaders via `gl_ViewIndex`.

Two-step rollout:

1. **Multiview into our own offscreen array image, single copy out**
   (the doc's "Option A"). Eliminates the per-eye CPU work; one copy remains.
2. **Direct render into the OpenXR swapchain image**, no copy
   (the doc's "Option B"). Eliminates remaining bandwidth.

Step 1 captures most of the CPU win and is lower risk. Step 2 is a small
follow-up once the multiview plumbing is in place.

---

## 3. Architecture decision: dual UBO

The vertex shader needs both eyes' view/proj matrices. Two options were
considered:

| Approach | Tradeoff |
|---|---|
| Unified UBO (`view[2]` / `proj[2]` everywhere) | Cleaner long-term, but touches the window path too — risk of breaking non-VR while iterating on XR |
| **Dual UBO (chosen)** | Window path 100% untouched. New `CameraXrUBO` + XR-only shader variants live in parallel. Slightly more pipeline-build code, but lower drift risk during refactor |

Both share the same descriptor set layout (set 0 binding 0 is just
`UniformBuffer` — the *shape* comes from the shader, not the layout). So XR
pipelines reuse `PipelineDescriptorSetLayouts.global` unchanged.

Long-term we may unify, but only after the XR path is stable.

---

## 4. Key findings (recon)

### Vulkano 0.35 API surface

- `PipelineRenderingCreateInfo.view_mask: u32` — exists. Set to e.g. `0b11`
  for stereo. (`vulkano-0.35.2/src/pipeline/graphics/subpass.rs:64`)
- `RenderingInfo.view_mask: u32` — exists.
  (`vulkano-0.35.2/src/command_buffer/mod.rs:613`)
- `Image::from_swapchain` is `pub(crate)` — **not** usable for foreign images.
- `RawImage::from_handle_borrowed` → `.assume_bound()` is the path for
  wrapping OpenXR-owned `vk::Image`s. (`vulkano-0.35.2/src/image/sys.rs:163`,
  `:1006`).
- `khr_dynamic_rendering` is already enabled.
- `VK_KHR_multiview` is core in Vulkan 1.1; we set
  `device_features.multiview = true` and the validator enforces it on
  non-zero `view_mask`.

### Pipeline count

The renderer builds **24 graphics pipelines** on `VulkanoState`
(`vulkano_renderer.rs:305–340`). Each XR-bound pipeline needs an XR variant
with:

- `PipelineRenderingCreateInfo.view_mask = 0b11`
- XR vertex shader (`*.xr.vert`)
- XR fragment shader where the shader reads camera UBO (only `toon-mesh.frag`
  does — emissive and unlit do not)

Best approached as a `build_xr_pipelines()` helper that mirrors the existing
window-pipeline construction block.

### Descriptor sets

Same descriptor set layouts apply to XR pipelines —
`PipelineDescriptorSetLayouts` does not need changes. The XR camera buffer
binds to `set 0, binding 0` like the window camera; only the buffer size and
contents differ.

### Bones slot consolidation

Today: `bones_slot = window_slots + eye` allocates a separate bones SSBO per
eye to avoid contention. With multiview, both eyes share one draw → one
bones slot. The XR path can drop the per-eye doubling.

### Frame loop

`openxr_system.rs:1733` per-eye loop becomes a single
`render_xr_multiview(...)` call. Layer submission at l. 1769–1792 is
unchanged: `image_array_index(0/1)` already maps to the two array layers we
render into.

### Shader layout match

The fragment shader's UBO declaration must match the byte offset of any
field it reads. `toon-mesh.frag` reads `ubo.ambient_light`, so its XR
variant must use the same `CameraXrUBO { view[2]; proj[2]; ... }` layout so
`ambient_light` lands at the right offset.

`emissive-toon-mesh.frag` and `unlit-mesh.frag` don't read the camera UBO;
they don't need XR variants.

---

## 5. Progress checklist

### ✅ Landed

- [x] **Multiview device feature enabled.** `device_features.multiview = true`
      at device creation. `vulkano_renderer.rs:489`.
- [x] **`CameraXrUBO` struct.** Mirrors XR shader UBO layout
      (`view[2]` + `proj[2]` + tail). `vulkano_renderer.rs` (right after
      `CameraUBO`).
- [x] **XR shader variants.** `assets/shaders/toon-mesh.xr.vert`,
      `toon-mesh.xr.frag`, `skinned-toon-mesh.xr.vert`. Use
      `GL_EXT_multiview`, index `view[gl_ViewIndex]` / `proj[gl_ViewIndex]`.
- [x] **`vulkano_shaders::shader!` modules** for the three XR variants
      wired in `vulkano_renderer.rs` (`toon_mesh_xr_vs`, `toon_mesh_xr_fs`,
      `skinned_toon_mesh_xr_vs`).
- [x] **Vulkano 0.35 multiview/foreign-image API verified.**

Everything above compiles clean. The XR shader modules and `CameraXrUBO` are
not yet *referenced* anywhere — they're scaffolding for the next phase.

### 🔧 Remaining

- [x] **XR pipeline variants.** `XrPipelines` struct holds 24 parallel
      pipelines, all with `view_mask = 0b11`. Built eagerly at init alongside
      the window pipelines. Stored as `VulkanoState.xr_pipelines: XrPipelines`.
      Uses `make_xr_ci` closure to mirror each window pipeline template with
      XR shader stages + multiview rendering info. Compiles clean.
- [x] **Multiview offscreen targets.** `XrMultiviewTargets` struct +
      `ensure_xr_multiview_targets(view_count, extent)` method. Builds:
  - color: `Dim2d`, `array_layers = view_count`, `TRANSFER_SRC | COLOR_ATTACHMENT`
  - depth: `Dim2d`, `array_layers = view_count`, `DEPTH_STENCIL_ATTACHMENT`
  - if MSAA: MSAA color array + resolve target = single-sample color array
  - all views built with `view_type = Dim2dArray`, `array_layers = 0..view_count`
  - `xr_multiview_color_vk_image()` accessor for the copy-out step
  - state lives parallel to existing `xr_offscreen` (per-eye fallback)
- [x] **`render_xr_multiview` entry point.** New method on `VulkanoRenderer`.
      Ensures multiview targets, runs `build_draw_batches_command_buffer`
      once with `is_xr_multiview = true`, submits + waits. Exposed on the
      outer wrapper as `VulkanoRenderer::render_xr_multiview`. Notes:
  - `build_draw_batches_command_buffer` got an `is_xr_multiview: bool`
    parameter. At top of fn: 8 local pipeline selectors clone the right Arc
    (`p_toon_mesh`, `p_emissive_prepass_toon_mesh`, etc); pipeline use sites
    swapped to those locals.
  - Camera UBO branches inline: XR path fills `CameraXrUBO` (both eyes
    read via `camera_view_for_eye(Xr, 0/1)`); window path keeps `CameraUBO`.
    Buffer erased to `Subbuffer<[u8]>` via `.into_bytes()` so both branches
    feed the same descriptor write.
  - All 3 `RenderingInfo` constructions (main, bloom, overlay) now use
    `xr_view_count` + `xr_view_mask` locals (0b11 / 2 in XR, 0 / 1 in window).
  - Bones slot: multiview uses a single shared slot (no per-eye doubling).
  - Post-processing in the multiview path is **deferred** — passed as `None`.
    The existing `PostProcessingRenderer` assumes per-eye single-layer
    targets; needs separate work to be multiview-aware.
- [x] **Multiview-aware copy.** New `copy_multiview_to_xr_layers` in
      `xr_renderer.rs`. Source = the multiview color array image
      (via `renderer.xr_multiview_color_vk_image()`). Single multi-layer
      `vkCmdCopyImage` region with `layer_count = view_count`. Same layout
      barrier dance as the per-eye copy, but applied once instead of per-eye.
- [x] **Thread multiview through `vulkano_cbb.rs` helpers.** Initial bring-up
      had `build_draw_batches_command_buffer` multiview-aware but missed that
      `record_opaque_draws`, `record_cutout_draws`, `record_overlay_draws`,
      `record_transparent_single_draws`, `record_transparent_multi_draws`,
      `record_background_draws`, `record_background_occluded_lit_draws`, and
      `record_phase_stream_draws` all hardcoded `self.pipeline_*` (window
      pipelines with `view_mask=0`). Binding those inside a multiview scope
      (`view_mask=0b11`) produced a black headset image. Fix: added a `pipe!`
      macro in `vulkano_cbb.rs` that picks from either `VulkanoState` or
      `XrPipelines` by field name; threaded `is_xr_multiview: bool` through
      every helper. Also widened `ClearRect.array_layers` to span both layers
      in multiview.
- [x] **Switch OpenXR frame loop.** `openxr_system.rs:1733` now branches on
      `self.xr_multiview_enabled`. Multiview path: one
      `render_xr_multiview` + one `copy_multiview_to_xr_layers`. Fallback
      path: the original per-eye loop + per-eye copy. Layer submission
      (l. 1769-1792) is unchanged — `image_array_index(0/1)` still references
      the two layers, regardless of how they were written.
- [x] **Fallback toggle.** `OpenXRSystem.xr_multiview_enabled: bool`
      (default `true`). Flip to `false` to bisect rendering issues against
      the proven per-eye path.
- [x] **Verified on ALVR.** Renders correctly, stereo is correct (confirmed
      via gl_ViewIndex tint diagnostic: left eye → red, right eye → green
      stably). Holds 60 fps (the ALVR stream rate) steadily. GPU wall-clock
      per frame is ~1.3ms multiview vs ~2-4ms per-eye total — multiview is
      actually faster.
- [ ] **Known caveat: motion ghosting.** During head rotation/translation
      the multiview path shows a "ghost of geometry" that intensifies with
      angular velocity, visible in each eye independently. The per-eye path
      doesn't ghost during rotation (smooth) and only has mild jerkiness
      during translation. Hypothesis: this is an ALVR/runtime interaction
      with our multiview-array submission timing — both paths produce
      identical swapchain content with identical poses, but ALVR's
      reprojection seems to behave worse with our multiview path despite
      it being faster wall-clock. Multiview ships behind
      `OpenXRSystem.xr_multiview_enabled` (default `true`); flip to `false`
      to fall back to the proven per-eye path. Investigation continues —
      candidates: pose-prediction timing, ALVR-specific multiview quirks,
      direct-render to swapchain (Option B) sidestepping the array image
      altogether.
- [ ] **Multiview post-processing** (separate task) — `PostProcessingRenderer`
      currently assumes per-eye single-layer targets. Multiview path runs
      with post-process disabled. To re-enable: bloom/composite passes need
      `view_mask = 0b11` and array-typed targets, or do post-process per
      array layer after the multiview render.

### 🚀 Direct-render follow-up (after Option A is stable)

- [ ] **Wrap OpenXR swapchain images for Vulkano** — `XRSwapchain` exposes
      `vulkano_image_view(index: usize) -> Arc<ImageView>` built via
      `RawImage::from_handle_borrowed(device, vk_image, ImageCreateInfo {
      image_type: Dim2d, extent: [w, h, 1], array_layers: view_count,
      usage: COLOR_ATTACHMENT, initial_layout: ColorAttachmentOptimal, ... }
      ).assume_bound()` → `ImageView::new(image, ImageViewCreateInfo {
      view_type: Dim2dArray, ... })`.
- [ ] **`render_xr_multiview` accepts an external color view** — point
      attachment at the swapchain view; depth stays our own array. Drop the
      offscreen color array image entirely.
- [ ] **Layout barriers** — OpenXR contract says acquired image is in
      `COLOR_ATTACHMENT_OPTIMAL`, must be returned in same. Vulkano auto-
      tracking on `from_handle_borrowed` images is the risk; verify, fall
      back to manual `ash` barriers if it fights us.
- [ ] **Delete `copy_offscreen_to_xr_layers` on the direct path.**

---

## 6. Spec quirks worth remembering

- **`view_mask` is on both PipelineRenderingCreateInfo *and* RenderingInfo,
  and they must match.** Set to `0b11` for stereo on both.
- **When `RenderingInfo.view_mask != 0`, `layer_count` must be `1`.** The
  layer count is implied by the bitmask. (Discovered the hard way during
  bring-up — Vulkano's error message was clear: "view_mask is not 0, but
  layer_count is not 1".)
- **`vkCmdClearAttachments` in a multiview scope must use
  `ClearRect { baseArrayLayer: 0, layerCount: 1 }`.** Multiview replicates
  the clear across views automatically. Setting wider layer ranges is a
  validation error.
- **`vkCmdCopyImage` with multi-layer regions is valid per spec** but we
  use one region per layer for robustness — the explicit form is
  unambiguous across drivers.
- **`view_mask != 0` requires `device_features.multiview = true`.**
  Vulkano enforces this at pipeline build time.

## 7. Risks / open questions

- **Motion ghosting on ALVR** (see caveat above). Open investigation.
- **Vulkano layout tracking on foreign images.** Worst case we drop to
  `ash` for the begin/end-rendering on the direct-render path while
  keeping draws in Vulkano. Confirm before declaring direct render done.
- **MSAA resolve to array** — dynamic rendering supports per-attachment
  resolve; verify the resolve view can be `2D_ARRAY` matching layer count.
- **Per-eye culling.** Confirm `build_draw_batches_command_buffer` doesn't
  cull per-eye (cursory read says it doesn't). If anything does, switch to
  conservative both-eye frustum or multiview-aware culling.
- **`should_render = false` path.** Multiview path must still call
  `xrEndFrame` with no layers when `frame_state.should_render` is false.

---

## 8. References

- `docs/analysis/VK_KHR_multiview.md` — original investigation that proposed
  Options A/B. This task implements both.
- Plan file (private, not in repo):
  `~/.claude/plans/hey-claude-can-you-smooth-teapot.md`
