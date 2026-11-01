use crate::engine::ecs::component::{Component, InputXRGamepadComponent};
use crate::engine::ecs::{CommandQueue, RxWorld, World};
#[test]
fn xr_flying_builder_is_optional_typed_and_roundtrips_without_runtime_state() {
    let (mut world, mut rx, mut queue) =
        (World::default(), RxWorld::default(), CommandQueue::new());
    super::tests::start_settings_scene(
        "InputXRGamepad.enable_flying(true) { speed(1.4) } InputXRGamepad {}",
        "examples/_flying_test.mms",
        &mut world,
        &mut rx,
        None,
        &mut queue,
    )
    .unwrap();
    let inputs: Vec<_> = world
        .all_components()
        .filter_map(|id| world.get_component_by_id_as::<InputXRGamepadComponent>(id))
        .collect();
    assert_eq!(inputs.len(), 2);
    assert_eq!(inputs.iter().filter(|c| c.enable_flying).count(), 1);
    assert!(inputs.iter().all(|c| !c.flying));
    let mut component = InputXRGamepadComponent::new().enable_flying(true);
    component.flying = true;
    let text = super::unparser::unparse_component(&component.to_mms_ast(&world));
    assert!(text.contains("enable_flying(true)"));
    assert!(!text.contains(".flying("));
    let (mut world, mut rx, mut queue) =
        (World::default(), RxWorld::default(), CommandQueue::new());
    assert!(
        super::tests::start_settings_scene(
            "InputXRGamepad { enable_flying(\"yes\") }",
            "examples/_flying_test.mms",
            &mut world,
            &mut rx,
            None,
            &mut queue
        )
        .is_err()
    );
}
