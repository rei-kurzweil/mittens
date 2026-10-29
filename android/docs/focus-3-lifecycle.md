# Focus 3 Android host validation

Verified on 2026-10-29 using the USB-connected VIVE Focus 3. This establishes
APK packaging, native library loading, Android context initialization, and
activity lifecycle. The user also confirmed launcher startup, resizing, and
closing of the blank activity panel directly in the headset OS. This does not
establish engine execution, OpenXR runtime initialization, Vulkan support, or
stereo graphics presentation.

## Environment and artifact

| Item | Recorded value |
| --- | --- |
| Device | VIVE Focus 3 (`kona`), arm64-v8a |
| Android | 10, API 29 |
| Build display ID | `QKQ1.210528.001 release-keys` |
| Build incremental | `11634` |
| Host package/activity | `org.mittens.engine.dev/android.app.NativeActivity` |
| Launcher label | Mittens Host |
| APK minimum/target API | 28 / 32 |
| NDK / CMake | 28.1.13356709 / 3.30.3 |
| Packaging | cargo-apk 0.10.0, debug signing; [build helper](../packaging/build-apk.sh) |
| APK | `target/debug/apk/mitten-droid.apk` |
| SHA-256 | `2b47c32aa77b0ba3f58db9b8ce78b3ad9e546a1b11fb64c8d83bec097d2ac66d` |

The APK signature verified and installation succeeded. The packaged arm64
library exports `ANativeActivity_onCreate` and `android_main`; its reported
shared-library dependencies are Android system libraries (`libandroid.so`,
`libc.so`, `libdl.so`, `liblog.so`). No external OpenXR loader is packaged yet.

## Device results

| Check | Observed result |
| --- | --- |
| Native startup | `host_start`, valid JVM/activity context, `Start`, `Resume`, native window events, `GainedFocus`. |
| Headset OS presentation | User confirmed that launching from the headset opens a blank activity panel that can be resized and closed. This is an OS-managed 2D panel, not an XR session. |
| Background | `Pause`, `LostFocus`, `TerminateWindow`, `Stop`, `SaveState`. |
| Foreground | `Start`, `Resume`, new native window, `GainedFocus`, without another `host_start`. |
| Normal Back exit | `Pause`, window/focus teardown, `Stop`, `Destroy`, `host_exit clean`. |
| Relaunch after destruction | Same process (PID 6115), new native application thread; context check and startup passed again, followed by another clean Back exit. |
| Fresh process launch | After force-stopping the already exited activity's process, `am start -W` reported `COLD`; new PID 6940 initialized normally and also exited cleanly through Back. |
| Failure diagnostics | No host panic, Java runtime exception, or native fatal message in the captured per-process logs. |

Captured final-build logs:

- [Background/resume, clean exit, and activity recreation](validation/focus-3-lifecycle-2026-10-29.log)
- [Fresh process startup and normal exit](validation/focus-3-cold-start-2026-10-29.log)

Device log timestamps show December 8 because the headset clock differs from
the development machine; the test date above is the host date. No device clock
or persistent device settings were changed.

## Focus 3 testing details

The host runs as a blank NativeActivity on the firmware's virtual 2D display
1 until an XR session is implemented. It has not demonstrated stereo output.
Injected keys can be ignored while the headset sleeps. For a reliable full
background transition, the test foregrounded the existing
`com.vive.android.secondaryhome/.launcher.EmptyActivity` on display 1, then
resumed the host. A plain Home key caused only a brief pause/resume in this run.
Back was injected on display 1 with the device awake. Exact commands are in
the [host bootstrap](host-bootstrap.md).

Immediately after replace-installing the final APK, one `am start -W` returned
the secondary Home activity. Repeating the launch reached the host; no crash
was recorded. A later explicit fresh-process launch passed. Keep this observed
launcher/install transition in mind if an immediate post-install launch returns
Home; inspect logs rather than treating `Status: ok` alone as startup evidence.

The final test ended with normal activity destruction and the headset back at
Home. Android can retain the process after `android_main` returns; clean
activity destruction does not require the process to disappear.

## Remaining work

Initialize Android OpenXR and capture runtime/extension diagnostics, then
implement the [Vulkan/OpenXR probe](focus-3-probe.md). Lifecycle with graphics
active, physical headset-removal behavior, engine workers, audio, tracking,
and sustained performance remain untested.
