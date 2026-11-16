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
