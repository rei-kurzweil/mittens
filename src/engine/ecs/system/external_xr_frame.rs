//! A host-owned OpenXR frame sampled once at its predicted presentation time.
use super::vr_types::{XrGamepadState, XrInputState};
use openxr as xr;

#[derive(Clone, Copy, Default)]
pub struct ExternalXrFrame {
    /// Eye poses and FOVs in the host's reference space; absent on tracking loss.
    pub views: Option<[xr::View; 2]>,
    /// Controller aim and grip poses in the same reference space and frame.
    pub aim: [Option<xr::Posef>; 2],
    pub grip: [Option<xr::Posef>; 2],
    pub input: XrInputState,
    pub gamepad: XrGamepadState,
    /// Eye data from the host's own OpenXR session, already normalized by the
    /// host's backend adapter (for example XR_HTC_facial_tracking). `None`
    /// when no native eye provider is active or it reported no data this frame.
    pub eyes: Option<NativeEyeSample>,
}

/// Provider-neutral eye data from a native XR backend. Backends fill only the
/// fields they actually measure; absent fields stay `None` and never borrow a
/// value from another backend.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NativeEyeSample {
    /// Lid closure per eye, `[left, right]`: 0 = open, 1 = closed.
    pub closure: [Option<f32>; 2],
}
