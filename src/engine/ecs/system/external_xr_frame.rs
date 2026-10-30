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
}
