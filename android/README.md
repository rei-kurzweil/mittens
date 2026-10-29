# Mittens on Android

Status: `mitten-droid` is now a workspace crate rooted in this directory. It is a host scaffold with no activity entry point or APK packaging yet. The portable base-library Android check and build passed on 2026-10-21. No Mittens APK or Focus 3 Vulkan session has been tested.

The target is **Mittens running locally on a Vive Focus 3**: the engine runs its ECS, MMS scripts, simulation, audio, and Vulkan renderer on the headset.

## Android host boundary

Use the local ALVR client's working Android integration as a reference:

- APK packaging and OpenXR loader distribution/discovery.
- `android_main`, Android activity events, thread startup, and shutdown.
- HTC controller, hand/eye tracking, and passthrough integration where needed.

ALVR's current client uses **OpenGL ES + EGL**, including `Session<OpenGlEs>` and GL swapchains. Its Vulkan streaming/server code is not an Android Vulkan scene renderer. A Mittens host must create a Vulkan-backed OpenXR session and swapchains, with one Vulkan device shared by OpenXR and Mittens. Streaming, network transport, and video decoding are outside this host's purpose.

Keep Android entry points, packaging, lifecycle adapters, and platform diagnostics here. Keep portable renderer, ECS, scripting, asset, and audio changes in the engine's existing source directories.

Current layout:

```text
android/
  Cargo.toml                 # mitten-droid cdylib; consumes mittens-engine
  src/lib.rs                 # host scaffold; activity entry point comes next
  README.md
  docs/
    host-bootstrap.md
    base-library-build.md
    focus-3-port-audit.md
    focus-3-probe.md
```

Start with the [host bootstrap](docs/host-bootstrap.md): add the activity event loop and APK packaging, then verify lifecycle on the headset. Future resources/build helpers belong under `android/packaging/`, with Cargo APK metadata in `android/Cargo.toml`. Do not copy ALVR's unrelated device requirements or package identity into the Mittens manifest.

## Rendering direction

Share `VisualWorld`, mesh/material preparation, draw ordering, and shaders where their capability requirements allow. The desktop [XR-only example](../docs/desktop/xr-only.md) now provides renderer initialization without a window, surface, or desktop swapchain. The default `desktop` feature includes Winit; disabling it builds the library and XR-only example without Winit, using engine-owned input types. See [library build commands](docs/base-library-build.md). Add an Android XR frame driver and a mobile quality policy.

The GPU and driver perform hardware tiling. **Vulkan dynamic rendering can use tile-based hardware efficiently.** Classic render passes are needed as a compatibility path if the headset lacks dynamic rendering, or for a measured benefit from subpasses. We do not yet have device evidence that decides this. See the [renderer and platform audit](docs/focus-3-port-audit.md).

## Milestones

1. **Verify window-free startup on desktop first.** Run `cargo run --release --example openxr-only`. Check stereo rendering and session stop/restart using the existing desktop runtime before introducing Android packaging. This is independent of Focus 3 capability validation.
2. **Probe the headset.** Package a small Android/OpenXR Vulkan application and record the [capability report](docs/focus-3-probe.md). Clear both eyes, then draw a triangle. Test passthrough composition separately if required.
3. **Choose the renderer compatibility path.** Confirm dynamic rendering, supported formats, compute limits, and multiview on the actual firmware. Add a classic render-pass path if dynamic rendering is unavailable.
4. **Run an embedded Mittens scene on Android.** Adapt the window-free bootstrap to Android loader/lifecycle and the selected Vulkan binding. Drive update/render work from OpenXR frame timing and connect controller input.
5. **Reduce mobile rendering costs.** Validate the existing direct XR output path (added 2026-10-20), add multiview where appropriate, and tune attachment lifetime, resolution, materials, and MSAA. Start with bloom, mirrors, and scene-color transmission disabled.
6. **Complete app integration.** Package assets and script imports, test audio output then microphone input, add optional HTC tracking/passthrough, and validate pause/resume and shutdown.
7. **Measure sustained behavior.** Record CPU/GPU time, missed XR frames, image memory, and thermal behavior in a representative scene.

The existing [Android epic](../docs/task/epic/android_support/README.md) retains the earlier ALVR integration investigation.
