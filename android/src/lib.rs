//! Android application host for Mittens Engine.
//!
//! NativeActivity lifecycle and an application-owned OpenXR/Vulkan scene
//! renderer. Default features enable the scene; disabling defaults retains the
//! lifecycle-only host. See `android/docs/xr-vulkan-bringup.md`.
//!
//! Portable engine work belongs in `mittens-engine`.

#[cfg(target_os = "android")]
mod activity;

#[cfg(all(target_os = "android", feature = "mobile-renderer"))]
mod demo_assets;
#[cfg(all(target_os = "android", feature = "mobile-renderer"))]
mod frame_profile;
#[cfg(all(target_os = "android", feature = "vulkan-probe"))]
mod frame_renderer;
#[cfg(all(target_os = "android", feature = "vulkan-probe"))]
mod graphics_device;
#[cfg(all(target_os = "android", feature = "mobile-renderer"))]
mod renderer;
#[cfg(all(target_os = "android", feature = "mobile-renderer"))]
mod scene_runtime;
#[cfg(all(
    target_os = "android",
    feature = "vulkan-probe",
    not(feature = "mobile-renderer")
))]
mod triangle_probe;
#[cfg(all(target_os = "android", feature = "mobile-renderer"))]
mod xr_controllers;
#[cfg(all(target_os = "android", feature = "xr-probe"))]
mod xr_host;
#[cfg(all(target_os = "android", feature = "vulkan-probe"))]
mod xr_resolution;
#[cfg(all(target_os = "android", feature = "vulkan-probe"))]
mod xr_session;
#[cfg(all(target_os = "android", feature = "resolution-probe"))]
mod xr_swapchain_probe;

/// Called by android-activity on its native application thread.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    activity::run(app);
}
