//! Android application host for Mittens Engine.
//!
//! NativeActivity lifecycle and an optional OpenXR/Vulkan capability/triangle
//! probe. Default features enable the probe; disabling defaults retains the
//! lifecycle-only host. See `android/docs/xr-vulkan-bringup.md`.
//!
//! Portable engine work belongs in `mittens-engine`.

#[cfg(target_os = "android")]
mod activity;

#[cfg(all(target_os = "android", feature = "vulkan-probe"))]
mod vulkan_probe;
#[cfg(all(target_os = "android", feature = "xr-probe"))]
mod xr_probe;

/// Called by android-activity on its native application thread.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    activity::run(app);
}
