# Focus 3 Android port audit

Audited 2026-10-19 against the current repository, locally installed dependency sources, the local ALVR checkout, and primary OpenXR/Vulkan/HTC documentation. This is a source audit, not an Android build or hardware validation.

## Recommendation

Build a small Android host around Mittens, using ALVR's working Android/OpenXR integration as a reference. Share the engine's scene preparation and add a mobile renderer policy. First measure the headset's Vulkan capabilities; that decides whether the initial rendering implementation can use Mittens' dynamic rendering or needs a classic render-pass implementation.

An entirely separate scene renderer is not a prerequisite. A separate implementation for render-pass recording may be necessary for compatibility. Keep that choice below shared scene, material, and draw-stream preparation.

## What ALVR provides

The local reference at `/home/rei/_/ALVR/ALVR` supplies an Android activity entry point, packaging metadata, loader initialization, and HTC integrations. Its `alvr/client_openxr/src/lib.rs` enables `XR_KHR_opengl_es_enable`, uses `Session<OpenGlEs>`, and explicitly has a Vulkan migration TODO. `alvr/client_openxr/src/graphics.rs` binds an EGL context to the XR session. It does **not** demonstrate Vulkan dynamic rendering on the Focus 3.

Reuse the lifecycle and device-integration patterns. Replace GL context/session/swapchain construction with Vulkan equivalents. Preserve tracker and passthrough behavior only after checking their extension, graphics binding, and lifetime requirements. ALVR's video decoder and streaming transport are unnecessary for local Mittens scenes.

## Current engine constraints

Paths below are relative to the repository root. Function names identify the audited code without relying on changing line numbers.

| Area | Evidence in source | Required work |
| --- | --- | --- |
| App and packaging | `Cargo.toml` declares a desktop binary and ordinary library; it has no Android host/packaging metadata or activity dependency. `winit = "0.30"` does not select an Android activity feature. | Add a host `cdylib`, activity entry point, arm64 APK build, manifest, loader/native libraries, and app resources. If retaining Winit on Android, select its activity integration and provide the `AndroidApp`; otherwise make desktop windowing optional. |
| Desktop-bound initialization | `Universe::init_renderer_for_window`, `VulkanoState::new`, and `VulkanoSwapchainState::new` require a window and window swapchain. XR offscreen format and bone-buffer slot counts also derive from that swapchain. | Separate device/resources from output targets and frame slots. XR-only startup must work without an Android window Vulkan surface or a desktop swapchain. |
| Android OpenXR initialization | `OpenXRSystem::try_init_openxr` loads the loader and immediately enumerates extensions. It omits Android loader initialization and `khr_android_create_instance`. | Initialize Android context, call the loader's Android initialization before enumeration/instance creation, and enable Android instance creation. Locked `openxr 0.19.0` already exposes the needed helper and instance chain. |
| Vulkan binding | `try_init_openxr` unconditionally enables legacy `khr_vulkan_enable` and disables `enable2`. `try_init_session` checks the runtime-selected physical device only after Vulkano has chosen a device. | Negotiate an advertised Vulkan binding, select the runtime's physical device during device creation, and follow that binding's creation requirements. |
| Vulkan version/extensions | `VulkanoState::new` fixes the instance API maximum to Vulkan 1.2 and maps a small list of XR-required extensions, logging unknown names. | Respect the runtime/device API range and enable every required extension. Android-specific requirements need complete handling; logging an unmapped requirement is insufficient. |
| Dynamic rendering | `VulkanoState::new` requires `khr_dynamic_rendering` and its feature; graphics pipelines use `PipelineRenderingCreateInfo`. | Probe the installed driver. Vulkan support alone does not establish dynamic-rendering support. If absent, add classic render-pass/framebuffer and compatible pipeline creation paths. |
| Compute | Initialization rejects graphics queues without compute, workgroups below 64 invocations, and fewer than eight per-stage storage-buffer descriptors. Deformation resources/pipelines are initialized eagerly. | Probe these limits. Make the deformation path and its resource/pipeline creation optional or provide a lower-requirement fallback, so a static triangle can run without the full deformation feature set. |
| Depth/stencil | `VulkanoSwapchainState::DEPTH_FORMAT` fixes `D32_SFLOAT_S8_UINT` for window, XR, and effect targets. | Query format/usage/sample-count support. Consider `D24_UNORM_S8_UINT` where supported and suitable; retain stencil for clipped UI. Select format before creating dependent pipelines. |
| XR image path | `submit_xr_eye_offscreen` renders separate eye images; `submit_offscreen_copy_to_xr_and_wait` copies into the runtime's array image. `OpenXRSystem::render_xr` records each eye separately and performs one completion-fence wait for the batch. | Keep this as an initial reference, then render or resolve into acquired XR images. Manage imported-image ownership and synchronization explicitly. The fence is currently also the resource-lifetime completion proof; do not simply remove it. |
| XR samples/formats | `XRSwapchain::new` picks the runtime's recommended swapchain sample count, while the offscreen source is single-sampled and uses the window's color format. | Negotiate the XR format independently. Request a supported single-sample swapchain for the copy/resolve path; use transient multisample attachments when desired. Validate format and sample compatibility. |
| Stereo | The XR swapchain has array layers, but draw recording is per eye. No engine multiview setup is enabled. | Introduce array attachments, per-eye camera arrays, `gl_ViewIndex`, and compatible pipeline/rendering view masks. Preserve per-eye transparency sorting or another correct stereo transparency policy. |
| Assets | CLI startup, script imports, glTF, textures, and audio use filesystem paths. Texture upload supports RGBA8 and DDS BC7. | Supply packaged/app-private assets and a consistent import base. Probe compressed formats and provide supported assets/fallbacks; consider ASTC/ETC2 for a later mobile asset pipeline. |
| Audio and input | CPAL input/output use the default host. Focus 3 controller profile and optional hand integrations already exist. | Test Android context/native linkage, playback, audio timing, microphone permission, and lifecycle. Connect the existing controller actions; add optional HTC eye/passthrough integrations incrementally. |

Android loader requirements are documented in the [OpenXR loader specification](https://registry.khronos.org/OpenXR/specs/1.1/loader.html) and [Android instance extension](https://registry.khronos.org/OpenXR/specs/1.0/man/html/XR_KHR_android_create_instance.html). HTC's [current Unreal setup guide](https://developer.vive.com/resources/openxr/unreal/unreal-tutorials/viveppenxrplugininstallation/) includes Android Vulkan setup; that supports pursuing a native Vulkan probe, but does not certify this engine's features on a particular firmware.

## Do we need a tiled renderer or classic render passes?

The mobile GPU handles tiling; applications control attachment lifetime, data dependencies, and work. Dynamic rendering still describes a rendering scope with attachment load/store operations. The base Mittens path already draws its ordered phases inside one such scope, discards depth/stencil afterward, and can resolve transient MSAA color within the scope. Those are useful starting properties.

Classic render passes are a compatibility option if `VK_KHR_dynamic_rendering` is absent. They also permit input-attachment subpasses for framebuffer-local reads. With suitable dependencies, the driver may merge subpasses and avoid external-memory round trips; this is not guaranteed. See Khronos' [dynamic rendering sample](https://docs.vulkan.org/samples/latest/samples/extensions/dynamic_rendering/README.html) and [subpasses sample](https://docs.vulkan.org/samples/latest/samples/performance/subpasses/README.html).

`VK_KHR_dynamic_rendering_local_read` offers framebuffer-local input reads with dynamic rendering, but must be probed separately. Neither it nor ordinary input attachments make arbitrary neighboring-pixel sampling tile-local. Bloom blur and displaced scene-color refraction still require a suitable sampled-image path. See the [rasterization-order attachment sample](https://docs.vulkan.org/samples/latest/samples/extensions/rasterization_order_attachment_access/README.html) and [tile rendering guide](https://docs.vulkan.org/guide/latest/tile_based_rendering_best_practices.html).

Changing `begin_rendering` to `begin_render_pass` alone is insufficient: pipeline compatibility, framebuffers, resolve attachments, dependencies, custom materials, and optional fullscreen passes must also follow the selected implementation. A forward rendering path with one main scope is the sensible initial mobile target; a deferred G-buffer or tiled light-culling system is not required by Android.

## Highest-value mobile changes

These are recommendations inferred from the source, not measured Focus 3 performance results.

1. **Eliminate the final eye-image copies.** Render/resolve directly into acquired XR image views using the same Vulkan device. For scale, assuming two 2448 x 2448 RGBA8 images at 90 Hz, a single full stereo copy represents about 8.6 GB/s of logical reads plus writes. Actual runtime extents, compression, and memory traffic differ. The resolution/refresh reference is HTC's [Focus 3 specification](https://business.vive.com/jp/product/vive-focus3/specs/).
2. **Keep the initial main pass simple.** The current XR path passes no scene-color snapshot or rough-transmission pyramid to `build_draw_batches_command_buffer`; do not assume parity with the richer window transmission path. Bloom does add XR targets and scopes, and mirrors add scene captures. Start without these features and add explicit mobile fallbacks/quality settings.
3. **Add multiview where it helps.** Vulkano 0.35.2 already exposes `view_mask` in both `RenderingInfo` and `PipelineRenderingCreateInfo`; wrapper support is not the unresolved issue in the earlier [multiview note](../../docs/analysis/VK_KHR_multiview.md). Device features, shaders, attachments, camera data, and transparency remain implementation work. Multiview reduces repeated submission/setup; it does not eliminate rasterization for the second eye.
4. **Tune attachment allocation.** MSAA color is marked transient today, but the context uses `StandardMemoryAllocator::new_default`, whose Vulkano 0.35.2 implementation excludes lazily allocated memory types. Add an appropriate transient allocation path when the device exposes useful lazy memory. Make depth/stencil transient only when its contents are not needed beyond the scope. See Khronos' [attachment guidance](https://docs.vulkan.org/samples/latest/samples/performance/render_passes/README.html).
5. **Choose MSAA and image size deliberately.** The engine defaults to 4x MSAA. Start the probe at 1x, then compare supported 2x/4x configurations and render scales; 2x needs a new engine setting. Avoid storing/reloading multisampled data merely to reopen a scope. Keep clipping and text readability in the quality evaluation.
6. **Budget scene/shader work.** Toon/anime fragment shaders iterate over the scene's active lights, and multi-layer transparency has per-eye sorting and individual draws. Set practical light, overdraw, geometry, texture, and deformation budgets and profile representative content before adding a more elaborate lighting renderer.

The broad statement in the main README/render-phase notes about a single rendering scope describes the base path. Optional post-processing and window scene-color sampling introduce additional scopes/copies; those docs must not be read as a complete bandwidth description.

Do not assume foveation can cover an otherwise over-budget renderer. HTC's current [Unreal foveation support table](https://developer.vive.com/resources/openxr/unreal/unreal-tutorials/rendering/foveated-rendering/) marks standalone Focus 3 unsupported for that plugin path. This does not establish every possible native API's capability; probe extensions on the actual headset before planning around foveation.

## Shared renderer structure

- Device/resources: a runtime-compatible Vulkan instance/device/queue plus allocators and shared assets; no required window surface.
- Targets: extent, color/depth formats, samples, image views, frame slots, and ownership, supplied independently for window, offscreen, or XR output.
- Pass recording: dynamic rendering or classic render passes selected from capabilities, sharing draw streams and material definitions with compatible pipeline variants.
- Platform host: Android lifecycle and OpenXR frame/session ownership; invoke portable engine work at predicted display time and stop/resume it coherently.

## Validation boundary

Inspected engine source and locked dependency implementations (`vulkano 0.35.2`, `vulkano-util 0.35.0`, `openxr 0.19.0`, `winit 0.30.12`), and rechecked ALVR's GL session and Android initialization. The Android Rust target is installed in this environment. No cross-compilation, APK creation, installation, Vulkan capability probe, or thermal/frame-time measurement was performed. The next decisive action is the [Focus 3 probe](focus-3-probe.md).
