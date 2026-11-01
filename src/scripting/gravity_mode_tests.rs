use super::runner::RuntimeSpecSession;
use crate::engine::ecs::component::{Component, GravityComponent, GravityMode};
use crate::engine::ecs::{CommandQueue, EventSignal, KeyboardEvent, RxWorld, Signal, World};
#[test]
fn gravity_modes_have_typed_constructors_builders_and_live_methods() {
    let (mut world, mut rx, mut queue) =
        (World::default(), RxWorld::default(), CommandQueue::new());
    let (mut session, output) = RuntimeSpecSession::start_at_path(
        r#"
        let local = Gravity.auto().coefficient(0.35) { name="local_gravity" }
        local
        Gravity.on() { name="on_gravity" }
        Gravity.off() { name="off_gravity" }
        Gravity { auto() name="body_auto_gravity" }
        on_global("KeyDown",fn(event){
            if event.code=="Off" { local.off() }
            if event.code=="Auto" { query("[name='local_gravity']").auto() }
            if event.code=="On" { local.on() }
        })
    "#,
        "examples/_gravity_modes_test.mms",
        &mut world,
        &mut rx,
        None,
        &mut queue,
    )
    .unwrap();
    assert!(output.errors.is_empty(), "{:?}", output.errors);
    let named = |w: &World, name: &str| {
        w.all_components()
            .find(|&id| w.component_label(id) == Some(name))
            .unwrap()
    };
    for (name, mode) in [
        ("local_gravity", GravityMode::Auto),
        ("on_gravity", GravityMode::On),
        ("off_gravity", GravityMode::Off),
        ("body_auto_gravity", GravityMode::Auto),
    ] {
        assert_eq!(
            world
                .get_component_by_id_as::<GravityComponent>(named(&world, name))
                .unwrap()
                .mode(),
            mode
        );
    }
    let local = named(&world, "local_gravity");
    for (code, mode) in [
        ("Off", GravityMode::Off),
        ("Auto", GravityMode::Auto),
        ("On", GravityMode::On),
        ("Auto", GravityMode::Auto),
    ] {
        rx.dispatch_event_handlers(
            &mut world,
            &Signal::event(
                local,
                EventSignal::KeyDown(KeyboardEvent {
                    code: Some(code.into()),
                    key: code.into(),
                }),
            ),
        );
        let output = session.service_callbacks(&mut world, &mut rx, None, &mut queue);
        assert!(output.errors.is_empty(), "{:?}", output.errors);
        let component = world
            .get_component_by_id_as::<GravityComponent>(local)
            .unwrap();
        assert_eq!(component.mode(), mode);
        assert!((component.coefficient - 0.35).abs() < 1e-6);
    }
    let serialized = super::unparser::unparse_component(
        &world
            .get_component_by_id_as::<GravityComponent>(local)
            .unwrap()
            .to_mms_ast(&world),
    );
    assert!(serialized.contains("auto()"));
    let (mut copy, mut rx, mut queue) = (World::default(), RxWorld::default(), CommandQueue::new());
    super::tests::start_settings_scene(
        &serialized,
        "examples/_gravity_roundtrip_test.mms",
        &mut copy,
        &mut rx,
        None,
        &mut queue,
    )
    .unwrap();
    assert!(
        copy.all_components()
            .filter_map(|id| copy.get_component_by_id_as::<GravityComponent>(id))
            .all(|c| c.mode() == GravityMode::Auto)
    );
}
#[test]
fn gravity_mode_methods_reject_arguments() {
    for source in [
        "Gravity.auto(true)",
        "Gravity.off(0)",
        "let g=Gravity.auto() g g.auto(true)",
    ] {
        let (mut w, mut rx, mut q) = (World::default(), RxWorld::default(), CommandQueue::new());
        assert!(
            super::tests::start_settings_scene(
                source,
                "examples/_invalid_gravity_mode.mms",
                &mut w,
                &mut rx,
                None,
                &mut q
            )
            .is_err(),
            "{source}"
        );
    }
}
