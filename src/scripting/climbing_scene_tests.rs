use crate::engine::ecs::component::{
    ClimbableComponent, GLTFComponent, HandTrackingComponent, HumanoidBoneMapComponent,
};
use crate::engine::ecs::{CommandQueue, RxWorld, World};
use crate::engine::graphics::RenderAssets;
use crate::scripting::tests::start_settings_scene;

#[test]
fn xr_avatar_examples_have_hand_tracking_and_finger_maps() {
    let paths = [
        "bisket-vr-demo.mms",
        "bisket-vr-only-example.mms",
        "capsule-stick-figure.mms",
        "graphics-settings.mms",
        "input-xr-gamepad.mms",
        "lower-body-walking-vr.mms",
        "mittens-corp-agc.mms",
        "mittens-corp-linear-velocity.mms",
        "mittens-corp.mms",
        "rei(mu).mms",
        "sakura-drone-conversion.mms",
        "toon-outline-bisket.mms",
        "vtuber-editor-example.mms",
        "vtuber-eye-tracking-mirror-eye-stabilize.mms",
        "vtuber-eye-tracking-mirror.mms",
        "vtuber-microphone-speaking-xr-eye-tracking-htc.mms",
        "vtuber-microphone-speaking-xr-eye-tracking.mms",
        "vtuber-mirror-example.mms",
        "vtuber-secondary-motion.mms",
        "vtuber-slidedeck.mms",
        "xr-grab-demo.mms",
        "xr-settings.mms",
        "youtuber-room.mms",
    ];
    for name in paths {
        let path = format!("examples/{name}");

        let mut source = std::fs::read_to_string(&path).unwrap();
        // Avoid host audio-device enumeration in this headless fixture.
        source = source.replace(
            "let audio_input_devices = AudioInput.devices()",
            "let audio_input_devices = []",
        );
        if name == "youtuber-room.mms" {
            source = source
                .replace(
                    "let desktop_view_enabled = true",
                    "let desktop_view_enabled = false",
                )
                .replace("let xr_view_enabled = false", "let xr_view_enabled = true");
        }
        let mut world = World::default();
        let mut rx = RxWorld::default();
        let mut queue = CommandQueue::new();
        let mut assets = RenderAssets::new();
        let result = start_settings_scene(
            &source,
            &path,
            &mut world,
            &mut rx,
            Some(&mut assets),
            &mut queue,
        );
        assert!(result.is_ok(), "{path}: {:?}", result.err());
        let tracked = world.all_components().any(|id| {
            let model = world
                .get_component_by_id_as::<GLTFComponent>(id)
                .is_some_and(|g| {
                    g.uri == "assets/models/bisket.glb" || g.uri == "assets/models/rei(mu).glb"
                });
            model
                && world.children_of(id).iter().any(|id| {
                    world
                        .get_component_by_id_as::<HandTrackingComponent>(*id)
                        .is_some()
                })
                && world.children_of(id).iter().any(|id| {
                    world
                        .get_component_by_id_as::<HumanoidBoneMapComponent>(*id)
                        .is_some()
                })
        });
        assert!(
            tracked,
            "{path}: tracked avatar missing hand tracking or map"
        );
        if name == "youtuber-room.mms" {
            let count = world
                .all_components()
                .filter(|id| {
                    world
                        .get_component_by_id_as::<ClimbableComponent>(*id)
                        .is_some()
                })
                .count();
            assert!(count >= 12, "expected both pool ladders: {count}");
        }
    }
}

#[test]
fn pool_ladder_pointer_grips_move_the_tracking_origin() {
    use crate::engine::Universe;
    use crate::engine::ecs::component::TransformComponent;
    use crate::engine::ecs::system::external_xr_frame::ExternalXrFrame;
    use openxr as xr;
    let source = r#"
        import { indoor_pool } from "../assets/components/rooms/indoor-pool.mms"
        indoor_pool([0,0,0], 8.2, 14, 3.5, 9, true)
        indoor_pool([18,0,0], 8.2, 14, 3.5, 9, true)
        T { name = "climber" InputXR.on() {
            T { CXR {} }
            XRHand.new(true, "Left", "GripAim") { T { Pointer {} } }
        } }
    "#;
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(source, "examples/pool-climbing-test.mms")
        .unwrap();
    let pose = |position: [f32; 3], yaw: f32| xr::Posef {
        orientation: xr::Quaternionf {
            x: 0.0,
            y: (yaw / 2.0).sin(),
            z: 0.0,
            w: (yaw / 2.0).cos(),
        },
        position: xr::Vector3f {
            x: position[0],
            y: position[1],
            z: position[2],
        },
    };
    let view = xr::View {
        pose: pose([0.0, 1.65, 0.0], 0.0),
        fov: xr::Fovf {
            angle_left: -0.7,
            angle_right: 0.7,
            angle_up: 0.7,
            angle_down: -0.7,
        },
    };
    let mut frame = ExternalXrFrame {
        views: Some([view, view]),
        ..Default::default()
    };
    for (position, yaw) in [
        ([0.0, 0.18, 0.9], std::f32::consts::PI),
        ([0.92, 0.2, 12.3], 0.0),
        ([18.0, 0.18, 0.9], std::f32::consts::PI),
        ([18.92, 0.2, 12.3], 0.0),
    ] {
        let root = scene
            .world
            .all_components()
            .find(|&id| scene.world.component_label(id) == Some("climber"))
            .unwrap();
        let t = scene
            .world
            .get_component_by_id_as_mut::<TransformComponent>(root)
            .unwrap();
        t.transform.translation = [0.0; 3];
        t.transform.recompute_model();
        scene
            .systems
            .transform_changed(&mut scene.world, &mut scene.visuals, root);
        let hand = pose(position, yaw);
        frame.aim[0] = Some(hand);
        frame.grip[0] = Some(hand);
        frame.input = Default::default();
        scene.systems.xr.set_external_frame(Some(frame));
        scene.update(1.0 / 90.0, &Default::default());
        let root = scene
            .world
            .all_components()
            .find(|&id| scene.world.component_label(id) == Some("climber"))
            .unwrap();
        let before = scene
            .world
            .get_component_by_id_as::<TransformComponent>(root)
            .unwrap()
            .transform
            .translation;
        frame.input.grip_pressed[0] = true;
        frame.input.grip_down[0] = true;
        scene.systems.xr.set_external_frame(Some(frame));
        scene.update(1.0 / 90.0, &Default::default());
        frame.input.grip_pressed[0] = false;
        frame.aim[0].as_mut().unwrap().position.y -= 0.2;
        frame.grip[0].as_mut().unwrap().position.y -= 0.2;
        scene.systems.xr.set_external_frame(Some(frame));
        scene.update(1.0 / 90.0, &Default::default());
        let after = scene
            .world
            .get_component_by_id_as::<TransformComponent>(root)
            .unwrap()
            .transform
            .translation;
        assert!(
            (after[1] - before[1] - 0.2).abs() < 1e-4,
            "ladder at {position:?} did not climb: {before:?} -> {after:?}"
        );
        frame.input.grip_down[0] = false;
        frame.input.grip_released[0] = true;
        scene.systems.xr.set_external_frame(Some(frame));
        scene.update(1.0 / 90.0, &Default::default());
    }
}
