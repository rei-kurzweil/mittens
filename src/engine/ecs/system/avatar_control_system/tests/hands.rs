use super::*;

#[test]
fn avatar_supplies_solved_hand_grip_without_moving_tracking_or_pointer() {
    use crate::engine::ecs::component::PointerComponent;
    let mut world = World::default();
    let controller = world.add_component(TransformComponent::new());
    let tracked = world.add_component(TransformComponent::new().with_position(3., 1., 0.));
    let hand = world.add_component(TransformComponent::new().with_position(1., 1., 0.));
    let pointer = world.add_component(PointerComponent::new());
    world.add_child(controller, tracked).unwrap();
    world.add_child(tracked, pointer).unwrap();
    let correction = [0., 0.70710677, 0., 0.70710677];
    let mut queue = crate::engine::ecs::CommandQueue::new();
    ensure_hand_grab_attachment(&mut world, controller, hand, Some(correction), &mut queue);
    let grip = world
        .get_component_by_id_as::<PointerComponent>(pointer)
        .unwrap()
        .avatar_grab_attachment
        .unwrap();
    assert_eq!(world.parent_of(grip), Some(hand));
    assert_eq!(world.parent_of(pointer), Some(tracked));
    assert_eq!(
        world
            .get_component_by_id_as::<TransformComponent>(grip)
            .unwrap()
            .transform
            .rotation,
        quat_conjugate(correction)
    );
    ensure_hand_grab_attachment(&mut world, controller, hand, Some(correction), &mut queue);
    assert_eq!(
        world
            .get_component_by_id_as::<PointerComponent>(pointer)
            .unwrap()
            .avatar_grab_attachment,
        Some(grip)
    );
}
#[test]
fn automatic_source_switches_apply_absolute_non_accumulating_corrections() {
    let mut world = World::default();
    let avc_id = world.add_component(AvatarControlComponent::new());
    let controller_id = world.add_component(ControllerXRComponent::new(
        true,
        ControllerHand::Left,
        crate::engine::ecs::component::ControllerPoseKind::GripAim,
    ));
    let raw = world.add_component(TransformComponent::new());
    let visual = world.add_component(TransformComponent::new());
    world.add_child(avc_id, controller_id).unwrap();
    world.add_child(controller_id, raw).unwrap();
    world.add_child(raw, visual).unwrap();
    let correction = [0.0, 0.70710677, 0.0, 0.70710677];
    {
        let avc = world
            .get_component_by_id_as_mut::<AvatarControlComponent>(avc_id)
            .unwrap();
        avc.left_hand_raw_target_id = Some(raw);
        avc.left_hand_visual_target_id = Some(visual);
        avc.left_hand_aim_correction = Some(correction);
    }
    let mut emitted = RecordingEmitter::default();
    for (source, expected_rotation) in [
        (ControllerPoseSource::ControllerGripAim, correction),
        (ControllerPoseSource::WristPalm, correction),
        (ControllerPoseSource::ControllerAim, correction),
        (ControllerPoseSource::ControllerGripAim, correction),
        (ControllerPoseSource::None, [0.0, 0.0, 0.0, 1.0]),
        (ControllerPoseSource::WristPalm, correction),
    ] {
        world
            .get_component_by_id_as_mut::<ControllerXRComponent>(controller_id)
            .unwrap()
            .active_pose_source = source;
        update_hand_pose_corrections(avc_id, &world, &mut emitted, false);
        let transform = world
            .get_component_by_id_as_mut::<TransformComponent>(visual)
            .unwrap();
        transform.transform.rotation = expected_rotation;
        transform.transform.recompute_model();
    }
    assert_eq!(
        emitted.0,
        vec![correction, [0.0, 0.0, 0.0, 1.0], correction,]
    );
}
