use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::{Component, ComponentRef};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TwoBoneIkDebugVisuals {
    pub target_line: ComponentId,
    pub pole_line: ComponentId,
    pub plane_normal_line: ComponentId,
    pub elbow_line: ComponentId,
    pub elbow_point: ComponentId,
}

/// Solver configuration for an `IKChainComponent`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IKSolver {
    /// Single-bone pose match.
    ///
    /// Sets the root joint's world rotation to match the target TC's world rotation,
    /// post-multiplied by a fixed yaw offset.  Used for neck/head alignment from InputXR.
    ///
    /// `offset_yaw`: rotation applied after copying target world rotation.
    /// Use `std::f32::consts::PI` for the OpenXR (−Z forward) → VRM (+Z forward) flip.
    ///
    /// `copy_position`: when true, also overrides the joint's world position to the
    /// target's world position.  Required for HMD-driven head bones so the bone tracks
    /// physical head translation (HMD moves forward+down when you pitch), not just
    /// rotation.  Visually detaches the bone from its FK parent until a spine FABRIK
    /// solver bends the chain to follow.  Default false (rotation-only behavior).
    ///
    /// `target_position_offset`: offset applied in the **target's local frame** before
    /// copying its world position.  Used to compensate for the gap between the head
    /// bone pivot (typically at the skull base) and the camera/eye position: passing
    /// `(0, -eye_height, 0)` shifts the bone down so the eye mesh (which sits above
    /// the bone pivot) lands at the HMD position.  Ignored when `copy_position` is
    /// false.
    AimConstraint {
        offset_yaw: f32,
        copy_position: bool,
        target_position_offset: [f32; 3],
    },

    /// Closed-form 2-bone IK.
    ///
    /// Used for arms: UpperArm → LowerArm → Hand. All three joints are referenced
    /// by explicit `ComponentId` — the solver does NO topology discovery and is
    /// resilient to sibling helper / collider / cloth bones under the arm joints.
    /// The chain's `parent_of` is ignored for this solver; `end_effector_id` (the
    /// hand) lives on `IKChainComponent`, root + mid are here on the variant.
    ///
    /// `root_joint_id`: upper-arm TC (chain root).
    /// `mid_joint_id`:  lower-arm TC (elbow).
    /// `pole_direction`: direction hint for the middle joint (elbow / knee).
    ///   Interpreted in body-local space when an ancestor `AvatarControlComponent`
    ///   exists, otherwise world-space.  Body-local mode rotates the pole by the
    ///   model root's world rotation each tick so the elbow stays anatomically
    ///   correct when the body turns.
    /// `copy_end_rotation`: if true, also aligns the end-effector bone to the target's rotation.
    TwoBoneIK {
        root_joint_id: ComponentId,
        mid_joint_id: ComponentId,
        pole_direction: [f32; 3],
        copy_end_rotation: bool,
    },

    /// Iterative FABRIK solver — works for any chain length ≥ 2.
    ///
    /// Used for spine bending: chain hips → ... → splice_head with the head pose
    /// driver as the target.  The spine rotates so the end-effector (splice_head)
    /// FK-lands at the target position.
    ///
    /// `target_position_offset`: same semantics as `AimConstraint` — offset in the
    /// target's local frame, added to its world position before chasing.  Used to
    /// shift the target down by `eye_offset` so the head bone pivot (not the eye
    /// mesh above it) lines up with the HMD position.
    Fabrik {
        max_iterations: u32,
        tolerance: f32,
        target_position_offset: [f32; 3],
    },
}

/// Marks the root joint of an IK chain.
///
/// Place this as a **child of the root joint TC** (e.g. `J_Bip_L_UpperArm`, `splice_head`).
/// The IKSystem finds this component, reads its parent TC as the root joint, walks down to
/// `end_effector_id` to collect the chain, reads the target pose from `target_id`, solves,
/// and emits `UpdateTransform` for each joint.
///
/// All three solver types are expressed through this single component; no separate
/// end-effector or pole-vector marker components are required.
#[derive(Debug, Clone)]
pub struct IKChainComponent {
    /// Which solver to run.
    pub solver: IKSolver,

    /// TC whose world pose is the IK target this frame.
    ///
    /// For `AimConstraint`: target world rotation is read here.
    /// For `TwoBoneIK` / `Fabrik`: target world position (and optionally rotation) is read here.
    pub target_id: ComponentId,

    /// TC at the end of the bone chain.
    ///
    /// For `AimConstraint`: set to the root joint itself (chain length = 1).
    /// For `TwoBoneIK`: set to the hand/foot bone (2 TCs below the root joint).
    /// For `Fabrik`: set to the last bone in the spine/neck chain.
    pub end_effector_id: ComponentId,

    /// Blend weight: 0.0 = no IK applied, 1.0 = full solve.
    pub weight: f32,

    /// Authored form of `target_id` for round-trip dump. `None` for
    /// IKChains wired purely at runtime (e.g. by `AvatarControlSystem`),
    /// which have no MMS source to preserve.
    pub target_source: Option<ComponentRef>,
    /// Authored form of `end_effector_id` for round-trip dump.
    pub end_effector_source: Option<ComponentRef>,

    /// Cached ancestor `AvatarControlComponent` ID, discovered by `IKSystem`
    /// on first tick via a parent-chain walk.  When `Some`, the solver
    /// transforms `TwoBoneIK.pole_direction` from body-local to world space
    /// using the AVC's model root rotation.  `None` → world-space pole
    /// (current behavior for non-AVC chains).
    pub(crate) avc_id: Option<ComponentId>,

    /// Cached InputXR/XRHand component governing this runtime AVC target.
    pub(crate) xr_pose_driver: Option<ComponentId>,

    /// Lazily created runtime-only debug visual ids for TwoBoneIK inspection.
    pub(crate) two_bone_debug_visuals: Option<TwoBoneIkDebugVisuals>,

    component: Option<ComponentId>,
}

impl IKChainComponent {
    pub fn new(solver: IKSolver, target_id: ComponentId, end_effector_id: ComponentId) -> Self {
        Self {
            solver,
            target_id,
            end_effector_id,
            weight: 1.0,
            target_source: None,
            end_effector_source: None,
            avc_id: None,
            xr_pose_driver: None,
            two_bone_debug_visuals: None,
            component: None,
        }
    }

    pub fn with_weight(mut self, w: f32) -> Self {
        self.weight = w;
        self
    }

    pub fn with_target_source(mut self, src: ComponentRef) -> Self {
        self.target_source = Some(src);
        self
    }

    pub fn with_end_effector_source(mut self, src: ComponentRef) -> Self {
        self.end_effector_source = Some(src);
        self
    }
}

impl Component for IKChainComponent {
    fn name(&self) -> &'static str {
        "ik_chain"
    }

    fn set_id(&mut self, id: ComponentId) {
        self.component = Some(id);
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(component, crate::engine::ecs::IntentValue::RegisterIkChain { component_ids: vec![component] });
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        use crate::meow_meow::ast::Expression;
        let solver_call = match self.solver {
            IKSolver::AimConstraint {
                offset_yaw,
                copy_position,
                target_position_offset,
            } => (
                "aim_constraint",
                vec![
                    num(offset_yaw as f64),
                    b(copy_position),
                    array(nums(target_position_offset.iter().map(|&v| v as f64))),
                ],
            ),
            IKSolver::TwoBoneIK {
                pole_direction,
                copy_end_rotation,
                ..
            } => (
                "two_bone_ik",
                vec![
                    array(nums(pole_direction.iter().map(|&v| v as f64))),
                    b(copy_end_rotation),
                ],
            ),
            IKSolver::Fabrik {
                max_iterations,
                tolerance,
                target_position_offset,
            } => (
                "fabrik",
                vec![
                    num(max_iterations as f64),
                    num(tolerance as f64),
                    array(nums(target_position_offset.iter().map(|&v| v as f64))),
                ],
            ),
        };
        fn target_expr(t: &ComponentRef) -> Expression {
            match t {
                ComponentRef::Guid(u) => Expression::String(format!("@uuid:{u}")),
                ComponentRef::Query(s) => Expression::String(s.clone()),
            }
        }
        let mut ce = ce_call("IKChain", solver_call.0, solver_call.1)
            .with_call("weight", vec![num(self.weight as f64)]);
        if let Some(src) = &self.target_source {
            ce = ce.with_call("target", vec![target_expr(src)]);
        }
        if let Some(src) = &self.end_effector_source {
            ce = ce.with_call("end_effector", vec![target_expr(src)]);
        }
        ce
    }
}
