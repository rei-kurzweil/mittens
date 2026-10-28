//! Shared lower-body ownership. Policies produce goals; one system writes legs.
mod crouch;
mod leg;
mod rig;
mod support;
mod system;
mod walking;

use crate::engine::ecs::ComponentId;
pub use system::LowerBodyIkSystem;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LowerBodyStatus {
    #[default]
    Pending,
    Disabled,
    Mounted,
    NeedsSoleCalibration,
    NoSupport,
    Ready,
    Limited,
    UnsupportedRig(&'static str),
}

#[derive(Debug, Clone, Copy)]
pub struct FootState {
    pub support: Option<ComponentId>,
    pub support_height: Option<f32>,
    pub base_sole: [f32; 3],
    pub solved_sole: [f32; 3],
    pub penetration: f32,
    pub knee_flexion: f32,
    pub residual: f32,
    /// Desired world-space sole; remains fixed during stance.
    pub target_sole: [f32; 3],
    pub target_rotation: [f32; 4],
    /// Full target error, including unreachable extension above the floor.
    pub target_error: f32,
    pub phase: FootPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FootPhase {
    Rest,
    Planted,
    Swing,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WalkingDiagnostics {
    /// Left, right. Reset on gait release or binding replacement.
    pub steps_started: [u64; 2],
    pub swing_side: Option<usize>,
    pub swing_progress: Option<f32>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct FootGoal {
    pub sole: [f32; 3],
    pub rotation: [f32; 4],
}

#[cfg(test)]
mod tests;
