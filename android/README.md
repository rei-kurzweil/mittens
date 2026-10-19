# Mittens on Android

Status: port planning and source audit, 2026-10-19. This directory does not yet contain a buildable Android app. No Mittens APK or Focus 3 Vulkan session has been tested.

The target is **Mittens running locally on a Vive Focus 3**: the engine runs its ECS, MMS scripts, simulation, audio, and Vulkan renderer on the headset.

## Android host boundary

Use the local ALVR client's working Android integration as a reference:

- APK packaging and OpenXR loader distribution/discovery.
- `android_main`, Android activity events, thread startup, and shutdown.
- HTC controller, hand/eye tracking, and passthrough integration where needed.

ALVR's current client uses **OpenGL ES + EGL**, including `Session<OpenGlEs>` and GL swapchains. Its Vulkan streaming/server code is not an Android Vulkan scene renderer. A Mittens host must create a Vulkan-backed OpenXR session and swapchains, with one Vulkan device shared by OpenXR and Mittens. Streaming, network transport, and video decoding are outside this host's purpose.

Keep Android entry points, packaging, lifecycle adapters, and platform diagnostics here. Keep portable renderer, ECS, scripting, asset, and audio changes in the engine's existing source directories.

Suggested layout as implementation lands:

```text
android/
  README.md
  docs/
    focus-3-port-audit.md
    focus-3-probe.md
  host/                      # future Rust cdylib: android_main + engine integration
  packaging/                 # future manifest/build configuration + app resources
```

Only the documentation exists today. Start with the ALVR packaging recipe; choose and commit a single packaging workflow when the probe is implemented. Do not copy ALVR's unrelated device requirements or package identity into the Mittens manifest.

## Rendering direction

Share `VisualWorld`, mesh/material preparation, draw ordering, and shaders where their capability requirements allow. Extract renderer device/target initialization from the current desktop window setup. Add an Android XR frame driver and a mobile quality policy.

The GPU and driver perform hardware tiling. **Vulkan dynamic rendering can use tile-based hardware efficiently.** Classic render passes are needed as a compatibility path if the headset lacks dynamic rendering, or for a measured benefit from subpasses. We do not yet have device evidence that decides this. See the [renderer and platform audit](docs/focus-3-port-audit.md).

## Milestones

1. **Probe the headset.** Package a small Android/OpenXR Vulkan application and record the [capability report](docs/focus-3-probe.md). Clear both eyes, then draw a triangle. Test passthrough composition separately if required.
2. **Choose the renderer compatibility path.** Confirm dynamic rendering, supported formats, compute limits, and multiview on the actual firmware. Add a classic render-pass path if dynamic rendering is unavailable.
3. **Run an embedded Mittens scene.** Initialize the engine's Vulkan device and XR targets without a desktop surface or swapchain. Drive update/render work from OpenXR frame timing and connect controller input.
4. **Reduce mobile rendering costs.** Render/resolve into runtime-owned XR images, add multiview where appropriate, and tune attachment lifetime, resolution, materials, and MSAA. Start with bloom, mirrors, and scene-color transmission disabled.
5. **Complete app integration.** Package assets and script imports, test audio output then microphone input, add optional HTC tracking/passthrough, and validate pause/resume and shutdown.
6. **Measure sustained behavior.** Record CPU/GPU time, missed XR frames, image memory, and thermal behavior in a representative scene.

The existing [Android epic](../docs/task/epic/android_support/README.md) retains the earlier ALVR integration investigation.
