use crate::engine::{
    Universe,
    ecs::{
        ComponentId, EventSignal, KeyboardEvent, SignalEmitter, SignalKind, World,
        component::{
            AvatarControlComponent, Camera3DComponent, CameraXRComponent, ControllerXRComponent,
            GravityComponent, InputComponent, InputXRComponent, InputXRGamepadComponent,
            RenderableComponent, XrComponent,
        },
        system::{TransformSystem, pointer_system::nearest_ancestor_transform},
    },
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};

#[derive(Default)]
struct ScreenClock(AtomicU64);
impl crate::engine::ecs::system::ClockDriver for ScreenClock {
    fn name(&self) -> &'static str {
        "paper_screen_test"
    }
    fn time_now_sec(&self) -> f64 {
        self.0.load(Ordering::Relaxed) as f64 / 1000.
    }
}
fn named(scene: &Universe, name: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(name))
        .unwrap_or_else(|| panic!("missing {name}"))
}
fn advance(scene: &mut Universe, clock: &ScreenClock, frames: usize) {
    for _ in 0..frames {
        clock.0.fetch_add(16, Ordering::Relaxed);
        scene.update(0.016, &Default::default());
    }
}
fn grab(scene: &mut Universe, pointer: ComponentId, target: ComponentId) {
    let renderable = scene
        .world
        .all_components()
        .find(|&id| {
            scene
                .world
                .get_component_by_id_as::<RenderableComponent>(id)
                .is_some()
                && {
                    let mut p = scene.world.parent_of(id);
                    let mut found = false;
                    while let Some(id) = p {
                        if id == target {
                            found = true;
                            break;
                        }
                        p = scene.world.parent_of(id);
                    }
                    found
                }
        })
        .unwrap();
    scene.command_queue.push_event(
        target,
        EventSignal::GrabStart {
            pointer,
            raycaster: ComponentId::default(),
            renderable,
            target,
            ray_origin_world: [0.; 3],
            ray_dir_world: [0., 0., -1.],
        },
    );
}
fn release(scene: &mut Universe, pointer: ComponentId, target: ComponentId) {
    scene
        .command_queue
        .push_event(target, EventSignal::GrabEnd { pointer, target });
}
fn fire_key(scene: &mut Universe, down: bool) {
    let key = KeyboardEvent {
        code: Some("KeyF".into()),
        key: "f".into(),
    };
    scene.command_queue.push_event(
        ComponentId::default(),
        if down {
            EventSignal::KeyDown(key)
        } else {
            EventSignal::KeyUp(key)
        },
    );
}

#[test]
fn paper_craft_screen_mode_initializes_without_xr_and_grabs_and_fires_only_held_tools() {
    let source = include_str!("../../examples/paper-craft.mms").replacen(
        "let use_openxr = true",
        "let use_openxr = false",
        1,
    );
    assert!(source.contains("let use_openxr = false"));
    let mut scene = Universe::new(World::default());
    let clock = Arc::new(ScreenClock::default());
    scene.systems.clock.set_driver(clock.clone());
    scene
        .load_mms_source_at_path(&source, "examples/paper-craft.mms")
        .unwrap();
    super::display_car_tests::wait_for_material_compilation(&scene.visuals);
    advance(&mut scene, &clock, 6);
    super::display_car_tests::wait_for_material_compilation(&scene.visuals);
    let world = &scene.world;
    assert_eq!(
        world
            .all_components()
            .filter(|&id| world
                .get_component_by_id_as::<Camera3DComponent>(id)
                .is_some())
            .count(),
        1
    );
    assert_eq!(
        world
            .all_components()
            .filter(|&id| world.get_component_by_id_as::<InputComponent>(id).is_some())
            .count(),
        1
    );
    for id in world.all_components() {
        assert!(
            world
                .get_component_by_id_as::<CameraXRComponent>(id)
                .is_none()
        );
        assert!(
            world
                .get_component_by_id_as::<ControllerXRComponent>(id)
                .is_none()
        );
        assert!(
            world
                .get_component_by_id_as::<InputXRComponent>(id)
                .is_none()
        );
        assert!(
            world
                .get_component_by_id_as::<InputXRGamepadComponent>(id)
                .is_none()
        );
        if let Some(xr) = world.get_component_by_id_as::<XrComponent>(id) {
            assert!(!xr.enabled);
        }
    }
    let avatar = world
        .get_component_by_id_as::<AvatarControlComponent>(named(
            &scene,
            "paper_craft_mitten_avatar_control",
        ))
        .unwrap();
    assert!(
        avatar.model_root_id.is_some(),
        "real GLTF import and AVC splicing must complete"
    );
    assert!(
        avatar.left_hand_raw_target_id.is_none() && avatar.right_hand_raw_target_id.is_none(),
        "procedural holding arms remain a future feature"
    );
    // The startup overlay lowers both arms without changing limb lengths or
    // mutating the imported rest snapshots used by tracking and IK.
    for side in ["L", "R"] {
        use crate::engine::ecs::component::{BoneRestPoseComponent, TransformComponent};
        let upper = named(&scene, &format!("J_Bip_{side}_UpperArm"));
        let lower = named(&scene, &format!("J_Bip_{side}_LowerArm"));
        let t = world
            .get_component_by_id_as::<TransformComponent>(upper)
            .unwrap();
        let rest = world
            .children_of(upper)
            .iter()
            .find_map(|&id| world.get_component_by_id_as::<BoneRestPoseComponent>(id))
            .unwrap();
        assert_eq!(t.transform.translation, rest.translation);
        assert_eq!(t.transform.scale, rest.scale);
        assert!(
            (t.transform.rotation[0] - (-0.3134671)).abs() < 1e-5,
            "relaxed arm startup overlay"
        );
        assert!(
            (rest.rotation[0] - 0.0727172).abs() < 1e-5,
            "imported bind pose stays immutable"
        );
        let shoulder = TransformSystem::world_position(world, upper).unwrap();
        let elbow = TransformSystem::world_position(world, lower).unwrap();
        assert!(
            elbow[1] < shoulder[1] - 0.10,
            "A-pose elbows are below shoulders: {shoulder:?} -> {elbow:?}"
        );
    }
    assert!(
        world
            .get_component_by_id_as::<GravityComponent>(named(&scene, "paper_craft_mitten_gravity"))
            .unwrap()
            .enabled,
        "CapsuleReady enables screen gravity too"
    );
    let camera = named(&scene, "paper_craft_mitten_screen_camera");
    let position = TransformSystem::world_position(world, camera).unwrap();
    assert!(
        (20.0..22.0).contains(&position[1]),
        "first-person view must be above the roof, not at feet: {position:?}"
    );
    let pointer = named(&scene, "paper_craft_screen_pointer");
    let gohei = named(&scene, "paper_craft_gohei_black");
    let gohei_parent = scene.world.parent_of(gohei);
    let pointer_frame = nearest_ancestor_transform(&scene.world, pointer).unwrap();
    grab(&mut scene, pointer, gohei);
    advance(&mut scene, &clock, 3);
    assert_eq!(
        scene.world.parent_of(gohei),
        Some(pointer_frame),
        "current screen holding uses the camera frame"
    );
    release(&mut scene, pointer, gohei);
    advance(&mut scene, &clock, 2);
    assert_eq!(scene.world.parent_of(gohei), gohei_parent);

    let shots = Arc::new(AtomicUsize::new(0));
    let count = shots.clone();
    let completion = named(&scene, "paper_craft_pistol_laser_completion");
    scene
        .systems
        .rx
        .add_handler_closure(SignalKind::DataEvent, completion, move |_, _, signal| {
            if matches!(&signal.event,Some(EventSignal::DataEvent {name,..}) if name=="ShotFired") {
                count.fetch_add(1, Ordering::Relaxed);
            }
        });
    fire_key(&mut scene, true);
    advance(&mut scene, &clock, 3);
    fire_key(&mut scene, false);
    assert_eq!(
        shots.load(Ordering::Relaxed),
        0,
        "F cannot fire a pistol on the table"
    );
    let pistol = named(&scene, "paper_craft_machine_pistol_placement");
    grab(&mut scene, pointer, pistol);
    advance(&mut scene, &clock, 2);
    fire_key(&mut scene, true);
    advance(&mut scene, &clock, 4);
    assert_eq!(shots.load(Ordering::Relaxed), 1, "F fires the held pistol");
    fire_key(&mut scene, false);
    release(&mut scene, pointer, pistol);
    advance(&mut scene, &clock, 25);
    fire_key(&mut scene, true);
    advance(&mut scene, &clock, 3);
    assert_eq!(
        shots.load(Ordering::Relaxed),
        1,
        "release removes firing ownership"
    );
    // A later live pose write must survive subsequent frames: the A-pose is
    // a one-shot initial condition, not a continuous owner fighting arm IK.
    let upper = named(&scene, "J_Bip_L_UpperArm");
    let live_rotation = [0., 0., 0., 1.];
    scene
        .world
        .get_component_by_id_as_mut::<crate::engine::ecs::component::TransformComponent>(upper)
        .unwrap()
        .transform
        .rotation = live_rotation;
    scene
        .world
        .get_component_by_id_as_mut::<crate::engine::ecs::component::TransformComponent>(upper)
        .unwrap()
        .transform
        .recompute_model();
    scene
        .systems
        .transform_changed(&mut scene.world, &mut scene.visuals, upper);
    advance(&mut scene, &clock, 3);
    assert_eq!(
        scene
            .world
            .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(upper)
            .unwrap()
            .transform
            .rotation,
        live_rotation
    );
    super::display_car_tests::wait_for_material_compilation(&scene.visuals);
}
