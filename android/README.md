# Mittens on Android

Status: NativeActivity lifecycle and the OpenXR/Vulkan clear/triangle probe were verified on Focus 3 on 2026-10-29. The user confirmed the stereo output. The installed Adreno 650 driver exposes Vulkan 1.1.128 without dynamic rendering, so the successful probe uses classic render passes. See [Vulkan results](docs/focus-3-vulkan-results.md), [lifecycle results](docs/focus-3-lifecycle.md), and [build/run instructions](docs/xr-vulkan-bringup.md). Full engine scenes and graphics lifecycle validation remain outstanding.

The target is **Mittens running locally on a Vive Focus 3**: the engine runs its ECS, MMS scripts, simulation, audio, and Vulkan renderer on the headset.

The new custom opaque renderer and default scene APK are implemented and have
desktop GPU checks; their Focus 3 scene validation remains pending. See the
[implementation and limits](docs/opaque-renderer.md), the
[desktop OpenXR comparison demo](../examples/xr-renderer-compare/README.md), and
the [crate/API boundary audit](docs/renderer-boundary.md):
the Android application can own its renderer while consuming the engine's
existing public scene preparation and upload contracts.

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
  src/lib.rs                 # Android-only android_main entry point
  src/activity.rs            # context checks, lifecycle/input polling, logcat
  src/xr_host.rs             # Android OpenXR worker and persistent report
  src/xr_session.rs          # shared session and stereo frame driver
  src/graphics_device.rs     # runtime-selected GPU creation/query
  src/triangle_probe.rs      # retained diagnostic renderer
  src/scene_runtime.rs       # embedded MMS Universe + app renderer
  src/renderer/             # custom opaque classic-pass backend
  shaders/                   # triangle and surface vertex shaders
  assets/                    # embedded scene and custom albedo fragment
  packaging/build-apk.sh     # debug APK build helper
  README.md
  docs/
    host-bootstrap.md
    xr-vulkan-bringup.md
    base-library-build.md
    focus-3-port-audit.md
    focus-3-probe.md
    focus-3-lifecycle.md
    validation/              # captured host lifecycle logs
```

Use the [host bootstrap](docs/host-bootstrap.md) for the lifecycle-only build and the [graphics bring-up](docs/xr-vulkan-bringup.md) for the diagnostic and Vulkan probe. Cargo APK metadata lives in `android/Cargo.toml`, and resources/build helpers belong under `android/packaging/`. Next, validate graphics lifecycle and add a classic render-pass path to the engine for this driver. Do not copy ALVR's unrelated device requirements or package identity into the Mittens manifest.

## Rendering direction

Share `VisualWorld`, mesh/material preparation, draw ordering, and shaders where their capability requirements allow. The desktop [XR-only example](../docs/desktop/xr-only.md) now provides renderer initialization without a window, surface, or desktop swapchain. The default `desktop` feature includes Winit; disabling it builds the library and XR-only example without Winit, using engine-owned input types. See [library build commands](docs/base-library-build.md). Add an Android XR frame driver and a mobile quality policy.

The GPU and driver perform hardware tiling. **Vulkan dynamic rendering can use tile-based hardware efficiently.** The measured Focus 3 driver lacks dynamic rendering, so the new backend uses classic passes. It clears/stores XR color directly and clears/discards transient depth, preferring lazy memory. These are bandwidth-oriented policies, not a measured speedup. See the [implementation](docs/opaque-renderer.md) and [device evidence](docs/focus-3-vulkan-results.md).

## Milestones

1. **Retain the desktop XR baseline.** The window-free `openxr-only` example has presented stereo through SteamVR. Explicit session stop/restart remains to test independently of Focus 3 capability validation. Android APK/activity lifecycle has now been verified separately.
2. **Probe the headset.** Package a small Android/OpenXR Vulkan application and record the [capability report](docs/focus-3-probe.md). Clear both eyes, then draw a triangle. Test passthrough composition separately if required.
3. **Choose the renderer compatibility path.** Confirm dynamic rendering, supported formats, compute limits, and multiview on the actual firmware. Add a classic render-pass path if dynamic rendering is unavailable.
4. **Run an embedded Mittens scene on Android.** Adapt the window-free bootstrap to Android loader/lifecycle and the selected Vulkan binding. Drive update/render work from OpenXR frame timing and connect controller input.
5. **Reduce mobile rendering costs.** Validate the existing direct XR output path (added 2026-10-20), add multiview where appropriate, and tune attachment lifetime, resolution, materials, and MSAA. Start with bloom, mirrors, and scene-color transmission disabled.
6. **Complete app integration.** Package assets and script imports, test audio output then microphone input, add optional HTC tracking/passthrough, and validate pause/resume and shutdown.
7. **Measure sustained behavior.** Record CPU/GPU time, missed XR frames, image memory, and thermal behavior in a representative scene.

The existing [Android epic](../docs/task/epic/android_support/README.md) retains the earlier ALVR integration investigation.
