use mittens_engine::engine::{
    Universe,
    ecs::{ComponentId, EventSignal, SignalEmitter, SignalKind, component::TextComponent},
};
fn named(scene: &Universe, name: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(name))
        .unwrap()
}
fn tick(scene: &mut Universe, frames: usize) {
    for _ in 0..frames {
        scene.update(1. / 75., &Default::default());
    }
}
fn send(scene: &mut Universe, name: &str) {
    scene.command_queue.push_event(
        named(scene, "controls"),
        EventSignal::DataEvent {
            name: name.into(),
            payload: None,
        },
    );
    tick(scene, 1);
}
fn count(scene: &Universe) -> u32 {
    scene
        .world
        .get_component_by_id_as::<TextComponent>(named(scene, "count"))
        .unwrap()
        .text
        .parse()
        .unwrap()
}
#[test]
fn native_bypass_stops_frame_generation_and_removal_releases_the_route() {
    let mut scene = Universe::new(Default::default());
    scene.load_mms_source_at_path(r#"
  let output=Text {name="count" "0"}
  let state={calls=0}
  let frames=on(world,"FrameTick",fn(event){state.calls=state.calls+1 output.set_text(""+state.calls)})
  frames.bypass(true)
  let controls=T {name="controls"}
  on(controls,"DataEvent",fn(event) {
   if event=="resume" {frames.bypass(false)}
   if event=="pause" {frames.bypass(true)}
   if event=="remove" {frames.remove() frames.remove()}
  })
 "#,"examples/_event_binding_lifecycle.mms").unwrap();
    assert!(!scene.systems.rx.has_global_handlers(SignalKind::FrameTick));
    tick(&mut scene, 8);
    assert_eq!(count(&scene), 0);
    for _ in 0..4 {
        send(&mut scene, "resume");
        assert!(scene.systems.rx.has_global_handlers(SignalKind::FrameTick));
        let before = count(&scene);
        tick(&mut scene, 5);
        assert_eq!(
            count(&scene),
            before + 5,
            "one retained binding, no duplicates"
        );
        send(&mut scene, "pause");
        assert!(!scene.systems.rx.has_global_handlers(SignalKind::FrameTick));
        let before = count(&scene);
        tick(&mut scene, 8);
        assert_eq!(count(&scene), before);
    }
    send(&mut scene, "resume");
    tick(&mut scene, 2);
    send(&mut scene, "remove");
    assert!(!scene.systems.rx.has_global_handlers(SignalKind::FrameTick));
    let before = count(&scene);
    tick(&mut scene, 8);
    assert_eq!(count(&scene), before);
}
#[test]
fn scoped_bindings_keep_ancestry_routing_and_can_be_removed_independently() {
    let mut scene = Universe::new(Default::default());
    scene
        .load_mms_source_at_path(
            r#"
  let output=Text {name="count" "0"}
  let root=T {name="controls" T {name="child"}}
  let scoped=on(root,"DataEvent",fn(event){if event=="hit" {output.set_text("7")}})
  on(world,"DataEvent",fn(event){if event=="remove" {scoped.remove()}})
 "#,
            "examples/_event_binding_scope.mms",
        )
        .unwrap();
    let child = named(&scene, "child");
    scene.command_queue.push_event(
        child,
        EventSignal::DataEvent {
            name: "hit".into(),
            payload: None,
        },
    );
    tick(&mut scene, 2);
    assert_eq!(count(&scene), 7);
    send(&mut scene, "remove");
    // A retained resource no longer dispatches after explicit removal.
    scene
        .world
        .get_component_by_id_as_mut::<TextComponent>(named(&scene, "count"))
        .unwrap()
        .text = "0".into();
    scene.command_queue.push_event(
        child,
        EventSignal::DataEvent {
            name: "hit".into(),
            payload: None,
        },
    );
    tick(&mut scene, 2);
    assert_eq!(count(&scene), 0);
}

#[test]
fn closing_a_session_retires_its_frame_generation_request() {
    let mut scene = Universe::new(Default::default());
    scene
        .load_mms_source_at_path(
            "on(world, \"FrameTick\", fn(event) {})",
            "examples/_event_binding_close.mms",
        )
        .unwrap();
    assert!(scene.systems.rx.has_global_handlers(SignalKind::FrameTick));
    scene.close_mms_session();
    assert!(!scene.systems.rx.has_global_handlers(SignalKind::FrameTick));
    tick(&mut scene, 4);
}
