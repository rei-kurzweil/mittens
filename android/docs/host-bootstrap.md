# Android host bootstrap

`mitten-droid` is a workspace crate rooted at `android/Cargo.toml`. It consumes
`mittens-engine`; `crates/` contains the engine's standalone dependencies.
Android application code, packaging, and platform diagnostics belong here.

The default APK now includes the [OpenXR/Vulkan probe](xr-vulkan-bringup.md),
whose Focus 3 validation is pending. This page describes the previously
verified lifecycle-only build; use `--no-default-features` to select it.

## Current host

- Native `cdylib` crate with an Android-only dependency on `mittens-engine`,
  using `default-features = false` to exclude desktop windowing.
- Workspace membership and a shared root lockfile/target directory.
- NativeActivity entry point, an activity event loop, logcat diagnostics, and
  APK metadata/build helper. Package: `org.mittens.engine.dev`; launcher label:
  **Mittens Host**. It installs alongside ALVR.
- No engine startup, OpenXR loader/session, Vulkan rendering, or audio yet.

Expect a blank screen when launched. Logcat is the acceptance signal for this
stage; there is no scene or XR session to display.

`android-activity = 0.6.0` matches the working local ALVR client's context
constraint. Its NativeActivity entry point initializes `ndk-context` with the
JVM and Activity and attaches the application thread before calling our
`android_main`. It releases that context afterward; the host must not repeat
that initialization or teardown. Our host checks that context, drains input,
waits for events while idle/paused, and returns on `Destroy`. No host workers
are started in this stage.

From the repository root, check the crate boundary on the development machine:

```sh
cargo check -p mitten-droid --locked
```

That command does not compile the Android activity implementation. To check
the host and engine for arm64 Android, configure the NDK and CMake as described
in [portable library builds](base-library-build.md), then run:

```sh
cargo ndk -t arm64-v8a -p 28 check -p mitten-droid --lib --locked
```

An Android check does not create an APK or validate headset behavior.
See [portable library builds](base-library-build.md) for prerequisites and
previous engine validation.

## Scaffold validation on 2026-10-29

The desktop scaffold check and the arm64 Android host check passed with the
shared lockfile. The Android check used NDK 28.1.13356709/API 28 and CMake
3.30.3, and included the engine dependency. The resolved Android dependency
tree contains no Winit or `vulkano-util`; the default workspace member remains
`mittens-engine`. Formatting and documentation links were also checked.
No APK build or headset test was performed.

The section above records the initial scaffold validation. APK/device results
for the implemented activity host are tracked separately below.

## Build the APK

Prerequisites: `aarch64-linux-android` Rust target, Android SDK/platform tools,
NDK, CMake 3.x, Java/keytool, and `cargo-apk`. This host uses the local ALVR
packaging tool: `cargo-apk 0.10.0` from the `zarik5/cargo-apk` fork, installed
at revision `0fd3126dad5aa1c5f0f26cdae3410f2e5af62c60`. The debug build uses
cargo-apk's development signing key; release signing is not set up.

From the repository root:

```sh
export ANDROID_HOME="$HOME/Android/Sdk"
android/packaging/build-apk.sh --no-default-features
```

The helper defaults to NDK `28.1.13356709` and SDK CMake `3.30.3`. Set
`ANDROID_NDK_HOME` and `CMAKE` to override their locations. It supplies both
NDK environment variables needed by Cargo and shaderc's CMake build.
The APK's minimum API is 28 and target API is 32, following the ALVR baseline;
this is a development sideload, not a store submission.

Output: `target/debug/apk/mitten-droid.apk`. Cargo APK metadata in
`android/Cargo.toml` generates the manifest and packages `libmitten_droid.so`
and its native dependencies. The common packaging now also includes the
Khronos OpenXR loader and discovery permissions, but this lifecycle-only mode
does not initialize it. No GL requirement, tracking/audio permissions, or
streaming dependencies are added.

## Install and exercise lifecycle

Connect the Focus 3 by USB, enable USB debugging, and authorize the computer
on the headset. `adb devices -l` should report `device`, not `unauthorized` or
`offline`. With multiple devices connected, set `ANDROID_SERIAL` to the
Focus 3 serial.

```sh
adb devices -l
adb install --no-incremental -r target/debug/apk/mitten-droid.apk
adb shell am start -W -n org.mittens.engine.dev/android.app.NativeActivity
adb logcat -v threadtime 'mitten-droid:I' 'AndroidRuntime:E' 'libc:F' '*:S'
```

In another terminal, exercise background/foreground and a normal exit.
On the tested Focus 3 firmware this activity uses virtual display 1. A Home
key caused only a brief pause/resume; foregrounding its existing 2D Home
activity produced the complete pause/window teardown/stop transition:

```sh
adb shell am start -W --display 1 -n com.vive.android.secondaryhome/.launcher.EmptyActivity
adb shell am start -W -n org.mittens.engine.dev/android.app.NativeActivity
adb shell input keyevent KEYCODE_WAKEUP
adb shell input -d 1 keyevent KEYCODE_BACK
```

Expected logs include `host_start`, `android_context ready`, `Start`, `Resume`,
`Pause`/`Stop`, a later `Resume`, then `Destroy` and `host_exit clean` on exit.
The platform may add window/focus/save-state events and may retain a stopped
activity. Check the observed sequence rather than assuming exact event order.
Repeat launch and exit to check that subsequent activity instances also
initialize their context correctly.

Keep the headset awake while testing injected keys; events can be ignored
while it sleeps. Display IDs and the secondary Home component are specific
to this firmware: inspect `adb shell dumpsys activity activities` if the
commands do not target your running host. Do not change device settings to
force it awake for this test.

For a bounded log snapshot and emergency stop:

```sh
adb logcat -d -v threadtime 'mitten-droid:I' 'AndroidRuntime:E' 'libc:F' '*:S'
adb shell am force-stop org.mittens.engine.dev
```

`force-stop` kills the process and does not prove graceful destruction. Use
Back and the `Destroy`/`host_exit clean` logs for that check. See
[Focus 3 lifecycle validation](focus-3-lifecycle.md) for captured results.

## Next: Android OpenXR, then Vulkan

Initialize the Android OpenXR loader and instance, report runtime identity,
and obtain the HMD system before creating graphics resources. Package the
appropriate loader and add runtime discovery/permissions at that stage.
Then create a Vulkan session, clear both eyes, draw a triangle, and record the
[capability report](focus-3-probe.md) before starting the full engine renderer.

The local reference is `/home/rei/_/ALVR/ALVR/alvr/client_openxr/`. Its activity
and packaging provide Android integration examples; its GL renderer is not
the rendering backend for this host.

The remaining [implementation slices](../../docs/task/epic/android_support/implementation-slices.md)
cover Android OpenXR initialization, Vulkan stereo output, capability capture,
and the first embedded Mittens scene.
