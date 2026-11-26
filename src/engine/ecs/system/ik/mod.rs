//! IK-adjacent humanoid policy systems.
//!
//! The native head/neck/body constraint owns coordinated avatar placement.
//! Legacy body follow is its mutually exclusive unsupported-rig fallback.
//! `ik_system` remains at the parent level for chain-solver kinds.

pub mod body;
mod debug_draw;
pub mod head_neck_body_constraint;
mod head_pose_body_xz_follow;
pub mod lower_body;
pub(crate) mod rest_pose;
pub mod upper_body;
pub(crate) mod two_bone;

pub use head_neck_body_constraint::{HeadNeckBodyConstraint, HeadNeckBodyConstraintSystem};
pub use body::{BodyMode, BodySolverSystem};
pub use lower_body::LowerBodyIkSystem;
pub use upper_body::UpperBodyIkSystem;
