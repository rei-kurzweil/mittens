//! IK-adjacent humanoid policy systems.
//!
//! Started as a home for the simple-humanoid body-follow heuristic that replaced
//! the AVC spine FABRIK chain.  The original `ik_system` module remains at the
//! parent level for chain-solver kinds (AimConstraint, TwoBoneIK, FABRIK).

pub mod simple_humanoid;

pub use simple_humanoid::SimpleHumanoidSystem;
