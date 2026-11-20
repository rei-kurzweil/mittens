# Android renderer: single-pass stereo with Vulkan multiview

Question: how hard is it to make the Android scene renderer (`android/`, ash,
classic render passes, Focus 3 / Adreno 650) draw both eyes in one render pass
with `VK_KHR_multiview` (core in Vulkan 1.1), and is it worth doing?

Short answer: the device side is small. The OpenXR swapchain is already a
two-layer array, the device reports the feature, and classic render passes
support view masks. The real cost is the MeshSurfaceV1 camera contract. Three
of the Android demo's custom fragments read `camera.view` from set 0 binding 0,
which holds one eye today. That needs an additive, versioned stereo camera
binding, and desktop has to bind it too. Recorded Focus 3 profiles show the
frame is GPU-bound and command recording costs about 0.35 ms, so expect a small
CPU gain and an unknown, probably modest, GPU gain. Do it as a measured A/B
against the current two-pass path, not as an assumed speed-up.

Related:

- [Adreno 650 optimization epic, slice 5](../../android/docs/task/epic/adreno650_rendering_optimization/README.md)
  (multiview plan and caveats)
- [Desktop multiview analysis](../analysis/VK_KHR_multiview.md)
- [Focus 3 port audit, stereo row](../../android/docs/focus-3-port-audit.md)
- [Recorded Focus 3 Vulkan capabilities](../../android/docs/focus-3-vulkan-results.md)
- [Benchmark API spec](../spec/benchmark.md)
- [Perlin noise GPU benchmark](perlin-noise-gpu-benchmark.md) (same MMS
  `Benchmark` A/B approach on XR2)

Line numbers are from the 2026-11-20 working tree. `android/src/renderer/mod.rs`,
`resources.rs`, `graphics_device.rs`, `demo_assets.rs` and `android/Cargo.toml`
were being edited at the same time (ASTC upload work), so treat their line
numbers as approximate.

## Findings

### Device support (recorded probe)

- Focus 3 reports `multiview=1` and `maxMultiviewViewCount=6`
  (`android/docs/validation/focus-3-vulkan-probe-2026-10-29.txt:121,123`; the
  same values appear in `focus-3-opaque-1x/msaa4x-2026-10-29.txt` and
  `focus-3-sakura-startup-2026-10-29.txt`). It also advertises the
  `VK_KHR_multiview` extension (`:87`). The GPU's API is 1.1.128 (`:48`), so
  multiview is core. Desktop XR reports a view count of 32
  (`desktop-xr-compare-lean-startup-2026-10-29.log:220`).
- The following are not recorded: `multiviewGeometryShader` and
  `multiviewTessellationShader` (not needed), `maxMultiviewInstanceIndex`
  (needed because draws are instanced, though the largest recorded count is
  278 instances), and any FDM/multiview-specific property.
- The feature is queried but never enabled.
  `vk::PhysicalDeviceMultiviewFeatures` is chained into the features2 query at
  `android/src/graphics_device.rs:~187-205`, and the properties query reports
  the view count at `~270-280`. At device creation (`~413-433`), the
  `DeviceCreateInfo` chains only dynamic rendering (never on Focus 3) and the
  fragment density map. The concurrent ASTC change adds
  `pEnabledFeatures` for ASTC LDR.
- To enable multiview, chain
  `PhysicalDeviceMultiviewFeatures { multiview: TRUE }` into `pNext`. Use the
  1.1 struct: `PhysicalDeviceVulkan11Features` needs 1.2. Keep the ASTC
  `pEnabledFeatures` as is, because Vulkan forbids combining `pEnabledFeatures`
  with a `PhysicalDeviceFeatures2` in the chain. Do this only when the queried
  feature is TRUE, and store `Gpu::multiview: bool` for the renderer.
  `XR_KHR_vulkan_enable2` passes this same `DeviceCreateInfo` to
  `xrCreateVulkanDeviceKHR` (`~435-445`), so no separate path is needed. Size:
  S.

### OpenXR swapchain: already one array swapchain

- There is one swapchain with `array_size: 2`, `sample_count: 1`, and usage
  `COLOR_ATTACHMENT | SAMPLED`
  (`android/src/xr_session.rs:190-203`). Projection views select eye layers with
  `image_array_index(eye)` (`xr_session.rs:334-347`). The recorded run reports
  `layers=2 images=3` at 1712x1712
  (`android/docs/validation/focus-3-lean-profile-2026-10-30.txt:5`).
- So no swapchain or composition-layer change is needed. The renderer
  currently creates two `TYPE_2D` views per runtime image
  (`base_array_layer(eye)`, `layer_count(1)`, `android/src/renderer/pass.rs:245-261`)
  and two framebuffers with `layers(1)` (`pass.rs:274-284`).
- Multiview needs one `TYPE_2D_ARRAY` view per runtime image
  (`base_array_layer 0`, `layer_count 2`) and one framebuffer per image. That
  framebuffer must keep `layers = 1`: the Vulkan spec requires
  `layers == 1` when the render pass uses non-zero view masks, and the attachment
  views must have at least as many layers as the highest view bit plus one.

### Attachments: MSAA color, resolve, depth, density map

Today `EyePass` allocates one transient depth image and one transient 4x MSAA
color image per eye (`pass.rs:219-240`). `Image::new` hard-codes
`array_layers(1)` and a `TYPE_2D` view (`android/src/renderer/resources.rs:~189,
~230-236`). Load and store ops are already tiler-friendly: MSAA color is
`CLEAR/DONT_CARE`, depth is `CLEAR/DONT_CARE` with transient memory and lazy
allocation preferred, and resolve is `DONT_CARE/STORE` inside the subpass
straight into the XR image (`pass.rs:111-151`, `mod.rs:164-178`).

For multiview:

- **MSAA color and depth.** Use one 2-layer image each, created with
  `array_layers(2)` and a `TYPE_2D_ARRAY` view, and keep
  `TRANSIENT_ATTACHMENT` with lazy memory. Total attachment size is the same as
  two separate images. Load and store ops do not change: they apply per view, so
  each layer is still cleared in tile memory and discarded. Re-check
  `get_physical_device_image_format_properties` for `max_array_layers >= 2` at
  4x (`pass.rs:67-83` currently checks only samples and extent).
- **Resolve.** No change beyond the array view. The subpass's
  `pResolveAttachments` resolves each view into the matching layer of the
  runtime image, still inside the subpass. Nothing extra is copied and no extra
  target is created.
- **Fragment density map.** Foveation builds a separate `R8G8_UNORM` map for
  each eye from that eye's asymmetric projection (`android/src/renderer/foveation.rs:95-115`;
  `upload_density` at `resources.rs:~431`). The recorded GPU readback shows the
  two eyes really differ, for example medium 1x: eye 0 has 2007040 full pixels
  and eye 1 has 1832960
  (`android/docs/validation/focus-3-vulkan-foveation-2026-11-02.txt:14-15`).
  - In a multiview pass, the spec allows the FDM attachment view to have
    `layerCount` 1 (one map shared by all views) or more than the highest view
    index. With a layered map, each view reads its own layer.
  - Recommendation: upload both maps into one 2-layer image with a
    `TYPE_2D_ARRAY` view. Do not collapse them to one shared map, which would
    silently change the per-eye profile.
  - `fragmentDensityMapNonSubsampledImages` is already required and enabled
    (`graphics_device.rs:~232-241, ~415-417`), because the runtime images are
    not subsampled. This is unchanged.
  - `VK_EXT_fragment_density_map2` limits such as `maxSubsampledArrayLayers`
    apply only to subsampled images, so they do not apply here.
  - `VK_NVX_multiview_per_view_attributes` is NVIDIA-only (desktop log `:214`)
    and not needed.
  - The FDM attachment description and
    `VkRenderPassFragmentDensityMapCreateInfoEXT` (`pass.rs:152-215`) stay as
    they are.
  - Before relying on per-layer density on this 1.1.128 driver, confirm the
    spec wording and check the behavior with the per-eye readback in
    `android/validation/foveation/probe.rs:~120-148`.

### Render pass: classic passes take view masks directly

- Chain a `VkRenderPassMultiviewCreateInfo` (core 1.1) into the existing
  `RenderPassCreateInfo` (`pass.rs:209-218`), next to the FDM info:
  - `pViewMasks = [0b11]` for the single subpass.
  - `pCorrelationMasks = [0b11]`, which tells the driver the two views are
    spatially correlated.
  - No view offsets, since there is one subpass and the dependency is external.
- No renderpass2 or dynamic rendering is needed. This fits the Focus 3
  classic-pass constraint (`android/docs/focus-3-vulkan-results.md:17-18`).
- The external `BY_REGION` dependency (`pass.rs:186-202`) can stay. Per the
  spec, a multiview pass's dependencies on itself would also need
  `VIEW_LOCAL`, but there are none.
- The view mask is part of render pass compatibility, so pipelines built for
  the two-pass `EyePass` cannot be used inside the multiview pass. The pipeline
  key `(definition, background, transparent, foveated)` (`android/src/renderer/materials.rs:38,289`)
  needs a `multiview` component. `Pipeline::new` itself needs no
  multiview-specific state (`android/src/renderer/pipeline.rs:151-163`).
- `EyePass::begin(command, index, eye, ..)` (`pass.rs:299-345`) loses its `eye`
  parameter in multiview mode. The viewport and scissor are unchanged and apply
  to every view.

### Shaders: surface.vert and the MeshSurfaceV1 camera contract

- `android/shaders/surface.vert:7-10,20` reads a single
  `Camera { mat4 view; mat4 proj; mat3 camera2d; vec2 viewport; vec2 pad; vec3 ambient; uint renderer_flags; }`
  at set 0 binding 0. That is the 208-byte `CameraUniform`
  (`android/src/renderer/view.rs:321-353`). Background draws zero the view
  translation and set flag 256 (`view.rs:345-349`, `surface.vert:22`).
- The vertex shader is compiled at build time with Vulkan 1.0 target env
  (`android/build.rs:2-19`). A multiview variant needs
  `#extension GL_EXT_multiview : require` and `gl_ViewIndex`, which shaderc
  emits as the `MultiView` capability. That is legal under 1.0 + SPV_KHR_multiview.
  Build a second SPIR-V, for example `surface-multiview.vert`, or one source
  with a define. Keep the two-pass vertex shader as is. Size: S.
- **The contract problem.** `custom_material.rs` validates any fragment
  declaration of set 0 binding 0 against the exact MeshSurfaceV1 layout: 7
  members at offsets `[0, 64, 128, 176, 184, 192, 204]`, with matrix stride and
  column-major checks (`src/engine/graphics/custom_material.rs:186-188, 343-356,
  400-421`). The Android renderer accepts only fragment bindings
  `(0,0) (0,1) (1,0) (1,1)` (`materials.rs:282-288`). In a multiview pass there
  is one descriptor set for both views, so binding 0 can only describe one eye.
- Three fragments in the Android demo asset list (`android/src/demo_assets.rs:~34-36`)
  read eye-dependent camera data:
  - `assets/shaders/pool-tiles.frag:80-81` builds the view direction from
    `ubo.view` including translation. This is eye-dependent specular.
  - `assets/shaders/anime-surface.frag:55-56` computes `view_pos` and
    `view_normal` from `camera.view`. This is eye-dependent.
  - `assets/shaders/pool-metal-matcap.frag:21` uses `mat3(ubo.view)` only. This
    is eye-independent unless the headset's eye orientations differ (unverified
    for Focus 3).

  Outside the Android set, `anime-cutout.frag`, `pickup-painted-metal.frag` and
  `halo.frag` also read `camera.view`. `camera.ambient_light` (anime-surface:24),
  `viewport`, `camera2d` and `renderer_flags` are the same for both eyes, so
  binding 0 stays correct for those fields.
- **Recommended contract change (additive, explicit).** Keep MeshSurfaceV1
  binding 0 byte-identical and add a versioned stereo camera block at set 0
  binding 2, for example
  `layout(set=0, binding=2, std140) uniform StereoCameraV1 { mat4 view[2]; mat4 proj[2]; }`
  (256 bytes). The vertex shader and any view-dependent fragment index it with
  `gl_ViewIndex`.
  - The spec defines `gl_ViewIndex` as 0 when the render pass has no view mask.
    So the same shader works in the two-pass path (fill slot 0 with the current
    eye) and on desktop (mono, or per-eye XR submissions), with no runtime
    branching.
  - Extend `validate_engine_block` with a third, exactly validated layout for
    (0,2).
  - Add a reflected `reads_eye_camera` flag for fragments that access members
    0 or 1 (`view`/`proj`) of binding 0, found by scanning `OpAccessChain` on
    that variable.
  - In multiview mode, a material with `reads_eye_camera` is rejected with an
    explicit error or foveation-style feedback. There is no silent left-eye
    fallback, per AGENTS.md.
  - Migrate the three demo fragments to binding 2.
- **Desktop parity.** The desktop renderer has no multiview anywhere. A grep
  for `multiview|view_mask|ViewIndex` in `src/` finds nothing, and desktop XR
  records each eye separately (`src/engine/graphics/vulkano_xr_submit.rs:5-90`).
  Desktop needs no multiview to stay compatible. It only has to bind and fill
  the new binding 2 (slot 0 = the current eye; both slots for mono), and its
  set 0 layout gets one more uniform binding. Size of the contract work: M,
  spanning the engine crate, the desktop bindings, the Android materials and
  three shaders.
- Other options considered and not recommended:
  - Widening the MeshSurfaceV1 block itself. This breaks every existing custom
    fragment and the validation.
  - Rewriting the SPIR-V to index binding 0 by ViewIndex. This is fragile.
  - Using binding 0 as "left eye" in multiview. This silently gives wrong
    stereo shading.

### Draw recording: what disappears

The current loop (`android/src/renderer/mod.rs:~328-390`) runs `for eye in 0..2`,
and for every batch issues `cmd_bind_pipeline`, `cmd_bind_descriptor_sets`
(camera set `[eye][background]` + material set, `~355-365`), two vertex/index
buffer binds and `cmd_draw_indexed`. `stats.draws` counts both eyes (`~386`;
`scene_runtime.rs:218` says so).

With multiview:

- There is one `cmd_begin_render_pass`/`cmd_end_render_pass` instead of two.
  Each batch is bound and drawn once, which halves bind, descriptor and draw
  commands. `stats.draws` halves for the same scene, so label the profile
  output by mode.
- Camera sets go from `[2][2]` (eye × background, `materials.rs:37,161-184`) to
  `[2]` (background). Each set carries binding 0, plus binding 2 holding both
  eyes. `update_camera` writes two buffers instead of four (`mod.rs:~275-282`).
- The instance buffer is already packed once and shared by both eyes
  (`mod.rs:~283-309`). That does not change.
- Culling: the Android renderer does no frustum culling. It draws every
  prepared batch for both eyes, and the pipeline only does back-face culling
  (`pipeline.rs:124-132`). Nothing changes now. If culling is added later, a
  multiview draw has to use a combined frustum that covers both eyes.
- Transparency: only single-layer transparency is accepted (`mod.rs:~199-235`),
  and `transparent_single_stream` is already one ordering shared by both eyes.
  Multiview does not change that.
- GPU timing: the timestamps already bracket the whole command buffer, TOP_OF_PIPE
  to BOTTOM_OF_PIPE (`android/src/renderer/gpu_timing.rs:55-77`), so the
  numbers are directly comparable between modes. Only the comments saying "two
  eye passes" need to change. `stats.gpu_ms` already feeds the MMS benchmark
  through `benchmarks.record_frame` (`android/src/scene_runtime.rs:206-213`).

### Expected benefit on Adreno 650 (qualitative)

Measured baseline, worn, 1712x1712 per eye, 4x MSAA, two-pass path:

- Lean scene (`android/docs/validation/focus-3-lean-profile-2026-10-30.txt:8-23`):
  `record_cpu_ms` 0.33-0.38, `renderer_prepare_cpu_ms` about 0.10,
  `gpu_eye_pass_ms` about 9.2-15.3, `update_cpu_ms` about 3.4-3.9, and 20 draws
  (10 per eye), 62 instances.
- UI + collision (`android/docs/validation/focus-3-ui-collision-profile-2026-10-30.txt:9-15`):
  `record_cpu_ms` about 0.40, GPU about 11.4-15.1, `update_cpu_ms` about 9.6-10.8,
  and 76 draws (38 per eye), 278 instances.

What this means:

- **CPU.** Recording would at most roughly halve an already small ~0.35-0.40 ms.
  Engine update (3.4-10.8 ms) and the blocking fence wait dominate. There is
  already only one submission per frame, so multiview does not reduce
  submissions. Driver cost of per-render-pass setup also goes down (one begin/end
  instead of two), but the profile does not isolate it.
- **GPU.** The frame is GPU-bound: `submit_and_fence_wait_ms` is about the GPU
  time. Most GPU cost is fragment shading, 4x MSAA and resolve for two
  1712x1712 layers, and multiview does nothing for that (foveation does).
  Possible gains are one pass setup instead of two, and possibly shared vertex
  fetch and setup across views. It is not known whether this driver shares
  vertex work or bins each view separately (on a tiler each layer still gets
  its own tiles, clear and resolve), and with 62-278 instances vertex work is
  probably small anyway.
- **Honest expectation.** CPU recording time should drop measurably but stay
  small. GPU change is unknown and may be within run-to-run and thermal noise
  on the current scenes. Draw-heavy scenes benefit more. The repo has no
  multiview measurement yet. Do not quote a gain until the A/B below is run.

## Recommended plan

Ordered. Each step keeps the current two-pass path working and selectable. The
mode is explicit and reported. When a mode is unavailable (for example the
feature is missing or a material reads `view` from binding 0), the result is an
error or feedback, never a silent switch.

- [ ] **1. Device (S).** Enable `PhysicalDeviceMultiviewFeatures.multiview` when
      reported, expose `Gpu::multiview`, and also log `maxMultiviewInstanceIndex`.
      Coordinate with the in-flight ASTC edit in `graphics_device.rs`.
- [ ] **2. Layered images (S).** Add a layer-count parameter to `Image::new` and
      `upload_density` (`TYPE_2D_ARRAY` view when the count is above 1). Keep the
      transient/lazy memory selection. Do this after the concurrent
      `resources.rs` changes land.
- [ ] **3. Stereo pass (M).** Build a multiview variant of `EyePass` with a
      `0b11` view mask and correlation mask, one 2-layer depth and MSAA image,
      one array view and framebuffer per runtime image (`layers=1`), and the
      same load/store/resolve ops. Keep the type and file cohesive: a
      constructor variant, not a new per-primitive module.
- [ ] **4. Layered foveation (S).** Upload both eye maps into one 2-layer
      density image for the multiview pass. Extend the foveation-validation
      probe to read back both layers in a multiview pass.
- [ ] **5. Camera contract (M).** Add `StereoCameraV1` at set 0 binding 2, with
      validation in `custom_material.rs`, the `reads_eye_camera` reflection
      flag, desktop binding and fill (slot 0 = current eye), and Android
      binding. Migrate `pool-tiles.frag`, `anime-surface.frag` and
      `pool-metal-matcap.frag`. Add custom-material tests for the new layout,
      wrong layouts, and the flag. This is an explicit MeshSurfaceV1 ABI
      addition; update the material spec docs as well.
- [ ] **6. Shaders and pipelines (S).** Build-time `surface-multiview.vert`. Add
      `multiview` to the pipeline key and build pipelines lazily only for the
      active mode.
- [ ] **7. Draw loop (S).** Single-pass recording when multiview is active, with
      camera sets indexed by background only, and `Stats`/`FRAME_PROFILE`
      reporting `stereo=multiview|two_pass`.
- [ ] **8. Mode switch for A/B (M).** Start with a Cargo feature (`multiview`)
      for bring-up. Then add a runtime XR stereo-mode graphics setting modeled on
      `XrFoveationQuality` with its status feedback, so one MMS scene can switch
      modes between frames, the way foveation rebuilds its pass
      (`foveation.rs:54-115`). This avoids comparing across APK installs and
      thermal states.
- [ ] **9. Measure (M).** Run an MMS benchmark scene that alternates
      `Benchmark.start("two-pass")`/`stop` and `start("multiview")`/`stop`
      windows, with view, resolution, MSAA and foveation fixed, repeated and
      order-reversed, on the lean and UI scenes, foveation off and high. Record
      `gpu_frame_ms`, `cpu_update_ms` and the `FRAME_PROFILE` `record_cpu_ms`.
- [ ] **10. Decide.** Make multiview the default only if GPU time is the same or
      better and visual checks pass. Otherwise keep it opt-in and record why.

Total: about M-L. The device, pass and draw-loop work (steps 1-4, 6, 7) is
mechanical. The contract change (step 5) and the A/B setting (step 8) carry
most of the cross-crate work and review.

## Risks and open questions

- Driver maturity: Adreno 650 on Vulkan 1.1.128 combining multiview, 4x MSAA
  in-subpass resolve, and a layered FDM is the least-tested combination here.
  The same SoC family ships multiview with FDM foveation elsewhere, but this
  driver build is old. Validate on device, including the per-layer FDM readback.
- Spec checks to do while implementing: FDM `layerCount` rules in multiview
  passes and per-view layer selection, framebuffer `layers == 1` with view
  masks, `gl_ViewIndex == 0` without a view mask, and a per-instance-index limit
  (`maxMultiviewInstanceIndex`, not recorded).
- Focus 3 eye orientations: if the two views are not parallel, even
  `mat3(view)` users like matcap become eye-dependent. Log both eye
  orientations once.
- Future features that are per-eye by nature (mirrors, post-processing,
  stencil UI clips, multi-layer transparency sorting) are rejected today
  (`mod.rs:~191-246`). Each will need an explicit multiview policy when added.
- The runtime mode switch rebuilds passes and pipelines between frames. Pipeline
  creation cost when switching must not land in measured windows: use a warmup
  window.
- The mode switch could cause an asynchronous custom-compile hitch.
  `materials.binding` returning `None` makes draws skip (`mod.rs:~336-346`), so
  a switch could show clear-only frames until pipelines exist. Pre-build both
  modes' pipelines during warmup.

## Validation plan

- Unit/host: custom-material reflection tests for `StereoCameraV1`, the
  `reads_eye_camera` flag, and rejecting migrated or unmigrated shaders in the
  wrong mode. Shader compile checks for both vertex variants
  (`glslangValidator -V`, which works without a Rust toolchain).
- Device bring-up: the log shows `multiview enabled`, pass creation, and the
  attachment policy line for the multiview pass (lazy depth and MSAA still
  true).
- Visual, worn: correct stereo depth, specular and rim on pool tiles and anime
  surfaces in each eye, background sky rotation without translation, and no
  eye swap. Compare 1x and 4x MSAA.
- Foveation: the multiview variant of the GPU readback matches the per-eye
  full/coarse pixel counts recorded in `focus-3-vulkan-foveation-2026-11-02.txt`,
  for Off/Low/Medium/High and disable/re-enable.
- Lifecycle: session stop/restart, repeated acquire/release, and teardown with
  the multiview pass active.
- Performance: the A/B benchmark windows from plan step 9, with results recorded
  below together with build type, thermal notes and the scene.

## Results

(none yet)

## Work log

- 2026-11-20: Assessment only, no code changed. Confirmed the feature from the
  recorded probe (`multiview=1`, max 6 views), found the XR swapchain is already
  a two-layer array, identified the MeshSurfaceV1 fragment-camera dependency in
  three Android demo shaders as the main design cost, and noted that the
  recorded profiles are GPU-bound with about 0.35 ms of recording. Wrote the
  plan above.
