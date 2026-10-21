# Android host implementation slices

Plan recorded 2026-10-21. Target: Mittens running locally on a Vive Focus 3,
hosted by `crates/mitten-droid/` in this repository.

## Starting point

- The portable engine library checks and builds for arm64 Android with
  `default-features = false`; this does not establish Android runtime support.
- Desktop XR-only startup already exists without Winit, a window, or a Vulkan
  surface. Rendering writes/resolves directly into OpenXR swapchain images.
- There is no Mittens Android host or APK yet. Focus 3 Vulkan session creation
  and compatibility with the current renderer remain untested.

Use ALVR's working Android client as the reference for activity/lifecycle,
loader initialization, and packaging. Its client renderer uses OpenGL ES;
Mittens supplies the Vulkan path. Keep the first app independent of streaming,
passthrough, eye tracking, audio, and external assets.

Supporting details: [base-library build](../../../../android/docs/base-library-build.md),
[port audit](../../../../android/docs/focus-3-port-audit.md), and
[headset capability probe](../../../../android/docs/focus-3-probe.md).

## Slice 1: Build the Android host APK

- [ ] Add `crates/mitten-droid/` as an Android native `cdylib` crate, depending
  on `mittens-engine` with default features disabled.
- [ ] Keep Android dependencies/entry points target-specific and preserve the
  ordinary desktop build/test workflow when adding the workspace member.
- [ ] Put the host's Android manifest, resources, and packaging configuration
  alongside the crate. Choose one reproducible packaging command, using the
  local ALVR setup as the starting reference.
- [ ] Add the activity entry point, Android context setup, and logcat logging.
  Give the app its own package identity; preserve notices for copied code.

**Done when:** a documented command produces an arm64 APK with the native
entry point and required native libraries packaged. The host has no Winit
dependency. No rendering is required for this slice.

## Slice 2: Verify startup and activity lifecycle on the Focus 3

- [ ] Document ADB install, launch, log capture, and stop commands with the
  actual package/activity names from slice 1.
- [ ] Log startup and activity events. Establish which thread owns future XR
  work and how shutdown stops and joins any workers.
- [ ] Install and launch on the headset; exercise background/foreground,
  repeated launch, and clean shutdown without creating graphics resources.

**Done when:** the APK starts on the Focus 3 and captured logs demonstrate
that lifecycle handling completes without crashes or hanging workers. Record
device/firmware identity and separate successful build checks from device tests.

## Slice 3: Initialize Android OpenXR through the engine

- [ ] Add the engine startup interface needed by the Android host to supply
  platform context/loader setup while preserving existing desktop startup.
- [ ] Initialize the Android OpenXR loader before enumerating extensions;
  enable Android instance creation with the activity/JVM information.
- [ ] Create an instance and obtain the HMD system. Report runtime identity,
  available Vulkan bindings, view configuration, and blend modes.
- [ ] Keep host and engine OpenXR types compatible. Mittens currently uses
  `openxr 0.19`; the local ALVR reference uses `0.21`.

**Done when:** headset logs identify a working OpenXR instance and HMD system,
with exact API/result diagnostics on failure. Desktop XR-only startup still
works. A Vulkan device/session is the next slice.

## Slice 4: Prove Vulkan stereo output and record capabilities

- [ ] Use the runtime-selected physical device and satisfy its Vulkan API and
  extension requirements. Select a supported Vulkan binding; use its required
  creation procedure, including runtime Vulkan creation calls for `enable2`.
- [ ] Record the [capability report](../../../../android/docs/focus-3-probe.md):
  dynamic rendering, image formats/sample counts, depth/stencil support,
  compute/storage-buffer limits, queues, and optional multiview.
- [ ] Create one Vulkan device shared with the XR session and a supported
  single-sample stereo swapchain. Clear the eyes to distinct colors, honoring
  acquire/wait/release, GPU completion, image layouts, and session/frame state.
- [ ] Draw a triangle with the supported rendering API. Keep this probe small
  enough to work independently of Mittens' full scene pipeline requirements.

**Done when:** the user confirms both eye outputs on the headset, a triangle
renders, and the capability report is saved. Exercise session stop/start and
headset removal as well as activity background/foreground with graphics active.

**Decision:** reuse the existing dynamic-rendering path if supported. If it is
unavailable, define a focused classic render-pass implementation before full
scene bring-up. Handle other failed requirements explicitly; Vulkan session
success alone does not prove that the complete Mittens renderer can run.

## Slice 5: Run an embedded Mittens scene, then connect input

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
performance follow the basic scene. A separate renderer is not assumed in
advance; compatibility changes follow the headset results from slice 4.

Start with slice 1, then complete slice 2 before introducing OpenXR or Vulkan
startup failures into the activity/lifecycle test.
