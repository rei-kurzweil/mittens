use mittens_engine::engine::{
    Universe,
    ecs::{ComponentId, EventSignal, SignalEmitter, World},
};

fn named(world: &World, label: &str) -> ComponentId {
    world
        .all_components()
        .find(|&id| world.component_label(id) == Some(label))
        .unwrap()
}
fn count(world: &World, label: &str) -> usize {
    world
        .all_components()
        .filter(|&id| world.component_label(id) == Some(label))
        .count()
}
fn command(scene: &mut Universe, label: &str) {
    eprintln!("orchard command: {label}");
    scene.command_queue.push_event(
        named(&scene.world, "orchard_test"),
        EventSignal::DataEvent {
            name: label.into(),
            payload: None,
        },
    );
    for _ in 0..3 {
        scene.update(0.02, &Default::default());
    }
}

#[test]
fn orchard_halves_unload_and_rebuild_without_removing_the_ground() {
    let mut scene = Universe::new(World::default());
    scene
        .systems
        .xr
        .set_external_frame(Some(Default::default()));
    scene
        .load_mms_source_at_path(
            r##"
        import { ship_park } from "../assets/components/rooms/ship_park.mms"
        ship_park()
        let root=T {name="orchard_test"}
        root
        on(root,"DataEvent",fn(event) {
            if event=="HideNorth" {query("#ship_orchard_north").set_expanded(false)}
            if event=="HideSouth" {query("#ship_orchard_south").set_expanded(false)}
            if event=="ShowNorth" {query("#ship_orchard_north").set_expanded(true)}
            if event=="ShowSouth" {query("#ship_orchard_south").set_expanded(true)}
        })
    "##,
            "examples/_ship_orchard_test.mms",
        )
        .unwrap();
    let ground = named(&scene.world, "ship_park_ground");
    assert_eq!(count(&scene.world, "ship_orchard_tree"), 20);
    for _ in 0..2 {
        command(&mut scene, "HideNorth");
        assert_eq!(count(&scene.world, "ship_orchard_tree"), 10);
        command(&mut scene, "HideSouth");
        assert_eq!(count(&scene.world, "ship_orchard_tree"), 0);
        assert_eq!(count(&scene.world, "sakura_tree_branches"), 0);
        assert!(scene.world.get_component_record(ground).is_some());
        assert_eq!(count(&scene.world, "ship_park_glass_dome"), 1);
        command(&mut scene, "ShowNorth");
        assert_eq!(count(&scene.world, "ship_orchard_tree"), 10);
        command(&mut scene, "ShowSouth");
        assert_eq!(count(&scene.world, "ship_orchard_tree"), 20);
        command(&mut scene, "ShowSouth");
        assert_eq!(count(&scene.world, "ship_orchard_tree"), 20);
    }
}
