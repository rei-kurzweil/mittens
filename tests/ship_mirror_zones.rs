use mittens_engine::engine::{
    Universe,
    ecs::{ComponentId, IntentValue, SignalEmitter, World, component::MirrorComponent},
};

fn named(world: &World, label: &str) -> ComponentId {
    world
        .all_components()
        .find(|&id| world.component_label(id) == Some(label))
        .unwrap()
}
fn move_to(scene: &mut Universe, label: &str, z: f32) {
    let id = named(&scene.world, label);
    scene.command_queue.push_intent_now(
        id,
        IntentValue::UpdateTransform {
            component_id: id,
            translation: [-18., 6.45, z],
            rotation_quat_xyzw: [0., 0., 0., 1.],
            scale: [1.; 3],
        },
    );
    for _ in 0..4 {
        scene.update(0.02, &Default::default());
    }
}
fn enabled(scene: &Universe, label: &str) -> bool {
    scene
        .world
        .get_component_by_id_as::<MirrorComponent>(named(&scene.world, label))
        .unwrap()
        .enabled
}

#[test]
fn washroom_doorway_zones_toggle_only_their_mirror_for_each_player_rig() {
    let mut scene = Universe::new(World::default());
    scene
        .systems
        .xr
        .set_external_frame(Some(Default::default()));
    scene.load_mms_source_at_path(r##"
        import {ship_washroom_mirror_zones} from "../assets/components/rooms/ship_washroom_mirror_zones.mms"
        T {R.plane() {Mirror.quality(512).enabled(false) {name="ship_womens_washroom_mirror"}}}
        T.position(100,0,0) {R.plane() {Mirror {name="other_mirror"}}}
        ship_washroom_mirror_zones()
        for label in ["ship_player_root","ship_xr_free_rig","ship_desktop_camera_rig","unrelated_npc"] {
            T.position(-18,6.45,15.3) {
                name=label
                Zone.sphere(0.2) {Collidable.slide().movement_target("/#"+label) {}}
            }
        }
    "##,"examples/_ship_mirror_zone_test.mms").unwrap();
    assert!(!enabled(&scene, "ship_womens_washroom_mirror"));
    assert!(enabled(&scene, "other_mirror"));
    for label in [
        "ship_player_root",
        "ship_xr_free_rig",
        "ship_desktop_camera_rig",
    ] {
        move_to(&mut scene, label, 12.8); // before the door at Z=12
        assert!(enabled(&scene, "ship_womens_washroom_mirror"));
        move_to(&mut scene, label, 11.0); // leave the activation zone into the room
        assert!(enabled(&scene, "ship_womens_washroom_mirror"));
        move_to(&mut scene, "unrelated_npc", 14.1);
        assert!(
            enabled(&scene, "ship_womens_washroom_mirror"),
            "NPCs cannot freeze a player's mirror"
        );
        move_to(&mut scene, "unrelated_npc", 15.3);
        move_to(&mut scene, label, 12.8);
        move_to(&mut scene, label, 14.1); // outer exit sensor
        assert!(!enabled(&scene, "ship_womens_washroom_mirror"));
        assert!(enabled(&scene, "other_mirror"));
        move_to(&mut scene, label, 15.3);
    }
}
