use crate::engine::Universe;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::system::external_xr_frame::ExternalXrFrame;
use crate::engine::ecs::{
    ComponentId, EventSignal, IntentValue, KeyboardEvent, SignalEmitter, World,
};
use crate::engine::graphics::{CameraData, CameraTarget};
use crate::utils::math::{mat4_mul_vec4, vec3_len, vec3_sub};

fn setup(xr: bool) -> Universe {
    setup_at_scale(xr, 1.0)
}

fn setup_at_scale(xr: bool, scale: f32) -> Universe {
    let source = r##"
        import { mitten_model } from "../assets/components/characters/mitten.mms"
        let use_xr = __XR__
        let mitten = mitten_model("view_test_mitten", {})
        let body = AVC.movement_target("#view_test_root").third_person(use_xr) {
            T { mitten.model }
            T {
                name = "view_test_camera_mount"
                if use_xr { CXR.on() } else { C3D {} }
            }
        }
        let head = T.position(0, 1.6, 0) { name = "view_test_head" body }
        T.scale(__SCALE__, __SCALE__, __SCALE__) {
            name = "view_test_root"
            if use_xr { InputXR.on() { head } } else { I { head } }
        }
        on_global("KeyUp", fn(event) {
            if event.code == "KeyV" { body.set_third_person(true) }
            if event.code == "KeyB" { body.set_third_person(false) }
        })
    "##
    .replace("__XR__", if xr { "true" } else { "false" })
    .replace("__SCALE__", &format!("{scale:?}"));
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(&source, "examples/_avatar_camera_test.mms")
        .unwrap();
    if xr {
        supply_xr_frame(&mut scene, [0.0, 1.65, 0.0], 0.0);
    }
    for _ in 0..8 {
        scene.update(1.0 / 60.0, &Default::default());
    }
    scene
}

fn named(scene: &Universe, name: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(name))
        .unwrap_or_else(|| panic!("missing {name}"))
}

fn release_key(scene: &mut Universe, key: &str) {
    scene.systems.rx.push_event(
        ComponentId::default(),
        EventSignal::KeyUp(KeyboardEvent {
            code: Some(format!("Key{key}")),
            key: key.into(),
        }),
    );
    // The runtime services the callback after the systems frame.
    scene.update(1.0 / 60.0, &Default::default());
    scene.update(1.0 / 60.0, &Default::default());
}

fn eyes(scene: &Universe, target: CameraTarget) -> &[CameraData] {
    &scene.visuals.visual_camera(target).unwrap().eyes
}

fn eye_position(eye: &CameraData) -> [f32; 3] {
    let [x, y, z, _] = eye.transform.matrix_world[3];
    [x, y, z]
}

fn near(actual: [f32; 3], expected: [f32; 3]) {
    assert!(
        vec3_len(vec3_sub(actual, expected)) < 1e-4,
        "{actual:?} != {expected:?}"
    );
}

#[test]
fn third_person_desktop_frames_origin_and_follows_root_without_moving_head() {
    let mut scene = setup(false);
    let root = named(&scene, "view_test_root");
    let head = named(&scene, "view_test_head");
    let anchor = named(&scene, "view_test_camera_mount");
    let initial_head = TransformSystem::world_model(&scene.world, head);
    let initial_anchor = TransformSystem::world_model(&scene.world, anchor);
    let first = eyes(&scene, CameraTarget::Window)[0];

    release_key(&mut scene, "V");
    let third = eyes(&scene, CameraTarget::Window)[0];
    assert!(
        vec3_len(vec3_sub(
            third.transform.translation,
            first.transform.translation
        )) > 1.0
    );
    assert_eq!(third.proj, first.proj);
    for y in [0.0, 1.8] {
        let clip = mat4_mul_vec4(third.proj, mat4_mul_vec4(third.view, [0.0, y, 0.0, 1.0]));
        assert!(clip[3] > 0.0 && clip[0].abs() < clip[3] && clip[1].abs() < clip[3]);
    }
    assert_eq!(
        TransformSystem::world_model(&scene.world, head),
        initial_head
    );
    assert_eq!(
        TransformSystem::world_model(&scene.world, anchor),
        initial_anchor
    );
    release_key(&mut scene, "B");
    assert_eq!(eyes(&scene, CameraTarget::Window)[0].view, first.view);
    release_key(&mut scene, "V");

    let movement = [2.0, 0.4, -1.0];
    scene.command_queue.push_intent_now(
        root,
        IntentValue::UpdateTransform {
            component_id: root,
            translation: movement,
            rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        },
    );
    scene.update(1.0 / 60.0, &Default::default());
    let moved = eyes(&scene, CameraTarget::Window)[0];
    near(
        vec3_sub(moved.transform.translation, third.transform.translation),
        movement,
    );
}

fn supply_xr_frame(scene: &mut Universe, head: [f32; 3], yaw: f32) {
    let half_yaw = yaw * 0.5;
    let views = [-0.032, 0.032].map(|eye_x| openxr::View {
        pose: openxr::Posef {
            orientation: openxr::Quaternionf {
                x: 0.0,
                y: half_yaw.sin(),
                z: 0.0,
                w: half_yaw.cos(),
            },
            position: openxr::Vector3f {
                x: head[0] + eye_x * yaw.cos(),
                y: head[1],
                z: head[2] - eye_x * yaw.sin(),
            },
        },
        fov: openxr::Fovf {
            angle_left: -0.7,
            angle_right: 0.7,
            angle_up: 0.7,
            angle_down: -0.7,
        },
    });
    scene.systems.xr.set_external_frame(Some(ExternalXrFrame {
        views: Some(views),
        ..Default::default()
    }));
}

#[test]
fn third_person_xr_keeps_stereo_and_head_motion_separate_from_the_tracked_head() {
    let mut scene = setup(true);
    let head = named(&scene, "view_test_head");
    let head_position = TransformSystem::world_position(&scene.world, head).unwrap();
    near(head_position, [0.0, 1.65, 0.0]);
    let first = eyes(&scene, CameraTarget::Xr)[0];
    assert!(vec3_len(vec3_sub(eye_position(&first), head_position)) > 1.0);

    supply_xr_frame(&mut scene, [0.0, 1.15, 0.0], 0.6);
    scene.update(1.0 / 60.0, &Default::default());
    let [left, right] = eyes(&scene, CameraTarget::Xr) else {
        panic!("expected two XR eyes")
    };
    assert!((vec3_len(vec3_sub(eye_position(right), eye_position(left))) - 0.064).abs() < 1e-4);
    assert!(vec3_len(vec3_sub(eye_position(left), eye_position(&first))) > 0.4);
    assert_ne!(
        left.transform.matrix_world[2],
        first.transform.matrix_world[2]
    );
    near(
        TransformSystem::world_position(&scene.world, head).unwrap(),
        [0.0, 1.15, 0.0],
    );
    release_key(&mut scene, "B");
    let [left, right] = eyes(&scene, CameraTarget::Xr) else {
        panic!("expected two XR eyes")
    };
    let eye_center =
        std::array::from_fn(|i| (eye_position(left)[i] + eye_position(right)[i]) * 0.5);
    near(
        eye_center,
        TransformSystem::world_position(&scene.world, head).unwrap(),
    );
}

#[test]
fn third_person_xr_frames_a_scaled_rig_at_its_own_scale() {
    let eyes_at = |scale: f32| {
        let scene = setup_at_scale(true, scale);
        let root = named(&scene, "view_test_root");
        let origin = TransformSystem::world_position(&scene.world, root).unwrap();
        let [left, right] = eyes(&scene, CameraTarget::Xr) else {
            panic!("expected two XR eyes")
        };
        let separation = vec3_len(vec3_sub(eye_position(right), eye_position(left)));
        (separation, vec3_len(vec3_sub(eye_position(left), origin)))
    };
    let (full_separation, full_distance) = eyes_at(1.0);
    let (tiny_separation, tiny_distance) = eyes_at(0.1);
    assert!((full_separation - 0.064).abs() < 1e-4);
    assert!((tiny_separation - 0.0064).abs() < 1e-5, "{tiny_separation}");
    assert!(
        (tiny_distance - full_distance * 0.1).abs() < 1e-3,
        "{tiny_distance} vs {full_distance}"
    );
}

#[test]
fn third_person_camera_pulls_in_before_a_wall_but_not_the_players_own_collider() {
    use crate::engine::ecs::component::{
        CollidableComponent, CollisionShape, TransformComponent, ZoneComponent,
    };
    fn solid(world: &mut World, parent: Option<ComponentId>, position: [f32; 3], half: f32) {
        let frame = world.add_component(
            TransformComponent::new().with_position(position[0], position[1], position[2]),
        );
        let t = world.get_component_by_id_as_mut::<TransformComponent>(frame).unwrap();
        t.transform.matrix_world = t.transform.model;
        let zone = world.add_component(ZoneComponent::new(CollisionShape::cube_half_extents([half; 3])));
        let collidable = world.add_component(CollidableComponent::static_());
        world.add_child(frame, zone).unwrap();
        world.add_child(zone, collidable).unwrap();
        if let Some(parent) = parent {
            world.add_child(parent, frame).unwrap();
        }
    }
    let mut world = World::default();
    let root = world.add_component(TransformComponent::new());
    // The player's own body box, a metre behind the focus, under its root.
    solid(&mut world, Some(root), [0.0, 0.0, 1.0], 0.3);
    let open = super::unblocked_distance(&world, root, [0.0; 3], [0.0, 0.0, 1.0], 4.0, 1.0);
    assert!((open - 4.0).abs() < 1e-4, "own collider ignored: {open}");
    // A wall whose near face is 2.5 m behind.
    solid(&mut world, None, [0.0, 0.0, 3.0], 0.5);
    let blocked = super::unblocked_distance(&world, root, [0.0; 3], [0.0, 0.0, 1.0], 4.0, 1.0);
    assert!((blocked - 2.25).abs() < 1e-3, "sphere stops short of the wall: {blocked}");
}
