//! Arm IK binding and eligibility. Moves to the upper body solver later.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ArmIkBinding {
    pub(super) controller: ComponentId,
    pub(super) raw_target: ComponentId,
    pub(super) upper_arm: ComponentId,
    pub(super) lower_arm: ComponentId,
    pub(super) hand: ComponentId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ArmIkEligibility {
    Eligible(ArmIkBinding),
    IncompleteArmMap,
    NoHandDriver,
    MalformedHandDriver,
}

/// Classify one AVC arm without mutating the world.
///
/// A complete humanoid arm is only a capability. AVC-owned arm IK activates when
/// that capability also has an enabled, direct hand pose driver with the tracked
/// transform topology expected by `OpenXRSystem`.
pub(super) fn classify_arm_ik_eligibility(
    world: &World,
    humanoid_map: &HumanoidBoneMapReport,
    controller: Option<ComponentId>,
    left: bool,
) -> ArmIkEligibility {
    let (upper_slot, lower_slot, hand_slot) = if left {
        (
            HumanoidSlot::LeftUpperArm,
            HumanoidSlot::LeftLowerArm,
            HumanoidSlot::LeftHand,
        )
    } else {
        (
            HumanoidSlot::RightUpperArm,
            HumanoidSlot::RightLowerArm,
            HumanoidSlot::RightHand,
        )
    };
    let (Some(upper_arm), Some(lower_arm), Some(hand)) = (
        map_target(humanoid_map, upper_slot),
        map_target(humanoid_map, lower_slot),
        map_target(humanoid_map, hand_slot),
    ) else {
        return ArmIkEligibility::IncompleteArmMap;
    };
    let Some(controller) = controller else {
        return ArmIkEligibility::NoHandDriver;
    };
    let Some(config) = world.get_component_by_id_as::<ControllerXRComponent>(controller) else {
        return ArmIkEligibility::MalformedHandDriver;
    };
    if !config.enabled {
        return ArmIkEligibility::NoHandDriver;
    }
    let Some(raw_target) = world
        .children_of(controller)
        .iter()
        .copied()
        .find(|&child| {
            world
                .get_component_by_id_as::<TransformComponent>(child)
                .is_some()
        })
    else {
        return ArmIkEligibility::MalformedHandDriver;
    };
    ArmIkEligibility::Eligible(ArmIkBinding {
        controller,
        raw_target,
        upper_arm,
        lower_arm,
        hand,
    })
}

/// Create one two-bone IK chain per prepared hand.
pub(super) fn create_arm_chains(
    world: &mut World,
    id: ComponentId,
    sides: [(
        PreparedHand,
        crate::engine::ecs::component::ArmTwoBoneIkConfig,
        &str,
    ); 2],
    emit: &mut dyn SignalEmitter,
) {
    // -----------------------------------------------------------------------
    // Arm IK (TwoBoneIK) with explicit joint IDs.
    //
    // For each side: resolve all three arm bones (upper + lower + hand) and
    // hand them to the solver via `IKSolver::TwoBoneIK { root_joint_id,
    // mid_joint_id, .. }` + `IKChainComponent::end_effector_id`. The solver
    // does no topology discovery, so sibling cloth / collider / helper bones
    // under the arm joints (e.g. bisket's `J_Sec_L_TopsUpperArm_*` and
    // `J_Bip_L_UpperArm_collider_*`) are irrelevant.
    //
    // Bone IDs and the tracked target come from the already validated
    // `ArmIkBinding`; there is no name lookup, topology inference, or synthetic
    // origin target in this construction path.
    // -----------------------------------------------------------------------
    for (hand_opt, ik_config, side_label) in sides {
        let Some(((binding, raw_driver, hand_driver), _)) = hand_opt else {
            continue;
        };
        let upper_arm = binding.upper_arm;
        let lower_arm = binding.lower_arm;
        let hand_bone = binding.hand;

        let bone_name =
            |id: ComponentId| -> String { world.component_name(id).unwrap_or("?").to_string() };
        let upper_name_s = bone_name(upper_arm);
        let lower_name_s = bone_name(lower_arm);
        let hand_name_s = bone_name(hand_bone);
        println!(
            "[AVC] {} arm IK: root={} (id={:?}), mid={} (id={:?}), hand={} (id={:?}), target=(id={:?})",
            side_label,
            upper_name_s,
            upper_arm,
            lower_name_s,
            lower_arm,
            hand_name_s,
            hand_bone,
            hand_driver,
        );
        let looks_suspicious = |n: &str| {
            n.contains("Twist")
                || n.contains("Roll")
                || n.contains("Helper")
                || n.contains("_collider")
                || n.contains("J_Sec_")
        };
        if looks_suspicious(&upper_name_s) || looks_suspicious(&lower_name_s) {
            println!(
                "[AVC] WARNING: {} arm IK resolved to a helper/cloth/collider bone — \
                override the {}_upper_arm and {}_lower_arm slots in HumanoidBoneMap.",
                side_label, side_label, side_label
            );
        }

        let mut chain = IKChainComponent::new(
            IKSolver::TwoBoneIK {
                root_joint_id: upper_arm,
                mid_joint_id: lower_arm,
                pole_direction: ik_config.pole_direction,
                copy_end_rotation: ik_config.copy_end_rotation,
            },
            hand_driver,
            hand_bone,
        );
        chain.weight = ik_config.weight;
        chain.forbidden_bend_normal_z_degrees = ik_config.forbidden_bend_normal_z_degrees;
        chain.xr_pose_driver = find_xr_pose_driver(world, hand_driver);
        let chain_id = world.add_component(chain);
        let chain_serialize_id = world.add_component(SerializeComponent::off());
        let _ = world.set_parent(chain_serialize_id, Some(chain_id));
        let _ = raw_driver;
        // Parent under AVC for cleanup; the solver itself ignores the chain's parent.
        emit_attach(emit, id, chain_id);
    }
}
