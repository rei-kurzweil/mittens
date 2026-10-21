# Mittens on standalone Android OpenXR headsets

Status: initial investigation 2026-09-28; renderer/platform audit updated 2026-10-19. No Android build or Focus 3 Vulkan probe has been run for Mittens.

The Android port now has a home in [`android/`](../../../../android/README.md), with a [current source audit](../../../../android/docs/focus-3-port-audit.md) and [Focus 3 capability-probe specification](../../../../android/docs/focus-3-probe.md). This epic preserves the earlier integration investigation. Dynamic rendering can use tile-based GPUs; whether a classic render-pass implementation is needed depends first on the headset's driver capabilities, then measured rendering costs.

## Desired end state

`mittens_android` should host **Mittens Engine itself** on the headset. The Android shell owns activity lifecycle, OpenXR initialization, device integration, and headset packaging; Mittens owns its ECS, scripts, simulation, audio graph, and Vulkan scene rendering. The first visual target is a Mittens scene over Focus 3 passthrough, with HMD/controller poses and HTC eye samples available locally. The ALVR streamer and video path are unnecessary.

## What the two codebases currently do

- The verified Focus 3 ALVR client (`/home/rei/_/ALVR/ALVR`) uses **OpenGL ES**, EGL, `XR_KHR_opengl_es_enable`, and an OpenXR `Session<OpenGlEs>` (`alvr/client_openxr/src/lib.rs`, `graphics.rs`). Its waiting lobby combines an HTC passthrough layer with an alpha blended projection layer. It also probes `XR_HTC_eye_tracker` locally without a stream. This is useful for lifecycle, passthrough, tracker ABI, and Android manifest knowledge, but supplies **no Vulkan context** to reuse.
- Mittens already has a desktop **Vulkan OpenXR** path. `Universe::init_renderer_for_window` queries OpenXR's Vulkan extension requirements, creates a Vulkano renderer, and passes its raw instance/physical device/device/queue handles back to `OpenXRSystem` (`src/engine/universe.rs:424`, `src/engine/graphics/vulkano_renderer.rs:6608`). `OpenXRSystem` creates `Session<Vulkan>` and its own XR swapchain (`src/engine/ecs/system/openxr_system.rs:2300`, `src/engine/graphics/xr_swapchain.rs`). As of 2026-10-20, the renderer writes/resolves/composites directly into runtime-owned XR image layers, with a completion-only fence submission (`src/engine/graphics/xr_renderer.rs`).
- The engine now also has `Universe::init_renderer_for_xr` and a [desktop XR-only example](../../../desktop/xr-only.md). This path creates no Winit event loop, window, Vulkan surface, or desktop swapchain. It selects the runtime-required GPU before device creation and uses the actual XR format for scene pipelines. Winit remains a compile dependency through desktop/input APIs; Android packaging and runtime validation remain outstanding.
- Mittens currently pins `openxr 0.19`, `vulkano 0.35`, and `cpal 0.15` (`Cargo.toml`); the ALVR reference uses `openxr 0.21`. Directly sharing their OpenXR types is therefore not a simple copy. Prefer porting the HTC ABI/extension calls into Mittens' selected OpenXR version or deliberately aligning versions after a compatibility check.

## Vulkan/OpenXR ownership proposal

1. Android shell initializes the OpenXR loader and runtime, enumerates headset extensions, obtains the HMD system, and confirms `XR_KHR_vulkan_enable` or `XR_KHR_vulkan_enable2`. Keep the working ALVR Android activity/loader setup as a reference.
2. Create **one** Vulkan instance, physical device, logical device, and graphics queue that satisfy both OpenXR's requirements and Mittens/Vulkano's requirements. OpenXR's requested physical device and graphics API limits must be respected. Use these same handles in `XrGraphicsBindingVulkan*KHR` for session creation. Which of the legacy and enable2 paths works on Focus 3 is a device test, not yet known.
3. Reuse Mittens' window-free `Universe::init_renderer_for_xr` bootstrap and shared ECS/visual world/shaders. Adapt Android initialization and the selected Vulkan binding; validate this extraction first with the desktop XR-only example.
4. Android OpenXR session owns frame timing and XR swapchain acquire/wait/release; Mittens renders the predicted stereo views. Submit the HTC passthrough composition layer first and an alpha blended Vulkan projection layer above it. Check alpha format, clear alpha, image layout, and layer lifetime on the headset. Feed timestamped view/HMD/controller/hand/eye samples into engine input, with validity and reference space preserved.

Passing Mittens an arbitrary second Vulkan context, or trying to render into ALVR's OpenGL ES swapchain, would not satisfy its existing `Session<Vulkan>` and XR image path. A temporary GL-to-Vulkan bridge is possible in principle but adds synchronization and image sharing complexity before the basic runtime capability is known.

## Focus 3 gate before large renderer work

Build a tiny Android probe using the known ALVR packaging/loader recipe to log: runtime name/version, advertised Vulkan and HTC extensions, OpenXR Vulkan API requirements, required Vulkan instance/device extensions, runtime-selected `VkPhysicalDevice`, supported swapchain formats, and `xrCreateSession` result with a Vulkan binding. Then clear a stereo XR image and composite it over HTC passthrough. This is the first decisive milestone for putting Mittens' renderer on the headset. If the Vulkan extension or session fails, record the exact result before planning a GL fallback.

## Other platform work

| Area | Finding / check |
| --- | --- |
| Android lifecycle and threads | ALVR's `android_main` polls activity events while a rendering thread runs OpenXR. Rust threads and Mittens' script worker should be viable, but surface/activity pause, resume, destroy, and joining workers must be explicit. Keep OpenXR frame ownership and Vulkan queue use coordinated on the render thread; do not let audio callbacks or script workers touch XR handles without synchronization. |
| Audio (`cpal`) | **Likely viable, unverified on Focus 3.** The locked `cpal 0.15.3` includes an Android Oboe/AAudio backend (local crate metadata includes `oboe`, `jni`, and `ndk-context`). Mittens uses `cpal::default_host()` for output and input (`audio_system.rs`, `audio_input_system.rs`). Test APK linkage, JNI/activity context initialization, playback format/buffer size, device changes, and microphone permission on hardware. Audio input can be deferred behind a feature if the first render probe does not use it. |
| Renderer capabilities | Mittens requires dynamic rendering and checks compute limits including eight compute-stage storage buffers. Probe these separately from Vulkan session support. If dynamic rendering is unavailable, add classic render-pass/framebuffer and compatible pipeline creation paths. Share scene preparation; gate deformation and expensive effects/resources by capabilities and mobile quality policy. See the current audit for depth/stencil formats, XR sample compatibility, image copies, multiview, and transient allocation. |
| Window and input | Current startup is driven by `winit` `RedrawRequested`, creates a desktop window, and handles keyboard/mouse. Add an Android XR frame driver that calls the same engine update/render work from OpenXR frame timing. Map OpenXR Focus 3 controller and hand actions to Mittens input components; keep desktop paths intact. |
| Files and scripting | `main.rs`, asset loading, glTF, and audio decoding use filesystem paths. APK assets are not ordinary working-directory files. Define an asset source abstraction (packaged assets or app-private storage) and load an embedded test `.mms` scene first. Disable stdin REPL assumptions on the headset; use logcat for diagnostics. |
| Tracking | Existing `OpenXRSystem` handles controllers and hands on desktop, including Focus 3 interaction profile hooks. Port HTC proprietary eye samples from ALVR into the same time/reference-space model. Eye tracker creation and teardown order needs device validation; ALVR's facial tracker code warns about a crash on destruction. |
| Android packaging | Distinct package ID, arm64 target, OpenXR loader/native libraries, activity metadata, HTC features, microphone permission when used, asset packaging, and MIT notices for copied ALVR code. ALVR pins `android-activity = 0.6.0` due to a context regression it observed; retain that as a starting constraint until tested. |

## Recommended sequence

- [x] Extract window-free Vulkan resources, XR format/pipeline selection, and eye slots; add the desktop `openxr-only` example.
- [x] Validate desktop stereo presentation: SteamVR/OpenXR 2.16.7, 4x MSAA, bounded run exited successfully; user confirmed headset presentation.
- [ ] Explicitly exercise session stop/restart with the desktop XR-only example.
- [ ] Capture Focus 3 Vulkan extension/session capability with a tiny Android probe.
- [ ] Prove Vulkan stereo clear plus HTC passthrough on the headset.
- [ ] Run an embedded Mittens scene, then wire HMD/controller/hand/eye inputs.
- [ ] Probe CPAL output, then microphone input and audio clock behavior.
- [ ] Test pause/resume, headset removal, eye accessory attached, and packaging of assets.

## External references checked

- [OpenXR specification: graphics API binding and session creation](https://registry.khronos.org/OpenXR/specs/1.1-khr/html/xrspec.html)
- [OpenXR `xrGetVulkanGraphicsDevice2KHR` reference](https://registry.khronos.org/OpenXR/specs/1.1/man/html/xrGetVulkanGraphicsDevice2KHR.html)
- [CPAL 0.15 changelog: Android AAudio/Oboe](https://github.com/RustAudio/cpal/blob/master/CHANGELOG.md?plain=1)
