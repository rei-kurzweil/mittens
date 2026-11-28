use super::runner::RuntimeSpecSession;
use crate::engine::ecs::component::SecondaryMotionComponent;
use crate::engine::ecs::{CommandQueue, EventSignal, KeyboardEvent, RxWorld, Signal, World};

#[test]
fn secondary_motion_pause_is_scoped_to_the_selected_component() {
    let (mut world, mut rx, mut queue) = (World::default(), RxWorld::default(), CommandQueue::new());
    let (mut session, output) = RuntimeSpecSession::start_at_path(
        r#"
        let actor = T { name = "actor" SecondaryMotion { name = "first" } SecondaryMotion { name = "second" } }
        let other = SecondaryMotion { name = "other" }
        actor other
        on_global("KeyDown", fn(event) {
            if event.code == "Pause" { for motion in actor.query_all("secondary_motion") { motion.set_paused(true) } }
            if event.code == "Resume" { for motion in actor.query_all("secondary_motion") { motion.set_paused(false) } }
        })
        "#,
        "examples/_secondary_motion_pause_test.mms", &mut world, &mut rx, None, &mut queue,
    ).unwrap();
    assert!(output.errors.is_empty(), "{:?}", output.errors);
    let named = |w: &World, name: &str| w.all_components().find(|&id| w.component_label(id) == Some(name)).unwrap();
    let ids = [named(&world, "first"), named(&world, "second"), named(&world, "other")];
    for (code, paused) in [("Pause", true), ("Resume", false)] {
        rx.dispatch_event_handlers(&mut world, &Signal::event(ids[0], EventSignal::KeyDown(KeyboardEvent {
            code: Some(code.into()), key: code.into(),
        })));
        let output = session.service_callbacks(&mut world, &mut rx, None, &mut queue);
        assert!(output.errors.is_empty(), "{:?}", output.errors);
        for id in &ids[..2] { assert_eq!(world.get_component_by_id_as::<SecondaryMotionComponent>(*id).unwrap().paused, paused); }
        assert!(!world.get_component_by_id_as::<SecondaryMotionComponent>(ids[2]).unwrap().paused);
    }
}
