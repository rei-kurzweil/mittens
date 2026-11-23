//! The sakura twilight fork of the rooftop example loads without its train.
use crate::engine::Universe;
use crate::engine::ecs::component::{
    Camera3DComponent, CameraXRComponent, DestroyableComponent, MirrorComponent, TextComponent,
    TransformComponent, VelocityComponent,
};
use crate::engine::ecs::system::{TransformSystem, grabbable_system::grabbable_owner_for_hit};
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, SignalEmitter, World};

fn named(scene: &Universe, label: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(label))
        .unwrap_or_else(|| panic!("missing {label}"))
}

#[test]
fn sakura_twilight_loads_its_city_without_the_train() {
    let path = "examples/sakura-twilight.mms";
    let original = std::fs::read_to_string(path).unwrap();
    let mut source = original
        .replace("let use_xr = true", "let use_xr = false")
        .replace("let use_desktop = false", "let use_desktop = true");
    source.push_str(
        r##"
        let ramp_test = T { name = "ramp_test_driver" }
        on(ramp_test, "DataEvent", fn(event) {
            let truck = pickup.query("#pickup_truck")
            if event == "DriveRamp" {
                truck.set_powered(true)
                truck.drive_command(1,0,false)
            }
            if event == "TurnLeft" { truck.drive_command(1,-0.63,false) }
        })
    "##,
    );
    let mut scene = Universe::new(World::default());
    scene.load_mms_source_at_path(&source, path).unwrap();
    for _ in 0..3 {
        scene.update(1.0 / 60.0, &Default::default());
    }
    let labels: Vec<&str> = scene
        .world
        .all_components()
        .filter_map(|id| scene.world.component_label(id))
        .collect();
    assert!(
        !labels.iter().any(|label| label.contains("train")),
        "no train parts"
    );
    for name in [
        "sakura_twilight_main",
        "sakura_twilight_row_0",
        "sakura_twilight_row_1",
        "sakura_twilight_row_2",
        "sakura_twilight_row_3",
        "sakura_twilight_road_dashes",
        "sakura_twilight_ground",
        "sakura_twilight_pickup",
        "sakura_twilight_road_ramp",
        "sakura_twilight_ramp_landing",
        "sakura_twilight_road_turn",
        "sakura_twilight_avatar_settings",
        "avatar_height_slider",
        "ship_washroom_suite",
        "ship_womens_washroom",
        "ship_washroom_passage",
        "sakura_twilight_row_sta_launcher_grip",
        "sakura_twilight_row_sta_ammo_cabinet",
        "kawaii_twilight_sky",
    ] {
        assert_eq!(
            labels.iter().filter(|&&label| label == name).count(),
            1,
            "{name}"
        );
    }
    for (rack, indices) in [
        ("sakura_twilight_main_pistol_rack", 1..=6),
        ("sakura_twilight_row_pistol_rack", 7..=12),
    ] {
        let rack = named(&scene, rack);
        for index in indices {
            let label = if index == 1 {
                "rooftop_machine_pistol".to_owned()
            } else {
                format!("rooftop_machine_pistol_{index}")
            };
            assert!(
                scene
                    .world
                    .find_component(rack, &format!("#{label}_grip"))
                    .is_some()
            );
            let ammo = named(&scene, &format!("{label}_ammo_count"));
            assert_eq!(
                scene
                    .world
                    .get_component_by_id_as::<TextComponent>(ammo)
                    .unwrap()
                    .text,
                "96"
            );
        }
    }
    assert!(scene.world.all_components().any(|id| {
        scene
            .world
            .get_component_by_id_as::<Camera3DComponent>(id)
            .is_some_and(|camera| camera.z_far == 250.0)
    }));
    for (building, floors) in [
        ("main", 3),
        ("row_0", 2),
        ("row_1", 2),
        ("row_2", 2),
        ("row_3", 1),
    ] {
        let sections = (0..floors)
            .map(|floor| format!("floor_{floor}"))
            .chain(["roof".into()]);
        for section in sections {
            let section = named(&scene, &format!("sakura_twilight_{building}_{section}"));
            assert!(scene.world.children_of(section).iter().any(|&child| {
                scene
                    .world
                    .get_component_by_id_as::<DestroyableComponent>(child)
                    .is_some()
            }));
        }
    }
    let driver = named(&scene, "ramp_test_driver");
    scene.command_queue.push_event(
        driver,
        EventSignal::DataEvent {
            name: "DriveRamp".into(),
            payload: None,
        },
    );
    let pickup = named(&scene, "pickup_truck");
    let mut position = [0.0; 3];
    for _ in 0..690 {
        scene.update(1.0 / 60.0, &Default::default());
        position = TransformSystem::world_position(&scene.world, pickup).unwrap();
        if position[0] >= 33.5 && position[1] < 0.1 {
            break;
        }
    }
    assert!(
        position[0] >= 33.5 && (position[1] + 0.23).abs() < 0.2 && (position[2] + 0.4).abs() < 1.27,
        "pickup should reach the level landing from its authored roof spawn: {position:?}"
    );
    scene.command_queue.push_event(
        driver,
        EventSignal::DataEvent {
            name: "TurnLeft".into(),
            payload: None,
        },
    );
    for _ in 0..260 {
        scene.update(1.0 / 60.0, &Default::default());
        position = TransformSystem::world_position(&scene.world, pickup).unwrap();
        if position[2] < -9.0 {
            break;
        }
    }
    assert!(
        (36.0..47.0).contains(&position[0])
            && position[2] < -9.0
            && (position[1] + 0.23).abs() < 0.2,
        "pickup should steer left from the landing onto the main road: {position:?}"
    );

    check_mirror_controls(&mut scene);
    drop(scene);
    let mut xr = Universe::new(World::default());
    xr.systems.xr.set_external_frame(Some(Default::default()));
    xr.load_mms_source_at_path(&original, path).unwrap();
    for _ in 0..3 {
        xr.update(1.0 / 60.0, &Default::default());
    }
    named(&xr, "sakura_twilight_avatar_settings");
    named(&xr, "avatar_height_slider");
    named(&xr, "xr_floor_correction_slider");
    assert!(xr.world.all_components().any(|id| {
        xr.world
            .get_component_by_id_as::<CameraXRComponent>(id)
            .is_some_and(|camera| camera.z_far == 250.0)
    }));
}

fn check_mirror_controls(scene: &mut Universe) {
    fn enabled(scene: &Universe, mirror: ComponentId) -> bool {
        scene
            .world
            .get_component_by_id_as::<MirrorComponent>(mirror)
            .unwrap()
            .enabled
    }
    fn advance(scene: &mut Universe) {
        for _ in 0..4 {
            scene.update(1.0 / 60.0, &Default::default());
        }
    }
    fn click(scene: &mut Universe, button: ComponentId) {
        scene.command_queue.push_event(
            button,
            EventSignal::Click {
                raycaster: button,
                renderable: button,
                hit_point: [0.0; 3],
                screen_pos_px: None,
            },
        );
        advance(scene);
    }
    fn move_player(scene: &mut Universe, player: ComponentId, translation: [f32; 3]) {
        // These waypoints place the rig directly. Discard the preceding physics
        // sweep so the move from the roof cannot catch on the bathroom roof.
        scene
            .systems
            .static_contact
            .forget_target(&scene.world, player);
        scene.command_queue.push_intent_now(
            player,
            IntentValue::UpdateTransform {
                component_id: player,
                translation,
                rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0; 3],
            },
        );
        advance(scene);
        let position = TransformSystem::world_position(&scene.world, player).unwrap();
        assert!(
            (position[1] - translation[1]).abs() < 0.3,
            "player should be at the doorway's floor level: {position:?}"
        );
    }
    fn readout(scene: &Universe) -> &str {
        let label = named(scene, "sakura_twilight_mirror_toggle_label");
        &scene
            .world
            .get_component_by_id_as::<TextComponent>(label)
            .unwrap()
            .text
    }

    let frame = named(scene, "sakura_twilight_mirror_frame");
    let rooftop = named(scene, "sakura_twilight_mirror");
    let bathroom = named(scene, "ship_womens_washroom_mirror");
    let button = named(scene, "sakura_twilight_mirror_toggle");
    let title = scene.world.find_component(frame, "#title_bar").unwrap();
    assert_eq!(grabbable_owner_for_hit(&scene.world, title), Some(frame));
    assert_eq!(grabbable_owner_for_hit(&scene.world, rooftop), Some(frame));
    assert_eq!(grabbable_owner_for_hit(&scene.world, button), None);
    assert!(!enabled(scene, rooftop) && !enabled(scene, bathroom));
    assert_eq!(readout(scene), "Off");

    click(scene, button);
    assert!(enabled(scene, rooftop) && !enabled(scene, bathroom));
    assert_eq!(readout(scene), "On");

    // Enter from the street, turn into the bathroom, then return by the same route.
    let player = named(scene, "local_root");
    let front_door = [29.4, -0.07, -22.8];
    let hallway = [21.0, 0.015, -22.8];
    let bathroom_door = [21.0, 0.015, -25.0];
    move_player(scene, player, front_door);
    move_player(scene, player, hallway);
    assert!(!enabled(scene, bathroom));
    move_player(scene, player, bathroom_door);
    assert!(enabled(scene, bathroom) && enabled(scene, rooftop));
    click(scene, button);
    assert!(!enabled(scene, rooftop) && enabled(scene, bathroom));
    assert_eq!(readout(scene), "Off");
    move_player(scene, player, hallway);
    assert!(enabled(scene, bathroom));
    move_player(scene, player, front_door);
    assert!(!enabled(scene, bathroom) && !enabled(scene, rooftop));
    move_player(scene, player, hallway);
    assert!(!enabled(scene, bathroom));
    move_player(scene, player, bathroom_door);
    assert!(enabled(scene, bathroom));
}

#[test]
fn sakura_twilight_pickup_coasts_and_follows_an_incline() {
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(
            r##"
        import { pickup_truck } from "../assets/components/vehicles/pickup_truck.mms"
        let motion = pickup_truck(null, null, true, {acceleration_drive = true})
        T { motion }
        T.position(0, -0.15, 1).scale(16, 0.30, 10) {
            Zone.cube([0.5,0.5,0.5]) { Collidable.static().friction(0.55) {} }
        }
        T.position(0, -1, -9).rotation(-Math.atan2(2,10), 0, 0) {
            T.position(0,-0.15,0).scale(16,0.30,Math.sqrt(104)) {
                Zone.cube([0.5,0.5,0.5]) { Collidable.static().friction(0.55) {} }
            }
        }
        T.position(0, -2.15, -20).scale(16, 0.30, 12) {
            Zone.cube([0.5,0.5,0.5]) { Collidable.static().friction(0.55) {} }
        }
        let control = T { name = "test_driver" }
        on(control, "DataEvent", fn(event) {
            let truck = motion.query("#pickup_truck")
            truck.set_powered(true)
            if event == "Drive" { truck.drive_command(1,0,false) }
            if event == "Coast" { truck.drive_command(0,0,false) }
            if event == "Steer" { truck.drive_command(0,0.5,false) }
        })
    "##,
            "examples/_sakura_twilight_pickup_test.mms",
        )
        .unwrap();
    fn tick(scene: &mut Universe, frames: usize) {
        for _ in 0..frames {
            scene.update(1.0 / 60.0, &Default::default());
        }
    }
    fn command(scene: &mut Universe, name: &str) {
        scene.command_queue.push_event(
            named(scene, "test_driver"),
            EventSignal::DataEvent {
                name: name.into(),
                payload: None,
            },
        );
    }
    fn speed(scene: &Universe) -> f32 {
        let velocity = named(scene, "pickup_velocity");
        -scene
            .world
            .get_component_by_id_as::<VelocityComponent>(velocity)
            .unwrap()
            .linear_local_mps[2]
    }
    fn rotation(scene: &Universe, label: &str) -> [f32; 4] {
        scene
            .world
            .get_component_by_id_as::<TransformComponent>(named(scene, label))
            .unwrap()
            .transform
            .rotation
    }
    tick(&mut scene, 6);
    command(&mut scene, "Steer");
    tick(&mut scene, 4);
    assert!(rotation(&scene, "steering_fl")[1].abs() > 0.05);
    assert_eq!(rotation(&scene, "steering_rl"), [0.0, 0.0, 0.0, 1.0]);
    command(&mut scene, "Drive");
    tick(&mut scene, 20);
    let driven = speed(&scene);
    assert!(driven > 1.0 && driven < 5.0, "forward speed: {driven}");
    assert!(rotation(&scene, "wheel_fl")[0].abs() > 0.02);
    command(&mut scene, "Coast");
    tick(&mut scene, 8);
    let coasting = speed(&scene);
    assert!(
        coasting > 0.3 && coasting < driven,
        "coasting: {coasting} from {driven}"
    );
    command(&mut scene, "Drive");
    tick(&mut scene, 105);
    let chassis = named(&scene, "pickup_truck");
    let position = TransformSystem::world_position(&scene.world, chassis).unwrap();
    assert!(
        position[2] < -5.0 && position[1] < -0.15 && position[1] > -2.0,
        "ramp pose: {position:?}"
    );
    assert!(
        rotation(&scene, "pickup_truck")[0] < -0.035,
        "truck should pitch down the ramp"
    );
}
