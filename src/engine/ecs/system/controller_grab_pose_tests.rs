use super::*;
use crate::engine::{
    Universe,
    ecs::{
        EventSignal, SignalEmitter, World,
        component::{PointerComponent, TextComponent},
        system::{TransformSystem, external_xr_frame::ExternalXrFrame},
    },
};

fn named(scene: &Universe, name: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(name))
        .unwrap()
}
fn pose(position: [f32; 3], rotation: [f32; 4]) -> openxr::Posef {
    openxr::Posef {
        position: openxr::Vector3f {
            x: position[0],
            y: position[1],
            z: position[2],
        },
        orientation: openxr::Quaternionf {
            x: rotation[0],
            y: rotation[1],
            z: rotation[2],
            w: rotation[3],
        },
    }
}
fn frame(grip: Option<openxr::Posef>) -> ExternalXrFrame {
    let eye = |x| openxr::View {
        pose: pose([x, 1.6, 0.], [0., 0., 0., 1.]),
        fov: openxr::Fovf {
            angle_left: -0.6,
            angle_right: 0.6,
            angle_up: 0.6,
            angle_down: -0.6,
        },
    };
    ExternalXrFrame {
        views: Some([eye(-0.03), eye(0.03)]),
        aim: [None, Some(pose([0.8, 1.2, -0.2], [0., 0., 0., 1.]))],
        grip: [None, grip],
        ..Default::default()
    }
}
fn near(a: [f32; 3], b: [f32; 3]) {
    assert!(
        a.into_iter().zip(b).all(|(a, b)| (a - b).abs() < 0.001),
        "{a:?} != {b:?}"
    );
}
#[test]
fn controller_grip_holding_also_supports_a_root_controller_without_an_avatar() {
    use crate::engine::ecs::component::{ControllerHand, ControllerPoseKind, PointerGrabPose};
    let mut world = World::default();
    let controller = world.add_component(ControllerXRComponent::new(
        true,
        ControllerHand::Right,
        ControllerPoseKind::Aim,
    ));
    let driver = world.add_component(TransformComponent::new());
    let pointer =
        world.add_component(PointerComponent::new().grab_pose(PointerGrabPose::ControllerGrip));
    world.add_child(controller, driver).unwrap();
    world.add_child(driver, pointer).unwrap();
    assert!(
        ControllerGripHold::start(&world, pointer, TransformTrs::IDENTITY.to_matrix().unwrap())
            .is_none(),
        "untracked grip must not use aim"
    );
    world
        .get_component_by_id_as_mut::<ControllerXRComponent>(controller)
        .unwrap()
        .grip_pose_in_parent = Some(TransformTrs::new([1., 2., 3.], [0., 0., 0., 1.], [1.; 3]));
    let (mut hold, parent) =
        ControllerGripHold::start(&world, pointer, TransformTrs::IDENTITY.to_matrix().unwrap())
            .unwrap();
    assert_eq!(parent, driver);
    near(
        hold.tick(&world, parent, 1.).unwrap().translation,
        [1., 2., 3.],
    );
}
#[test]
fn controller_grip_holding_ignores_hand_bone_and_aim_follows_grip_and_copies_mms_trs() {
    let mut scene = Universe::new(World::default());
    scene.load_mms_source_at_path(r#"
        XR.off()
        let controller=XRHand.new(true,"Right","GripAim") {name="controller"
            T {T.position(0,0,3).rotation(0,0,1.57) {name="fake_hand"
                Pointer.grab_pose("controller_grip").grab_attachment("[name='fake_hand']") {name="pointer"}
            }}
        }
        T.position(2,0,1) {name="rig" CXR {} controller}
        T.scale(0.25,0.5,0.75) {name="held" Grabbable.to_hand(true).hand_offset([1,2,3]) {} R.cube() {name="mesh"}}
        let sample=T {name="sample"}
        let status=Text {name="sample_status" "unset"}
        let control=T {name="sample_control"}
        sample status control
        on(control,"DataEvent",fn(event) {
            let grip=controller.get_grip_pose()
            if grip==null {status.set_text("untracked")}
            else {sample.world_trs(grip) status.set_text("tracked")}
        })
    "#,"examples/_controller_grip_test.mms").unwrap();
    let controller = named(&scene, "controller");
    let pointer = named(&scene, "pointer");
    let target = named(&scene, "held");
    let mesh = named(&scene, "mesh");
    let control = named(&scene, "sample_control");
    let rotation = crate::utils::math::quat_rotation_y(0.8);
    scene
        .systems
        .xr
        .set_external_frame(Some(frame(Some(pose([0.3, 1.0, -0.6], rotation)))));
    scene.update(0.016, &Default::default());
    let get_grip = |scene: &Universe| {
        scene
            .world
            .get_component_by_id_as::<ControllerXRComponent>(controller)
            .unwrap()
            .get_grip_pose(&scene.world)
    };
    let grip = get_grip(&scene).expect("raw grip sample is available even for a GripAim driver");
    assert!(
        (grip.rotation_quat_xyzw[1] - rotation[1]).abs() < 1e-4,
        "grip orientation must not come from aim"
    );
    assert_eq!(
        scene
            .world
            .get_component_by_id_as::<PointerComponent>(pointer)
            .unwrap()
            .grab_pose,
        crate::engine::ecs::component::PointerGrabPose::ControllerGrip
    );
    let target_world = TransformSystem::world_model(&scene.world, target).unwrap();
    scene.systems.grabbable.start(
        &mut scene.world,
        &scene.render_assets,
        &mut scene.command_queue,
        pointer,
        target,
        mesh,
        [0.; 3],
        [0., 0., -1.],
    );
    assert_ne!(
        scene.world.parent_of(target),
        Some(named(&scene, "fake_hand"))
    );
    for _ in 0..70 {
        scene.update(0.016, &Default::default());
    }
    let held = TransformSystem::world_trs(&scene.world, target).unwrap();
    near(held.translation, get_grip(&scene).unwrap().translation);
    near(held.scale, [0.25, 0.5, 0.75]);
    assert!(
        held.rotation_quat_xyzw
            .into_iter()
            .zip(rotation)
            .all(|(a, b)| (a - b).abs() < 0.001)
    );
    // A settled object follows a new controller sample immediately, without a
    // second smoothing delay or influence from the deliberately distant hand bone.
    scene
        .systems
        .xr
        .set_external_frame(Some(frame(Some(pose([0.7, 1.3, -0.8], rotation)))));
    scene.update(0.016, &Default::default());
    near(
        TransformSystem::world_position(&scene.world, target).unwrap(),
        get_grip(&scene).unwrap().translation,
    );
    scene.command_queue.push_event(
        control,
        EventSignal::DataEvent {
            name: "Sample".into(),
            payload: None,
        },
    );
    scene.update(0.016, &Default::default());
    let sample = named(&scene, "sample");
    let copy = TransformSystem::world_trs(&scene.world, sample).unwrap();
    near(copy.translation, get_grip(&scene).unwrap().translation);
    assert_eq!(
        scene
            .world
            .get_component_by_id_as::<TextComponent>(named(&scene, "sample_status"))
            .unwrap()
            .text,
        "tracked"
    );
    // Aim remains valid while grip is lost: neither API nor holding falls back to it.
    scene.systems.xr.set_external_frame(Some(frame(None)));
    scene.update(0.016, &Default::default());
    scene.command_queue.push_event(
        control,
        EventSignal::DataEvent {
            name: "Sample".into(),
            payload: None,
        },
    );
    scene.update(0.016, &Default::default());
    assert!(get_grip(&scene).is_none());
    assert_eq!(
        scene
            .world
            .get_component_by_id_as::<TextComponent>(named(&scene, "sample_status"))
            .unwrap()
            .text,
        "untracked"
    );
    near(
        TransformSystem::world_trs(&scene.world, sample)
            .unwrap()
            .translation,
        copy.translation,
    );
    // Keep an assertion on the original pose to ensure the ease-in began from
    // the actual object rather than an unrelated hand/aim frame.
    assert_eq!(
        [target_world[3][0], target_world[3][1], target_world[3][2]],
        [0.; 3]
    );
}
