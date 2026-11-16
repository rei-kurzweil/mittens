use super::*;

#[test]
fn arm_ik_eligibility_uses_the_xr_hand_direct_tracked_transform() {
    let mut world = World::default();
    let upper = world.add_component(TransformComponent::new());
    let lower = world.add_component(TransformComponent::new());
    let hand = world.add_component(TransformComponent::new());
    let report = left_arm_report(upper, lower, hand);

    let controller = world.add_component(ControllerXRComponent::new(
        true,
        ControllerHand::Left,
        crate::engine::ecs::component::ControllerPoseKind::GripAim,
    ));
    let tracked = world.add_component(TransformComponent::new());
    let attachment = world.add_component(RestAttachmentComponent::new(
        ComponentRef::Query("#hand".into()),
        ComponentRef::Query("#tip".into()),
    ));
    let pointer = world.add_component(crate::engine::ecs::component::PointerComponent::new());
    world.add_child(controller, tracked).unwrap();
    world.add_child(tracked, attachment).unwrap();
    world.add_child(attachment, pointer).unwrap();

    let eligibility = classify_arm_ik_eligibility(&world, &report, Some(controller), true);
    let ArmIkEligibility::Eligible(binding) = eligibility else {
        panic!("expected eligible arm, got {eligibility:?}");
    };
    assert_eq!(binding.raw_target, tracked);
    assert_eq!(binding.hand, hand);
    assert_eq!(binding.upper_arm, upper);
    assert_eq!(binding.lower_arm, lower);
    assert_eq!(world.parent_of(tracked), Some(controller));
}

#[test]
fn complete_arm_without_hand_driver_is_not_eligible() {
    let mut world = World::default();
    let upper = world.add_component(TransformComponent::new());
    let lower = world.add_component(TransformComponent::new());
    let hand = world.add_component(TransformComponent::new());
    let report = left_arm_report(upper, lower, hand);

    assert_eq!(
        classify_arm_ik_eligibility(&world, &report, None, true),
        ArmIkEligibility::NoHandDriver
    );
}

#[test]
fn missing_direct_tracked_transform_is_malformed_not_an_origin_target() {
    let mut world = World::default();
    let upper = world.add_component(TransformComponent::new());
    let lower = world.add_component(TransformComponent::new());
    let hand = world.add_component(TransformComponent::new());
    let report = left_arm_report(upper, lower, hand);
    let controller = world.add_component(ControllerXRComponent::new(
        true,
        ControllerHand::Left,
        crate::engine::ecs::component::ControllerPoseKind::GripAim,
    ));
    let wrapper = world.add_component(RestAttachmentComponent::new(
        ComponentRef::Query("#hand".into()),
        ComponentRef::Query("#tip".into()),
    ));
    let nested = world.add_component(TransformComponent::new());
    world.add_child(controller, wrapper).unwrap();
    world.add_child(wrapper, nested).unwrap();

    assert_eq!(
        classify_arm_ik_eligibility(&world, &report, Some(controller), true),
        ArmIkEligibility::MalformedHandDriver
    );
}
