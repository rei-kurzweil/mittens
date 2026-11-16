//! Hand drivers: grab attachment, targets, aim correction, hand basis, pose corrections.
use super::*;

pub(super) fn find_xr_pose_driver(world: &World, start: ComponentId) -> Option<ComponentId> {
    let mut current = Some(start);
    while let Some(component) = current {
        if world
            .get_component_by_id_as::<ControllerXRComponent>(component)
            .is_some()
            || world
                .get_component_by_id_as::<crate::engine::ecs::component::InputXRComponent>(
                    component,
                )
                .is_some()
        {
            return Some(component);
        }
        current = world.parent_of(component);
    }
    None
}

/// A canonical grip follows the solved skeleton, independently of the raw IK driver.
pub(super) fn ensure_hand_grab_attachment(
    world: &mut World,
    controller: ComponentId,
    hand: ComponentId,
    correction: Option<[f32; 4]>,
    emit: &mut dyn SignalEmitter,
) {
    use crate::engine::ecs::component::PointerComponent;
    let pointers: Vec<_> = world
        .all_components()
        .filter(|&id| {
            if world
                .get_component_by_id_as::<PointerComponent>(id)
                .is_none()
            {
                return false;
            }
            let mut cursor = world.parent_of(id);
            while let Some(parent) = cursor {
                if parent == controller {
                    return true;
                }
                cursor = world.parent_of(parent);
            }
            false
        })
        .collect();
    if pointers.is_empty() {
        return;
    }
    let existing = world
        .children_of(hand)
        .iter()
        .copied()
        .find(|&id| world.component_label(id) == Some("avatar_hand_grip_attachment"));
    let grip = existing.unwrap_or_else(|| {
        let id = world.add_component(
            TransformComponent::new()
                .with_rotation_quat(quat_conjugate(correction.unwrap_or([0., 0., 0., 1.]))),
        );
        world.get_component_record_mut(id).unwrap().name = "avatar_hand_grip_attachment".into();
        let serialization = world.add_component(SerializeComponent::off());
        world
            .add_child(id, serialization)
            .expect("runtime grip serialization marker");
        world
            .add_child(hand, id)
            .expect("new grip frame under hand");
        world.init_component_tree(id, emit);
        id
    });
    for pointer in pointers {
        world
            .get_component_by_id_as_mut::<PointerComponent>(pointer)
            .unwrap()
            .avatar_grab_attachment = Some(grip);
    }
}

/// Create the optional rotation-correction child beneath an already classified hand target.
pub(super) fn resolve_hand_target(
    world: &mut World,
    binding: ArmIkBinding,
    rotation_offset: Option<[f32; 4]>,
) -> (ArmIkBinding, ComponentId, ComponentId) {
    let driver = binding.raw_target;
    let hand_driver = if let Some(offset_q) = rotation_offset {
        let offset = world.add_component(TransformComponent::new().with_rotation_quat(offset_q));
        let offset_serialize_id = world.add_component(SerializeComponent::off());
        let _ = world.set_parent(offset_serialize_id, Some(offset));
        let _ = world.set_parent(offset, Some(driver));
        offset
    } else {
        driver
    };

    (binding, driver, hand_driver)
}

/// Returns `None` only while the imported skeleton or its retained basis is still in flight.
/// The inner option is absent when this target has no usable authored basis.
pub(super) fn derive_hand_aim_correction(
    _world: &World,
    retargeting: &JointBasisRetargetingSystem,
    _model_root: ComponentId,
    hand_bone: Option<ComponentId>,
    side: &str,
) -> Option<Option<[f32; 4]>> {
    let Some(hand_bone) = hand_bone else {
        return Some(None);
    };
    match retargeting.status_for(hand_bone) {
        Some(RetargetBasisStatus::Ready) => {
            // HumanoidBoneMap will provide this same target hand ID through semantic slots.
            return retargeting
                .basis_for(hand_bone)
                .map(|basis| Some(mat_to_quat(basis.target_rest_to_canonical)));
        }
        Some(RetargetBasisStatus::Invalid(error)) => {
            eprintln!("[AVC][hand-basis] {side} retained definition is invalid: {error}");
            return Some(None);
        }
        Some(RetargetBasisStatus::ConflictingDefinition { .. }) => {
            eprintln!("[AVC][hand-basis] {side} has conflicting retained definitions");
            return Some(None);
        }
        Some(RetargetBasisStatus::WaitingForGltf) => return None,
        None => {}
    }
    Some(None)
}

pub(super) fn ensure_map_hand_basis(
    world: &World,
    retargeting: &mut JointBasisRetargetingSystem,
    report: &HumanoidBoneMapReport,
    left: bool,
) {
    let (hand, middle_start, middle_end, little, index) = if left {
        (
            HumanoidSlot::LeftHand,
            HumanoidSlot::LeftMiddleProximal,
            HumanoidSlot::LeftMiddleDistal,
            HumanoidSlot::LeftLittleProximal,
            HumanoidSlot::LeftIndexProximal,
        )
    } else {
        (
            HumanoidSlot::RightHand,
            HumanoidSlot::RightMiddleProximal,
            HumanoidSlot::RightMiddleDistal,
            HumanoidSlot::RightLittleProximal,
            HumanoidSlot::RightIndexProximal,
        )
    };
    let Some(target) = map_target(report, hand) else {
        return;
    };
    // An authored definition is the expert override and is never replaced.
    if retargeting.status_for(target).is_some() {
        return;
    }
    let Some(forward_start) = map_target(report, middle_start) else {
        return;
    };
    let Some(forward_end) = map_target(report, middle_end) else {
        return;
    };
    let Some(up_start) = map_target(report, little) else {
        return;
    };
    let Some(up_end) = map_target(report, index) else {
        return;
    };
    retargeting.replace_definition(
        world,
        target,
        RetargetBasisDefinition {
            target,
            forward: LandmarkDirection {
                start: forward_start,
                end: forward_end,
            },
            up: LandmarkDirection {
                start: up_start,
                end: up_end,
            },
        },
    );
}

pub(super) fn update_hand_pose_corrections(
    avc_id: ComponentId,
    world: &World,
    emit: &mut dyn SignalEmitter,
    log_alignment: bool,
) {
    let (left_raw, right_raw, left_visual, right_visual, left_correction, right_correction) = {
        let Some(c) = world.get_component_by_id_as::<AvatarControlComponent>(avc_id) else {
            return;
        };
        (
            c.left_hand_raw_target_id,
            c.right_hand_raw_target_id,
            c.left_hand_visual_target_id,
            c.right_hand_visual_target_id,
            c.left_hand_aim_correction,
            c.right_hand_aim_correction,
        )
    };
    for (side, raw, visual, correction) in [
        (ControllerHand::Left, left_raw, left_visual, left_correction),
        (
            ControllerHand::Right,
            right_raw,
            right_visual,
            right_correction,
        ),
    ] {
        let controller = world.children_of(avc_id).iter().find_map(|id| {
            world
                .get_component_by_id_as::<ControllerXRComponent>(*id)
                .filter(|controller| controller.hand == side)
        });
        let source = controller
            .map(|controller| controller.active_pose_source)
            .unwrap_or(ControllerPoseSource::None);
        let uses_canonical_hand_basis = matches!(
            source,
            ControllerPoseSource::ControllerAim
                | ControllerPoseSource::ControllerGripAim
                | ControllerPoseSource::WristPalm
        );
        let applied = if uses_canonical_hand_basis {
            correction.unwrap_or([0.0, 0.0, 0.0, 1.0])
        } else {
            [0.0, 0.0, 0.0, 1.0]
        };
        if let (Some(raw), Some(visual)) = (raw, visual) {
            if raw != visual {
                update_local_rotation(world, emit, visual, applied);
            }
        }
        if log_alignment && source != ControllerPoseSource::None {
            log_hand_alignment(avc_id, world, side, controller, source, correction, applied);
        }
    }
}

pub(super) fn log_hand_alignment(
    avc_id: ComponentId,
    world: &World,
    hand: ControllerHand,
    controller: Option<&ControllerXRComponent>,
    source: ControllerPoseSource,
    correction: Option<[f32; 4]>,
    applied: [f32; 4],
) {
    let aim = controller.and_then(|controller| controller.raw_aim_rotation);
    let grip = controller.and_then(|controller| controller.raw_grip_rotation);
    let basis_mode = if correction.is_some() {
        "retained"
    } else {
        "none"
    };
    let grip_to_aim = match (grip, aim) {
        (Some(grip), Some(aim)) => Some(quat_to_axis_angle(quat_mul(quat_conjugate(grip), aim))),
        _ => None,
    };
    let mount = correction.map(quat_conjugate);
    let observed = world
        .get_component_by_id_as::<AvatarControlComponent>(avc_id)
        .and_then(|avatar| {
            let (raw, bone) = match hand {
                ControllerHand::Left => (avatar.left_hand_raw_target_id, avatar.left_hand_bone_id),
                ControllerHand::Right => {
                    (avatar.right_hand_raw_target_id, avatar.right_hand_bone_id)
                }
            };
            let rotation = |id| {
                world
                    .get_component_by_id_as::<TransformComponent>(id)
                    .map(|t| mat_to_quat(t.transform.matrix_world))
            };
            let target = rotation(raw?)?;
            let observed_canonical = quat_mul(rotation(bone?)?, mount?);
            // Native palm +Y is canonical -X for left, +X for right. These
            // observed bones precede this tick's IK, so compare while holding still.
            let normal_axis = [
                match hand {
                    ControllerHand::Left => -1.0,
                    ControllerHand::Right => 1.0,
                },
                0.0,
                0.0,
            ];
            let target_normal = quat_rotate_vec3(target, normal_axis);
            let observed_normal = quat_rotate_vec3(observed_canonical, normal_axis);
            let (axis, radians) =
                quat_to_axis_angle(quat_mul(quat_conjugate(target), observed_canonical));
            Some((target_normal, observed_normal, (axis, radians.to_degrees())))
        });
    let predicted = if matches!(
        source,
        ControllerPoseSource::ControllerAim
            | ControllerPoseSource::ControllerGripAim
            | ControllerPoseSource::WristPalm
    ) {
        mount.map(|mount| quat_to_axis_angle(quat_mul(applied, mount)))
    } else {
        None
    };
    let calibration = match source {
        ControllerPoseSource::WristPalm => "canonical-hand-calibrated",
        ControllerPoseSource::ControllerAim | ControllerPoseSource::ControllerGripAim => {
            "aim-calibrated"
        }
        _ => "identity",
    };
    eprintln!(
        "[AVC][hand-alignment] avc={avc_id:?} hand={hand:?} aim_valid={} grip_valid={} aim_q={aim:?} grip_q={grip:?} source={source:?} mode={calibration} avatar_basis={basis_mode} grip_to_aim_axis_angle={grip_to_aim:?} canonical_to_hand_q={mount:?} applied_correction_q={applied:?} final_basis_to_aim_axis_angle={predicted:?} pre_ik_target_normal_observed_normal_error_deg={observed:?}",
        aim.is_some(),
        grip.is_some(),
    );
}

pub(super) fn hand_alignment_debug_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CAT_DEBUG_XR_HAND_ALIGNMENT")
            .ok()
            .is_some_and(|value| value == "1")
    })
}
