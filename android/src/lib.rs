//! Android application host for Mittens Engine.
//!
//! The current host handles NativeActivity lifecycle and input delivery, with
//! logcat diagnostics. Engine startup, OpenXR, and Vulkan rendering come next.
//! See `android/docs/host-bootstrap.md` for build and device-test commands.
//!
//! Portable engine work belongs in `mittens-engine`.

#[cfg(target_os = "android")]
mod activity;

/// Called by android-activity on its native application thread.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    activity::run(app);
}
