# Android host implementation slices

Plan recorded 2026-10-21. Target: Mittens running locally on a Vive Focus 3,
hosted by the `mitten-droid` crate at `android/` in this repository. Scaffold
added 2026-10-29; APK/activity lifecycle also verified on Focus 3 that day.

## Starting point

- The portable engine library checks and builds for arm64 Android with
  `default-features = false`; this does not establish Android runtime support.
- Desktop XR-only startup already exists without Winit, a window, or a Vulkan
  surface. Rendering writes/resolves directly into OpenXR swapchain images.
- The Android host crate at `android/Cargo.toml` builds an APK and runs its
  NativeActivity lifecycle on Focus 3. It has an Android-only engine dependency
  but does not start the engine. OpenXR/Vulkan compatibility remains untested.

Use ALVR's working Android client as the reference for activity/lifecycle,
loader initialization, and packaging. Its client renderer uses OpenGL ES;
Mittens supplies the Vulkan path. Keep the first app independent of streaming,
passthrough, eye tracking, audio, and external assets.

Supporting details: [base-library build](../../../../android/docs/base-library-build.md),
[port audit](../../../../android/docs/focus-3-port-audit.md), and
[headset capability probe](../../../../android/docs/focus-3-probe.md).

## Slice 1: Build the Android host APK

- [x] Add `android/` as the `mitten-droid` native `cdylib` crate, depending
  on `mittens-engine` with default features disabled.
- [x] Add the workspace member with an Android-only engine dependency, keeping
  desktop support disabled for that dependency. Keep future Android activity
  dependencies and entry points target-specific as well.
- [x] Put manifest metadata in `android/Cargo.toml` and resources/build helpers
  under `android/packaging/`. Choose one reproducible packaging command, using the
  local ALVR setup as the starting reference.
- [x] Add the activity entry point, Android context setup, and logcat logging.
  Give the app its own package identity; preserve notices for copied code.

**Done when:** a documented command produces an arm64 APK with the native
entry point and required native libraries packaged. The host has no Winit
dependency. No rendering is required for this slice.

## Slice 2: Verify startup and activity lifecycle on the Focus 3

- [x] Document ADB install, launch, log capture, and stop commands with the
  actual package/activity names from slice 1.
- [x] Log startup and activity events on the application thread. No host workers
  are created in the verified lifecycle-only mode. The new graphics probe has
  a worker with XR/queue ownership and stop/join coordination; its device
  lifecycle tests remain outstanding.
- [x] Install and launch on the headset; exercise background/foreground,
  repeated launch, and clean shutdown without creating graphics resources.

**Done when:** the APK starts on the Focus 3 and captured logs demonstrate
that lifecycle handling completes without crashes or hanging workers. Record
device/firmware identity and separate successful build checks from device tests.

**Verified 2026-10-29:** [build/device results and captured logs](../../../../android/docs/focus-3-lifecycle.md).

## Slice 3: Initialize Android OpenXR through the engine

- [x] Add the engine startup interface needed by the Android host to supply
  platform context/loader setup while preserving existing desktop startup.
- [x] Initialize the Android OpenXR loader before enumerating extensions;
  enable Android instance creation with the activity/JVM information.
- [x] Create an instance and obtain the HMD system. Report runtime identity,
  available Vulkan bindings, view configuration, and blend modes.
- [x] Keep host and engine OpenXR types compatible. Mittens currently uses
  `openxr 0.19`; the local ALVR reference uses `0.21`.

**Done when:** headset logs identify a working OpenXR instance and HMD system,
with exact API/result diagnostics on failure. Desktop XR-only startup still
works. A Vulkan device/session is the next slice.

Implementation/build status: `OpenXRSystem::load_entry` supplies the shared
Android loader hook; engine and host enable Android instance creation. The
instance/HMD diagnostic passed on Focus 3 with VIVE WAVE and both Vulkan
bindings advertised. See [device results](../../../../android/docs/focus-3-vulkan-results.md)
and the [graphics bring-up](../../../../android/docs/xr-vulkan-bringup.md).

## Slice 4: Prove Vulkan stereo output and record capabilities

The basic device gate passed: Vulkan enable2, capability capture, and classic
render-pass stereo clear/triangle succeeded on Focus 3. The user confirmed
the eye outputs. Desktop pixel readback also passed for both renderer paths.
Graphics lifecycle remains a separate, unfinished acceptance check.

- [x] Use the runtime-selected physical device and satisfy its Vulkan API and
  extension requirements. Select a supported Vulkan binding; use its required
  creation procedure, including runtime Vulkan creation calls for `enable2`.
- [x] Record the [capability report](../../../../android/docs/focus-3-probe.md):
  dynamic rendering, image formats/sample counts, depth/stencil support,
  compute/storage-buffer limits, queues, and optional multiview.
- [x] Create one Vulkan device shared with the XR session and a supported
  single-sample stereo swapchain. Clear the eyes to distinct colors, honoring
  acquire/wait/release, GPU completion, image layouts, and session/frame state.
- [x] Draw a triangle with the supported rendering API. Keep this probe small
  enough to work independently of Mittens' full scene pipeline requirements.
- [ ] Validate graphics-active background/resume, session stop/start, headset
  removal, and clean destruction/relaunch.

**Done when:** the user confirms both eye outputs on the headset, a triangle
renders, and the capability report is saved. Exercise session stop/start and
headset removal as well as activity background/foreground with graphics active.

**Decision:** reuse the existing dynamic-rendering path if supported. If it is
unavailable, define a focused classic render-pass implementation before full
scene bring-up. Handle other failed requirements explicitly; Vulkan session
success alone does not prove that the complete Mittens renderer can run.

**Measured decision:** the installed Adreno 650 driver exposes Vulkan 1.1.128
without `VK_KHR_dynamic_rendering`. Add classic render-pass/framebuffer and
compatible pipeline recording to the full engine; the probe's Ash fallback
does not supply that engine backend. [Evidence](../../../../android/docs/focus-3-vulkan-results.md).

## Slice 5: Run an embedded Mittens scene, then connect input

The [renderer boundary audit](../../../../android/docs/renderer-boundary.md)
recommends implementing the first mobile renderer under `android/`, consuming
the engine's public upload/visual-stream API. The external consumer test passes
without desktop features. Full engine XR input still needs an external frame
source; the current `Universe::render`/XR convenience path is Vulkano-specific.

The [custom opaque implementation](../../../../android/docs/opaque-renderer.md)
now loads the embedded MMS scene, implements mesh/RGBA upload, retains generic
custom material inputs and batched instances, and renders through classic eye
passes with transient depth. Its Android APK builds and desktop GPU pixel checks
pass. The boxes below remain open for the Focus 3 presentation/lifecycle/input
gate. The new [desktop XR comparison](../../../../examples/xr-renderer-compare/README.md)
selects the same backend or the existing Vulkano renderer for a matched scene.

- [ ] Connect the Android bootstrap to the existing window-free engine/XR
  rendering path, sharing the Vulkan device and runtime-owned output images.
- [ ] Load a tiny embedded MMS scene with simple geometry and no external
  asset paths. Start with MSAA and costly effects disabled; gate unsupported
  subsystems based on slice 4 rather than requiring them for a static scene.
- [ ] Drive update/render work from OpenXR frame timing and predicted poses.
  Start with zero HID providers; verify head tracking and scene stability.
- [ ] Exercise the existing Focus 3 controller action/profile integration.
  Then add an Android HID adapter for keyboard/touchpad events using engine
  `HidEvent` types; verify delivery from actual connected hardware.

**Done when:** the embedded scene renders locally with head/controller input,
survives lifecycle transitions, and reports useful frame timing. Track Android
keyboard/touchpad validation separately if hardware is unavailable.

## Work after the first scene

Packaged asset access, audio output then microphone support, passthrough,
optional tracking extensions, mobile rendering quality, and sustained thermal
performance follow the basic scene. The chosen first backend is custom-material
opaque/albedo only; expand capabilities deliberately after the first scene.

Slices 1–3 and the basic slice 4 stereo/capability gate are verified. Graphics
lifecycle and the new embedded scene remain to verify on Focus 3. Keep exact
diagnostics and the lifecycle-only build available.
