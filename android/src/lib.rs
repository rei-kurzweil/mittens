//! Android application host for Mittens Engine.
//!
//! This crate is the host scaffold: activity/lifecycle handling, Android context
//! and OpenXR loader initialization, and the XR frame loop will live here.
//! Portable engine work belongs in `mittens-engine`.
//!
//! There is no `android_main` entry point or APK packaging yet. The next step is
//! a NativeActivity event loop with logcat diagnostics, before creating any XR
//! or Vulkan resources. See `android/docs/host-bootstrap.md`.
