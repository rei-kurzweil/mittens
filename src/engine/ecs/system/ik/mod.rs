//! IK-adjacent humanoid policy systems.
//!
//! Houses the head-pose-sensitive body XZ translate follow module that
//! replaced the scrapped planar-deadzone heuristic.  The original
//! `ik_system` module remains at the parent level for chain-solver kinds
//! (AimConstraint, TwoBoneIK, FABRIK).

pub mod head_pose_body_xz_follow;

pub use head_pose_body_xz_follow::HeadPoseBodyXzFollowSystem;
