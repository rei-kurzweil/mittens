use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Solver configuration for an `IKChainComponent`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IKSolver {
    /// Single-bone orientation match.
    ///
    /// Sets the root joint's world rotation to match the target TC's world rotation,
    /// post-multiplied by a fixed yaw offset.  Used for neck/head alignment from InputXR.
    ///
    /// `offset_yaw`: rotation applied after copying target world rotation.
    /// Use `std::f32::consts::PI` for the OpenXR (−Z forward) → VRM (+Z forward) flip.
    AimConstraint { offset_yaw: f32 },

    /// Closed-form 2-bone IK.
    ///
    /// Requires exactly 2 TC joints between the root joint and `end_effector_id`.
    /// Used for arms: UpperArm → LowerArm → Hand.
    ///
    /// `pole_direction`: world-space hint for the middle joint (elbow/knee).
    /// `copy_end_rotation`: if true, also aligns the end-effector bone to the target's rotation.
    TwoBoneIK {
        pole_direction: [f32; 3],
        copy_end_rotation: bool,
    },

    /// Iterative FABRIK solver — works for any chain length ≥ 2.
    ///
    /// Used for spine bending (future, gated on TranslationFollow existing).
    Fabrik {
        max_iterations: u32,
        tolerance: f32,
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

    component: Option<ComponentId>,
}

impl IKChainComponent {
    pub fn new(solver: IKSolver, target_id: ComponentId, end_effector_id: ComponentId) -> Self {
        Self {
            solver,
            target_id,
            end_effector_id,
            weight: 1.0,
            component: None,
        }
    }

    pub fn with_weight(mut self, w: f32) -> Self {
        self.weight = w;
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(&self) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        // target_id / end_effector_id are runtime-only (wired by AvatarControlSystem);
        // omitted from the AST.
        let solver_call = match self.solver {
            IKSolver::AimConstraint { offset_yaw } => {
                ("aim_constraint", vec![num(offset_yaw as f64)])
            }
            IKSolver::TwoBoneIK { pole_direction, copy_end_rotation } => (
                "two_bone_ik",
                vec![
                    array(nums(pole_direction.iter().map(|&v| v as f64))),
                    b(copy_end_rotation),
                ],
            ),
            IKSolver::Fabrik { max_iterations, tolerance } => (
                "fabrik",
                vec![num(max_iterations as f64), num(tolerance as f64)],
            ),
        };
        ce_call("IKChain", solver_call.0, solver_call.1)
            .with_call("weight", vec![num(self.weight as f64)])
    }
}
