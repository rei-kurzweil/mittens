use super::*;
use crate::engine::ecs::component::{AvatarControlComponent, PointerComponent, TransformComponent};
use crate::engine::ecs::system::SystemWorld;
use crate::engine::ecs::{CommandQueue, ComponentId};
use crate::engine::graphics::RenderAssets;
use crate::scripting::runner::RuntimeSpecSession;

fn near(actual: [f32; 3], expected: [f32; 3]) {
    for axis in 0..3 {
        assert!(
            (actual[axis] - expected[axis]).abs() < 1e-4,
            "{actual:?} != {expected:?}"
        );
    }
}

fn check_scene(source: &str, path: &str) {
    // Exercise the real authored rig without opening a hardware session. Only
    // runtime initialization is disabled; pose application is the production path.
    assert!(source.contains("\nXR.on()\n"));
    let source = source.replace("\nXR.on()\n", "\nXR.off()\n");
    let mut world = World::default();
    let mut systems = SystemWorld::default();
    let mut visuals = VisualWorld::default();
    let mut assets = RenderAssets::new();
    let mut queue = CommandQueue::new();
    let (_session, output) = RuntimeSpecSession::start_at_path(
        &source,
        path,
        &mut world,
        &mut systems.rx,
        Some(&mut assets),
        &mut queue,
    )
    .unwrap();
    assert!(output.errors.is_empty(), "{:?}", output.errors);
    for intent in output.intents {
        queue.push_intent_now(ComponentId::default(), intent);
    }
    systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);
    assert!(!world.all_components().any(|id| {
        world
            .get_component_by_id_as::<AvatarControlComponent>(id)
            .is_some()
    }));
    let input = world
        .all_components()
        .find(|&id| {
            world
                .get_component_by_id_as::<InputXRComponent>(id)
                .is_some()
        })
        .unwrap();
    let camera = world
        .all_components()
        .find(|&id| {
            world
                .get_component_by_id_as::<CameraXRComponent>(id)
                .is_some()
        })
        .unwrap();
    visuals.set_active_xr_camera(Some(camera));
    let hands: Vec<_> = [ControllerHand::Left, ControllerHand::Right]
        .into_iter()
        .map(|side| {
            let hand = world
                .all_components()
                .find(|&id| {
                    world
                        .get_component_by_id_as::<XRHandComponent>(id)
                        .is_some_and(|config| config.hand == side)
                })
                .unwrap();
            let driver = OpenXRSystem::transform_child_of(&world, hand)
                .expect("each XRHand must have a driven Transform child");
            let pointer = world
                .children_of(driver)
                .iter()
                .copied()
                .find(|&id| {
                    world
                        .get_component_by_id_as::<PointerComponent>(id)
                        .is_some()
                })
                .expect("pointer must descend from its tracked hand Transform");
            let lasers: Vec<_> = world
                .children_of(driver)
                .iter()
                .copied()
                .filter(|&id| world.component_label(id) == Some("xr_pointer_laser"))
                .collect();
            assert_eq!(lasers.len(), 1, "one hand-space beam per controller");
            let raycaster = systems.pointer.raycast_for_pointer(pointer).unwrap();
            (hand, driver, pointer, lasers[0], raycaster)
        })
        .collect();
    assert_eq!(
        world
            .all_components()
            .filter(|&id| world.component_label(id) == Some("xr_pointer_laser"))
            .count(),
        2
    );

    // Moving/rotating the head and each hand must move the beam and ray together.
    // The two controllers are independent, and neither follows the head a second time.
    for frame in 0..3 {
        let head = OpenXRSystem::pose_from_quat_translation(
            math::quat_rotation_y(0.3 * frame as f32),
            [0.1 * frame as f32, 1.65 - 0.1 * frame as f32, 0.0],
        );
        OpenXRSystem::drive_head_pose(&mut world, input, head, 0.0, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);
        let stage = OpenXRSystem::tracking_world_frame(&world, &visuals, head).unwrap();
        let mut expected = Vec::new();
        for (index, &(hand, _, _, _, _)) in hands.iter().enumerate() {
            let rotation = math::quat_rotation_y(0.2 * frame as f32 + 0.4 * index as f32);
            let position = [
                -0.35 + 0.7 * index as f32,
                1.1 + 0.12 * frame as f32,
                -0.5 - 0.1 * index as f32,
            ];
            let pose = OpenXRSystem::pose_from_quat_translation(rotation, position);
            assert!(OpenXRSystem::drive_controller_pose(
                &mut world, hand, pose, stage, &mut queue
            ));
            expected.push(OpenXRSystem::mul_mat4(
                &stage,
                &OpenXRSystem::mat4_from_pose(pose),
            ));
        }
        systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);
        let input = InputState::default();
        let activations = systems.pointer.build_activations(
            &world,
            &input,
            &XrInputState {
                trigger_down: [true, true],
                trigger_pressed: [true, false],
                ..Default::default()
            },
        );
        assert_eq!(
            activations.pressed,
            vec![hands[0].2],
            "left trigger activates only the left controller pointer"
        );
        assert_eq!(
            activations.down.len(),
            2,
            "both hand pointers work without an avatar"
        );
        systems.raycast.tick_with_queue(
            &mut world,
            &mut visuals,
            &input,
            &mut systems.rx,
            &systems.bvh,
            &activations,
            &systems.pointer,
            0.016,
        );
        for (index, &(hand, driver, _, laser, raycaster)) in hands.iter().enumerate() {
            let matrix = expected[index];
            let position = [matrix[3][0], matrix[3][1], matrix[3][2]];
            near(
                TransformSystem::world_position(&world, driver).unwrap(),
                position,
            );
            near(
                TransformSystem::world_position(&world, laser).unwrap(),
                position,
            );
            assert!(
                world
                    .get_component_by_id_as::<XRHandComponent>(hand)
                    .unwrap()
                    .pose_valid
            );
            let ray = systems
                .raycast
                .current_ray(raycaster)
                .expect("controller ray must use its current tracked pose");
            near(ray.origin_world, position);
            near(
                ray.direction_world,
                [-matrix[2][0], -matrix[2][1], -matrix[2][2]],
            );
        }
    }
}

#[test]
fn shared_materials_xr_controller_lasers_follow_poses_without_an_avatar() {
    check_scene(
        include_str!("../../../../examples/shared-materials-xr.mms"),
        "examples/shared-materials-xr.mms",
    );
}

#[test]
fn shading_models_xr_controller_lasers_follow_poses_without_an_avatar() {
    check_scene(
        include_str!("../../../../examples/shading-models-xr.mms"),
        "examples/shading-models-xr.mms",
    );
}
