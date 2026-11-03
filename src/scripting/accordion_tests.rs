// Exercise the shared prefab through the retained runtime used by scenes.
use super::runner::RuntimeSpecSession;
use crate::engine::ecs::component::TransformComponent;
use crate::engine::ecs::{
    CommandQueue, ComponentId, EventSignal, IntentValue, RxWorld, Signal, SignalEmitter, World,
};
use crate::engine::graphics::RenderAssets;

#[test]
fn accordion_factory_removes_body_and_emits_one_way_restore_request() {
    let src = r##"
        import { accordion, accordion_body } from "../assets/components/internal/ui/accordion.mms"

        let panel = accordion({
            root_name = "test_accordion"
            width_gu = 24.0
            unit_scale = 1.0
            background_color = [0.0, 0.1, 0.3, 1.0]
            children = [T { name = "test_title" Text { "test" } }]
            body = accordion_body(T { Text { "body" } })
        })
        panel
    "##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();
    let mut assets = RenderAssets::new();
    let (mut session, out) = RuntimeSpecSession::start_at_path(
        src,
        "examples/accordion-test.mms",
        &mut world,
        &mut rx,
        Some(&mut assets),
        &mut emit,
    )
    .unwrap();
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    let panel = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("test_accordion"))
        .expect("test accordion root");
    let layout_slot = world.parent_of(panel).expect("accordion layout slot");
    assert_eq!(
        world.component_label(layout_slot),
        Some("accordion_layout_slot")
    );
    let title_bar = world
        .find_component(panel, "#title_bar")
        .expect("accordion title bar");
    let toggle = world
        .find_component(panel, "#accordion_toggle")
        .expect("accordion toggle");
    let title = world
        .find_component(panel, "#test_title")
        .expect("caller-authored title child");
    let title_children = world.children_of(title_bar);
    assert!(
        title_children.iter().position(|id| *id == toggle)
            < title_children.iter().position(|id| *id == title),
        "the built-in toggle must precede caller title children"
    );
    let draggable = title_children
        .iter()
        .find_map(|id| {
            world.get_component_by_id_as::<crate::engine::ecs::component::DraggableComponent>(*id)
        })
        .expect("title bar draggable marker");
    assert!(matches!(
        &draggable.target,
        crate::engine::ecs::component::DraggableTarget::Explicit(
            crate::engine::ecs::component::ComponentRef::Query(selector)
        ) if selector == "../../#test_accordion"
    ));
    let toggle_icon = world
        .find_component(panel, "#accordion_toggle_icon")
        .expect("accordion toggle icon");
    assert_eq!(
        world
            .get_component_by_id_as::<TransformComponent>(toggle_icon)
            .expect("accordion toggle icon transform")
            .transform
            .rotation,
        [0.0, 0.0, 0.0, 1.0],
        "expanded accordion glyph starts down-facing"
    );
    let mount = world
        .find_component(panel, "#accordion_body_mount")
        .expect("accordion body mount");
    let body = world
        .find_component(panel, "#accordion_body")
        .expect("initial accordion body");
    let click = || EventSignal::Click {
        raycaster: ComponentId::default(),
        renderable: toggle,
        hit_point: [0.0, 0.0, 0.0],
        screen_pos_px: None,
    };

    rx.dispatch_event_handlers(&mut world, &Signal::event(toggle, click()));
    let out = session.service_callbacks(&mut world, &mut rx, Some(&mut assets), &mut emit);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    for intent in out.intents {
        rx.push_intent_now(ComponentId::default(), intent);
    }
    emit.drain_into_rx(&mut rx);
    let close_intents = rx.drain_ready_intents();
    assert!(close_intents.iter().any(|signal| matches!(
        signal.intent.as_ref().map(|intent| &intent.value),
        Some(IntentValue::UpdateTransform { component_id, rotation_quat_xyzw, .. })
            if *component_id == toggle_icon
                && (rotation_quat_xyzw[2] + std::f32::consts::FRAC_1_SQRT_2).abs() < 1.0e-4
                && (rotation_quat_xyzw[3] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1.0e-4
    )));
    assert!(close_intents.iter().any(|signal| matches!(
        signal.intent.as_ref().map(|intent| &intent.value),
        Some(IntentValue::RemoveSubtree { component_id }) if *component_id == body
    )));
    world
        .remove_component_subtree(body)
        .expect("apply body removal for restore half of test");

    rx.begin_frame();
    let close_events = rx.drain_ready_events();
    assert!(close_events.iter().any(|signal| matches!(
        signal.event.as_ref(),
        Some(EventSignal::DataEvent { name, payload })
            if name == "AccordionMinimized" && *payload == Some(mount)
    )));

    rx.dispatch_event_handlers(&mut world, &Signal::event(toggle, click()));
    let out = session.service_callbacks(&mut world, &mut rx, Some(&mut assets), &mut emit);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    for intent in out.intents {
        rx.push_intent_now(ComponentId::default(), intent);
    }
    emit.drain_into_rx(&mut rx);
    let open_intents = rx.drain_ready_intents();
    assert!(
        open_intents.iter().any(|signal| matches!(
            signal.intent.as_ref().map(|intent| &intent.value),
            Some(IntentValue::UpdateTransform { component_id, rotation_quat_xyzw, .. })
                if *component_id == toggle_icon
                    && rotation_quat_xyzw[2].abs() < 1.0e-4
                    && (rotation_quat_xyzw[3] - 1.0).abs() < 1.0e-4
        )),
        "opening must update its own disclosure affordance: {open_intents:?}"
    );
    assert!(
        open_intents.iter().all(|signal| !matches!(
            signal.intent.as_ref().map(|intent| &intent.value),
            Some(IntentValue::RemoveSubtree { .. } | IntentValue::Attach { .. })
        )),
        "opening must not mutate or recreate the body: {open_intents:?}"
    );

    rx.begin_frame();
    let open_events = rx.drain_ready_events();
    assert!(open_events.iter().any(|signal| matches!(
        signal.event.as_ref(),
        Some(EventSignal::DataEvent { name, payload })
            if name == "AccordionRestoreRequested" && *payload == Some(mount)
    )));
    assert!(
        world.find_component(panel, "#accordion_body").is_none(),
        "accordion must not listen for or perform restoration itself"
    );
}
