use mittens_engine::engine::{
    Universe,
    ecs::{
        World,
        component::{ControllerXRComponent, InputXRComponent, TransformComponent},
        system::{
            XrGamepadState, XrHandGamepadState, XrInputState, external_xr_frame::ExternalXrFrame,
        },
    },
    user_input::InputState,
};
use openxr as xr;

fn pose(position: [f32; 3]) -> xr::Posef {
    xr::Posef {
        orientation: xr::Quaternionf {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        },
        position: xr::Vector3f {
            x: position[0],
            y: position[1],
            z: position[2],
        },
    }
}

#[test]
fn external_frame_drives_avatar_free_controller_and_locomotion() {
    let source = r##"
        T.position(0.0, -0.1, 0.0).scale(20.0, 0.2, 20.0) { Zone.cube([0.5,0.5,0.5]) { Collidable.static() } }
        T.position(1.0, 1.0, 0.0).scale(0.2, 2.0, 4.0) { Zone.cube([0.5,0.5,0.5]) { Collidable.static() } }
        Gravity { let player_velocity = Velocity { T {
            name = "player"
            T.position(0.0, 0.85, 0.0) { Zone.capsule_y(0.25, 0.6) { Collidable.slide().movement_target("/#player") } }
            InputXR.on() {
                InputXRGamepad { locomotion() speed(1.4) }
                T { CXR {} }
                XRHand.new(true, "Left", "Aim") { T { name = "left_driver" Pointer {} } }
            }
        } }
            player_velocity
            on_global("XrButtonDown", fn(event) {
                if event.control == "ButtonB" && player_velocity.grounded() {
                    player_velocity.translate_world([0.0, 4.5, 0.0])
                }
            })
        }
    "##;
    let mut universe = Universe::new(World::default());
    universe
        .load_mms_source_at_path(source, "android/examples/sakura-stage.mms")
        .unwrap();
    let view = xr::View {
        pose: pose([0.0, 1.6, 0.0]),
        fov: xr::Fovf {
            angle_left: -0.7,
            angle_right: 0.7,
            angle_up: 0.7,
            angle_down: -0.7,
        },
    };
    let mut frame = ExternalXrFrame {
        views: Some([view, view]),
        aim: [Some(pose([-0.3, 1.2, -0.4])), None],
        grip: [None, None],
        input: XrInputState::default(),
        gamepad: XrGamepadState {
            active: true,
            hands: [
                XrHandGamepadState {
                    thumbstick: Some([1.0, 0.0]),
                    ..Default::default()
                },
                XrHandGamepadState::default(),
            ],
            head_pose_rotation: Some([0.0, 0.0, 0.0, 1.0]),
        },
    };
    for _ in 0..120 {
        universe.systems.xr.set_external_frame(Some(frame));
        universe.update(1.0 / 90.0, &InputState::default());
    }
    let world = &universe.world;
    let player = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("player"))
        .unwrap();
    let driver = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("left_driver"))
        .unwrap();
    let player_position = world
        .get_component_by_id_as::<TransformComponent>(player)
        .unwrap()
        .transform
        .translation;
    assert!(
        player_position[0] > 0.2 && player_position[0] < 0.75,
        "movement or wall collision failed: {player_position:?}"
    );
    assert!(
        player_position[1].abs() < 0.1,
        "player did not stand on floor: {player_position:?}"
    );
    let hand = world
        .all_components()
        .find_map(|id| world.get_component_by_id_as::<ControllerXRComponent>(id))
        .unwrap();
    assert!(hand.pose_valid, "external hand pose was not applied");
    let hand_pose = world
        .get_component_by_id_as::<TransformComponent>(driver)
        .unwrap()
        .transform
        .matrix_world;
    assert!(
        (hand_pose[3][0] - (player_position[0] - 0.3)).abs() < 0.1,
        "hand did not follow the rig: {:?}",
        hand_pose[3]
    );
    frame.gamepad.hands[0].thumbstick = Some([0.0, 0.0]);
    frame.gamepad.hands[1].button_b = Some((true, 1.0));
    for _ in 0..20 {
        universe.systems.xr.set_external_frame(Some(frame));
        universe.update(1.0 / 90.0, &InputState::default());
    }
    let jumped = universe
        .world
        .get_component_by_id_as::<TransformComponent>(player)
        .unwrap()
        .transform
        .translation[1];
    assert!(jumped > 0.3, "B did not cause a jump: {jumped}");
    frame.gamepad.hands[1].button_b = Some((false, 0.0));
    for _ in 0..150 {
        universe.systems.xr.set_external_frame(Some(frame));
        universe.update(1.0 / 90.0, &InputState::default());
    }
    let landed = universe
        .world
        .get_component_by_id_as::<TransformComponent>(player)
        .unwrap()
        .transform
        .translation[1];
    assert!(landed.abs() < 0.1, "player did not land: {landed}");
    frame.views = None;
    frame.aim = [None, None];
    frame.gamepad = XrGamepadState::default();
    universe.systems.xr.set_external_frame(Some(frame));
    universe.update(1.0 / 90.0, &InputState::default());
    assert!(
        universe
            .world
            .all_components()
            .filter_map(|id| universe
                .world
                .get_component_by_id_as::<InputXRComponent>(id))
            .all(|c| !c.pose_valid)
    );
    assert!(
        universe
            .world
            .all_components()
            .filter_map(|id| universe
                .world
                .get_component_by_id_as::<ControllerXRComponent>(id))
            .all(|c| !c.pose_valid)
    );
}

#[test]
fn explicit_left_stick_ignores_right_stick() {
    let source = r#"T { name = "player" InputXR.on() { InputXRGamepad { hand("left") locomotion() speed(1.0) } T { CXR {} } } }"#;
    let mut universe = Universe::new(World::default());
    universe
        .load_mms_source_at_path(source, "android/examples/sakura-stage.mms")
        .unwrap();
    let view = xr::View {
        pose: pose([0.0, 0.0, 0.0]),
        fov: xr::Fovf {
            angle_left: -0.7,
            angle_right: 0.7,
            angle_up: 0.7,
            angle_down: -0.7,
        },
    };
    let mut frame = ExternalXrFrame {
        views: Some([view, view]),
        aim: [None, None],
        grip: [None, None],
        input: XrInputState::default(),
        gamepad: XrGamepadState {
            active: true,
            hands: [
                XrHandGamepadState {
                    thumbstick: Some([0.0, 0.0]),
                    ..Default::default()
                },
                XrHandGamepadState {
                    thumbstick: Some([1.0, 0.0]),
                    ..Default::default()
                },
            ],
            head_pose_rotation: None,
        },
    };
    let id = universe
        .world
        .all_components()
        .find(|&id| universe.world.component_label(id) == Some("player"))
        .unwrap();
    for _ in 0..30 {
        universe.systems.xr.set_external_frame(Some(frame));
        universe.update(1.0 / 90.0, &InputState::default());
    }
    let position = universe
        .world
        .get_component_by_id_as::<TransformComponent>(id)
        .unwrap()
        .transform
        .translation;
    assert!(
        position[0].abs() < 0.01,
        "right stick moved player: {position:?}"
    );
    frame.gamepad.hands[0].thumbstick = Some([1.0, 0.0]);
    for _ in 0..30 {
        universe.systems.xr.set_external_frame(Some(frame));
        universe.update(1.0 / 90.0, &InputState::default());
    }
    let position = universe
        .world
        .get_component_by_id_as::<TransformComponent>(id)
        .unwrap()
        .transform
        .translation;
    assert!(
        position[0] > 0.15,
        "left stick did not move player: {position:?}"
    );
}
