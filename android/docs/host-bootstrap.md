# Android host bootstrap

`mitten-droid` is a workspace crate rooted at `android/Cargo.toml`. It consumes
`mittens-engine`; `crates/` contains the engine's standalone dependencies.
Android application code, packaging, and platform diagnostics belong here.

## Current scaffold

- Native `cdylib` crate with an Android-only dependency on `mittens-engine`,
  using `default-features = false` to exclude desktop windowing.
- Workspace membership and a shared root lockfile/target directory.
- No activity entry point, Android context initialization, APK packaging,
  OpenXR loader integration, or rendering yet.

From the repository root, check the scaffold on the development machine:

```sh
cargo check -p mitten-droid --locked
```

That command checks only the host scaffold on desktop. To also check its
engine dependency for arm64 Android, configure the NDK and CMake as described
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

## Next implementation

1. Add `android-activity` with the `native-activity` feature and an
   `android_main` entry point in `src/lib.rs` (or an Android-only module).
   Start from ALVR's `=0.6.0` pin and documented context constraint. Confirm
   how that version initializes `ndk-context` before adding context setup.
2. Add logcat diagnostics and an activity event loop. Handle input availability
   and exit cleanly on destroy. Keep XR/graphics startup out of this first
   lifecycle test; add worker shutdown/joining when workers are introduced.
3. Use ALVR's `cargo apk` packaging recipe as the starting point. Put its
   manifest metadata in `android/Cargo.toml` and any resources/build helpers
   under `android/packaging/`. Choose a Mittens package identity and include
   only requirements used by this host. Document and verify the exact build,
   install, launch, log capture, and stop commands when packaging exists.
4. Verify startup, background/foreground, repeated launch, and shutdown on the
   Focus 3 before adding OpenXR initialization and the Vulkan capability probe.

The local reference is `/home/rei/_/ALVR/ALVR/alvr/client_openxr/`. Its activity
and packaging provide Android integration examples; its GL renderer is not
the rendering backend for this host.

The remaining [implementation slices](../../docs/task/epic/android_support/implementation-slices.md)
cover Android OpenXR initialization, Vulkan stereo output, capability capture,
and the first embedded Mittens scene.
