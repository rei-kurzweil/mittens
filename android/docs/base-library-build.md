# Portable engine library builds

The base `mittens-engine` library can build independently of desktop windowing.
The default `desktop` feature retains existing desktop behavior. Disable default
features for an Android host or a desktop XR-only build without Winit.

The `mitten-droid` host at `android/Cargo.toml` declares its engine
dependency only for Android:

```toml
[target.'cfg(target_os = "android")'.dependencies]
mittens-engine = { path = "..", default-features = false }
```

`Windowing`, desktop CLI/executable, Winit event conversion, and window Vulkan
surface/swapchain APIs require `desktop`. The generic HID provider interface,
`UserInput::handle_event`, ECS, scripting, audio, Vulkan resources, and XR-only
startup remain in the base library. The unused `vulkano-util` dependency has
been removed. `InputState` and its key/button types have no Winit dependency.

## Commands

Check the portable library on the development machine:

```sh
cargo check --lib --no-default-features --locked
cargo test --test portable_hid_provider --no-default-features --locked
cargo run --release --example openxr-only --no-default-features
```

For Android, install the `aarch64-linux-android` Rust target, Android NDK, and
`cargo-ndk`. Set `ANDROID_NDK_HOME` to the installed NDK directory and
`ANDROID_NDK` to the same directory so CMake can find it too.

The locked `shaderc-sys 0.8.3` can build native code during cross-compilation.
Its bundled project fails to configure with the system CMake 4.4's default
policy compatibility. Use CMake 3.x for this build. With the versions installed
in the local SDK, the setup is:

```sh
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/28.1.13356709"
export ANDROID_NDK="$ANDROID_NDK_HOME"
export CMAKE="$ANDROID_HOME/cmake/3.30.3/bin/cmake"
```

Then run:

```sh
cargo ndk -t arm64-v8a -p 28 check --lib --no-default-features --locked
cargo ndk -t arm64-v8a -p 28 build --lib --no-default-features --locked
```

API level 28 is the initial build baseline, following the local ALVR client.
The eventual APK manifest/runtime requirements must be chosen for the actual
headset and packaging workflow.

## Validation boundary

On 2026-10-21 the arm64 Android library cross-check and build passed with NDK
28.1.13356709 and desktop support disabled. The build produces
`target/aarch64-linux-android/debug/libmittens_engine.rlib`. This is a Rust
library artifact, not an APK, linked Android activity, or Focus 3 Vulkan runtime
test.

Focused graphics/XR/input tests passed in both configurations: 98 library tests
with desktop support, 96 without it, plus the public-API consumer test in each.
The stereo GPU test passed without desktop support on a GeForce GTX 1080,
including pixel readback with MSAA off and at 4x. All desktop executables and
examples passed `cargo check` with default features.

The XR-only example built with `--no-default-features` completed a bounded
desktop OpenXR run, reaching SYNCHRONIZED and printing the no-HID message once.
The default-feature windowed cube/bloom example reached the present loop and
completed window rendering. These desktop checks do not establish headset
input routing or Vulkan feature support on Android.

The host crate, `mitten-droid`, lives under `android/`. Its APK and
NativeActivity lifecycle were [verified on Focus 3](focus-3-lifecycle.md) on
2026-10-29. See the [host bootstrap](host-bootstrap.md) for build and device
commands. The shared Android loader hook and instance setup are now implemented,
and the [OpenXR/Vulkan probe](xr-vulkan-bringup.md) passed on Focus 3, with
user-confirmed stereo output. [Measured capabilities](focus-3-vulkan-results.md)
require a classic render-pass path for the full engine on this driver. Engine
scene startup and input adaptation follow; ALVR remains the platform reference.
