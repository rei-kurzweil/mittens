# Android OpenXR/Vulkan bring-up

Status: OpenXR initialization, a Vulkan enable2 session, stereo clear, and
triangle rendering passed on Focus 3 on 2026-10-29, with user confirmation.
The driver lacks dynamic rendering; the probe used classic render passes.
See [device results](focus-3-vulkan-results.md). Graphics lifecycle validation
remains separate from the previous [NativeActivity tests](focus-3-lifecycle.md).

## Build and run

All modes use `org.mittens.engine.dev` / **Mittens Host**. Installing another
mode replaces that app's library; no ALVR app is changed.

| Mode | Build helper arguments | Behavior |
| --- | --- | --- |
| Lifecycle | `--no-default-features` | Previously verified blank native panel; no XR worker. |
| OpenXR diagnostic | `--no-default-features --features xr-probe` | Loader/instance/HMD, extensions and Vulkan API requirements; no graphics session. |
| Vulkan probe (default) | None | Adds Vulkan device/session, capability report, stereo clear and triangle. |

Build and test the diagnostic first, from the repository root:

```sh
android/packaging/build-apk.sh --no-default-features --features xr-probe
adb install --no-incremental -r target/debug/apk/mitten-droid.apk
adb shell am start -W -n org.mittens.engine.dev/android.app.NativeActivity
adb logcat -v threadtime 'mitten-droid:I' 'OpenXR-Loader:V' 'AndroidRuntime:E' 'libc:F' '*:S'
```

Connect/authorize USB debugging as in the [host bootstrap](host-bootstrap.md).
With multiple devices, set `ANDROID_SERIAL`. Expect loader initialization,
runtime/HMD details, and `OpenXR instance/HMD capability stage SUCCESS`.
The diagnostic still has a blank Android panel. Failures include the API name
and exact result; they are not reported as a successful graphics test.

Then build the default Vulkan probe and install/launch with the same commands:

```sh
android/packaging/build-apk.sh
```

Wear the headset for graphics testing. Once visible with valid tracking, expect
a dim reddish left-eye background and dim blue right-eye background. After
90 rendered clear-only frames, an RGB triangle appears in each eye. It is fixed
in the eye images, not a world-anchored engine scene. The probe uses 1x samples,
at most 1024x1024 per eye, no depth, and no multiview.

## Capture results

Each activity instance starts one worker on its first Resume. Close/relaunch
to retry a completed or failed probe. The report is flushed to app-private
storage and logcat:

```sh
adb shell run-as org.mittens.engine.dev cat files/xr-vulkan-probe.txt
adb shell getprop ro.product.model
adb shell getprop ro.build.display.id
adb shell getprop ro.build.version.incremental
adb shell getprop ro.build.version.sdk
```

Save the report/logs under `android/docs/validation/` with the date, firmware,
and user observations. Check runtime/binding requirements, selected GPU and
capabilities, `xrCreateSession(Vulkan) SUCCESS`, session states, and
`rendered_frame=0 mode=stereo_clear` / `rendered_frame=90 mode=stereo_triangle`.
Confirm both eyes visually, then test background/resume, session stop/start,
headset removal, and clean destruction. API success alone is not presentation.

## Implementation boundary

The probe prefers advertised `XR_KHR_vulkan_enable2`, creating the instance
and device through its runtime calls. Otherwise it queries/enables the legacy
binding's required extension lists. A failed path is logged rather than
silently retried with another binding. Android loader initialization is shared
with the engine through `OpenXRSystem::load_entry`; OpenXR types remain at 0.19.

Dynamic rendering is used when its feature and required extensions are usable;
otherwise this small Ash renderer uses classic render passes. This does not add
a classic path to Mittens' full Vulkano scene renderer. Compute/deformation
limits and optional capabilities are reported without requiring those features.

The worker owns XR and the Vulkan queue, honors session state/`should_render`
and valid poses, and completes its GPU fence before releasing each acquired
image. Framebuffers/views are destroyed before XR images; all XR handles
precede Vulkan device destruction. On Activity destruction, session exit is
requested and the worker is joined before Android context release. Driver
calls such as `xrWaitFrame` and GPU waits can still block, so actual graphics
lifecycle testing remains required. Force-stop is an emergency stop, not a
graceful-shutdown test.

The build helper fetches the official Khronos 1.1.36 AAR and verifies SHA-256
for it and the extracted arm64 loader. The library matches ALVR's generic loader
byte for byte. Its Apache 2.0 license and provenance are packaged in APK assets.
See the [Khronos release](https://github.com/KhronosGroup/OpenXR-SDK-Source/releases/tag/release-1.1.36)
and [Vulkan binding specification](https://registry.khronos.org/OpenXR/specs/1.0-khr/html/xrspec.html#XR_KHR_vulkan_enable2).

Native debug sections are stripped by default to avoid a roughly 300 MB debug
APK after linking the engine hook; log/panic diagnostics remain. Override
`CARGO_PROFILE_DEV_STRIP` when native debug information is needed. Development
signing and debuggability remain enabled, including ADB `run-as`.

## Local validation

ARM64 Android checks and diagnostic/default APK builds passed. The engine's 17
OpenXR tests passed with desktop support disabled. On GeForce GTX 1080, pixel
readback passed for both eye colors, triangle presence/absence, and image reuse
with dynamic rendering and classic render passes. The headless tool imports
the actual renderer source and uses application-owned images, without OpenXR:

```sh
CARGO_TARGET_DIR=target cargo run --manifest-path android/validation/renderer-smoke/Cargo.toml --locked
```

See the saved [pixel-readback results](validation/probe-renderer-headless-2026-10-29.log).

Focus 3 instance creation, Vulkan session support, and stereo triangle
presentation are now verified. Graphics lifecycle, full engine scenes,
controller input, passthrough, audio, and sustained performance remain to test.
