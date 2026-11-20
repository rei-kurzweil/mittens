//! Body solver in a real scene: the twilight rooftop's remote avatar is driven
//! by a plain head target while no peer is connected, so lowering that target
//! stands in for a player getting down on hands and knees.
use crate::engine::Universe;
use crate::engine::ecs::component::{
    AvatarControlComponent, BodyPosture, CollisionDetail, HumanoidSlot, TransformComponent,
};
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::system::ik::BodyMode;
use crate::engine::ecs::system::ik::upper_body::UpperBodyStatus;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};

const ROOF: f32 = 16.0;

fn named(scene: &Universe, name: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(name))
        .unwrap_or_else(|| panic!("missing {name}"))
}

fn bone(scene: &Universe, avatar: ComponentId, slot: HumanoidSlot) -> ComponentId {
    let gltf = scene
        .world
        .get_component_by_id_as::<AvatarControlComponent>(avatar)
        .and_then(|avc| avc.humanoid_map_gltf)
        .unwrap();
    let report = scene.systems.humanoid_bone_map.report(gltf).unwrap();
    report
        .target(slot)
        .unwrap_or_else(|| panic!("unmapped {slot:?}"))
        .component
}

fn position(scene: &Universe, id: ComponentId) -> [f32; 3] {
    TransformSystem::world_position(&scene.world, id).unwrap()
}

/// Hold the remote head target `height` metres above the roof for `frames`.
fn hold_head(scene: &mut Universe, head: ComponentId, height: f32, frames: usize) {
    for _ in 0..frames {
        let rotation = scene
            .world
            .get_component_by_id_as::<TransformComponent>(head)
            .unwrap()
            .transform
            .rotation;
        scene.systems.rx.push_intent_now(
            head,
            IntentValue::UpdateTransform {
                component_id: head,
                translation: [0.0, height, 0.0],
                rotation_quat_xyzw: rotation,
                scale: [1.0; 3],
            },
        );
        scene.update(1.0 / 60.0, &Default::default());
    }
}

#[test]
fn twilight_remote_avatar_goes_to_hands_and_knees_and_stands_back_up() {
    let source = std::fs::read_to_string("examples/twilight_rooftop.mms").unwrap();
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(&source, "examples/twilight_rooftop.mms")
        .unwrap();
    let avatar = named(&scene, "remote_avatar");
    let head = named(&scene, "remote_head_target");
    hold_head(&mut scene, head, 1.644, 30);
    assert_eq!(
        scene.systems.upper_body_ik.status(avatar),
        UpperBodyStatus::Ready
    );
    assert_eq!(scene.systems.body_solver.mode(avatar), BodyMode::Vertical);
    assert_eq!(scene.systems.body_solver.torso_pitch(avatar), 0.0);

    // Head target 0.7 m above the roof: a hands-and-knees head height.
    hold_head(&mut scene, head, 0.7, 90);
    assert_eq!(scene.systems.body_solver.mode(avatar), BodyMode::Horizontal);
    assert_eq!(scene.systems.body_solver.weight(avatar), 1.0);
    let pitch = scene.systems.body_solver.torso_pitch(avatar);
    assert!(pitch > 1.0, "torso pitched toward horizontal: {pitch}");
    for (side, slots) in [
        (
            "left",
            [
                HumanoidSlot::LeftUpperLeg,
                HumanoidSlot::LeftLowerLeg,
                HumanoidSlot::LeftFoot,
            ],
        ),
        (
            "right",
            [
                HumanoidSlot::RightUpperLeg,
                HumanoidSlot::RightLowerLeg,
                HumanoidSlot::RightFoot,
            ],
        ),
    ] {
        let [hip, knee, ankle] = slots.map(|slot| position(&scene, bone(&scene, avatar, slot)));
        assert!(
            (knee[1] - ROOF).abs() < 0.12,
            "{side} knee on the roof: {knee:?}"
        );
        assert!(
            (ankle[1] - ROOF).abs() < 0.12,
            "{side} ankle on the roof: {ankle:?}"
        );
        assert!(
            hip[1] > knee[1] + 0.25,
            "{side} hip above knee: {hip:?} {knee:?}"
        );
    }
    let chest = position(&scene, bone(&scene, avatar, HumanoidSlot::UpperChest));
    let hips = position(&scene, bone(&scene, avatar, HumanoidSlot::Hips));
    let rise = chest[1] - hips[1];
    let run = ((chest[0] - hips[0]).powi(2) + (chest[2] - hips[2]).powi(2)).sqrt();
    assert!(
        run > rise.abs(),
        "torso closer to horizontal than vertical: rise {rise}, run {run}"
    );
    assert!(
        (yaw_threshold(&scene, avatar) - 10f32.to_radians()).abs() < 1e-4,
        "body yaw deadband tightens on hands and knees"
    );

    // Stand back up: blends out to vertical and the torso returns to rest.
    hold_head(&mut scene, head, 1.644, 90);
    assert_eq!(scene.systems.body_solver.mode(avatar), BodyMode::Vertical);
    assert_eq!(scene.systems.body_solver.torso_pitch(avatar), 0.0);
    let chest = position(&scene, bone(&scene, avatar, HumanoidSlot::UpperChest));
    let hips = position(&scene, bone(&scene, avatar, HumanoidSlot::Hips));
    assert!(
        chest[1] - hips[1] > 0.2,
        "upright again: {chest:?} {hips:?}"
    );
    assert_eq!(yaw_threshold(&scene, avatar), std::f32::consts::FRAC_PI_4);
}

fn yaw_threshold(scene: &Universe, avatar: ComponentId) -> f32 {
    use crate::engine::ecs::component::TransformOperatorComponent;
    use crate::engine::ecs::component::transform_operator::TransformQuatOp;
    let operator = scene
        .world
        .get_component_by_id_as::<AvatarControlComponent>(avatar)
        .and_then(|avc| avc.body_operator_id)
        .unwrap();
    scene
        .world
        .get_component_by_id_as::<TransformOperatorComponent>(operator)
        .and_then(|o| o.fork_trs_settings())
        .and_then(|settings| {
            settings.rotation_ops.iter().find_map(|op| match op {
                TransformQuatOp::YawFollow { threshold, .. } => Some(*threshold),
                _ => None,
            })
        })
        .unwrap()
}

#[test]
fn detailed_collision_adds_torso_spheres_that_mirror_the_capsule() {
    use crate::engine::ecs::component::{CollidableComponent, ZoneComponent};
    let source = std::fs::read_to_string("examples/twilight_rooftop.mms").unwrap();
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(&source, "examples/twilight_rooftop.mms")
        .unwrap();
    let avatar = named(&scene, "remote_avatar");
    let head = named(&scene, "remote_head_target");
    hold_head(&mut scene, head, 1.644, 30);
    let world = &scene.world;
    let avc = world
        .get_component_by_id_as::<AvatarControlComponent>(avatar)
        .unwrap();
    let capsule = world
        .get_component_by_id_as::<CollidableComponent>(avc.capsule_collidable_id.unwrap())
        .unwrap();
    assert_eq!(avc.detail_collidable_ids.len(), 3);
    for (&sphere, slot) in avc.detail_collidable_ids.iter().zip([
        HumanoidSlot::Hips,
        HumanoidSlot::Chest,
        HumanoidSlot::UpperChest,
    ]) {
        let collider = world
            .get_component_by_id_as::<CollidableComponent>(sphere)
            .unwrap();
        assert_eq!(collider.movement_target_id, capsule.movement_target_id);
        assert_eq!(collider.response, capsule.response);
        assert_eq!(collider.enabled, capsule.enabled);
        assert_eq!(collider.query_owner, Some(avatar));
        let zone = world.parent_of(sphere).unwrap();
        assert!(matches!(
            world.get_component_by_id_as::<ZoneComponent>(zone).unwrap().shape,
            crate::engine::ecs::component::CollisionShape::Sphere { radius } if radius > 0.05 && radius < 0.3
        ));
        let frame = world.parent_of(zone).unwrap();
        assert_eq!(world.parent_of(frame), Some(bone(&scene, avatar, slot)));
    }
}

#[test]
fn simple_collision_adds_no_spheres() {
    let source = std::fs::read_to_string("examples/twilight_rooftop.mms").unwrap();
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(&source, "examples/twilight_rooftop.mms")
        .unwrap();
    let avatar = named(&scene, "remote_avatar");
    scene
        .world
        .get_component_by_id_as_mut::<AvatarControlComponent>(avatar)
        .unwrap()
        .collision_detail = CollisionDetail::Simple;
    let head = named(&scene, "remote_head_target");
    hold_head(&mut scene, head, 1.644, 30);
    let avc = scene
        .world
        .get_component_by_id_as::<AvatarControlComponent>(avatar)
        .unwrap();
    assert!(avc.capsule_collidable_id.is_some());
    assert!(avc.detail_collidable_ids.is_empty());
}

#[test]
fn vertical_posture_never_leaves_vertical() {
    let source = std::fs::read_to_string("examples/twilight_rooftop.mms").unwrap();
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(&source, "examples/twilight_rooftop.mms")
        .unwrap();
    let avatar = named(&scene, "remote_avatar");
    scene
        .world
        .get_component_by_id_as_mut::<AvatarControlComponent>(avatar)
        .unwrap()
        .posture = BodyPosture::Vertical;
    let head = named(&scene, "remote_head_target");
    hold_head(&mut scene, head, 0.55, 90);
    assert_eq!(scene.systems.body_solver.mode(avatar), BodyMode::Vertical);
    assert_eq!(scene.systems.body_solver.torso_pitch(avatar), 0.0);
}

#[test]
fn crawling_forward_steps_the_knees_alternately() {
    let source = std::fs::read_to_string("examples/twilight_rooftop.mms").unwrap();
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(&source, "examples/twilight_rooftop.mms")
        .unwrap();
    let avatar = named(&scene, "remote_avatar");
    let head = named(&scene, "remote_head_target");
    let root = named(&scene, "remote_root");
    hold_head(&mut scene, head, 1.644, 30);
    hold_head(&mut scene, head, 0.7, 90);
    assert_eq!(scene.systems.body_solver.mode(avatar), BodyMode::Horizontal);
    let settled = scene.systems.lower_body_ik.crawl_steps(avatar).unwrap();
    hold_head(&mut scene, head, 0.7, 60);
    assert_eq!(
        scene.systems.lower_body_ik.crawl_steps(avatar).unwrap(),
        settled,
        "holding still takes no steps"
    );

    // Crawl 0.8 m over 2 s by moving the locomotion root.
    let start = position(&scene, root);
    for frame in 1..=120 {
        let x = start[0] + 0.8 * frame as f32 / 120.0;
        scene.systems.rx.push_intent_now(
            root,
            IntentValue::UpdateTransform {
                component_id: root,
                translation: [x, start[1], start[2]],
                rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0; 3],
            },
        );
        hold_head(&mut scene, head, 0.7, 1);
        for slot in [HumanoidSlot::LeftFoot, HumanoidSlot::RightFoot] {
            let ankle = position(&scene, bone(&scene, avatar, slot));
            assert!(
                ankle[1] > ROOF - 0.02 && ankle[1] < ROOF + 0.2,
                "frame {frame}: ankle stays near the roof: {ankle:?}"
            );
        }
    }
    let steps = scene.systems.lower_body_ik.crawl_steps(avatar).unwrap();
    let taken = [steps[0] - settled[0], steps[1] - settled[1]];
    assert!(
        taken[0] >= 1 && taken[1] >= 1,
        "both knees stepped: {taken:?}"
    );
    assert!(
        taken[0].abs_diff(taken[1]) <= 1,
        "legs alternate: {taken:?}"
    );
    assert_eq!(scene.systems.body_solver.mode(avatar), BodyMode::Horizontal);
}

#[test]
fn standing_up_fades_the_torso_with_the_legs_and_regrounds_the_feet() {
    let source = std::fs::read_to_string("examples/twilight_rooftop.mms").unwrap();
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(&source, "examples/twilight_rooftop.mms")
        .unwrap();
    let avatar = named(&scene, "remote_avatar");
    let head = named(&scene, "remote_head_target");
    hold_head(&mut scene, head, 1.644, 60);
    // Head nearly on the floor: as low as lying down gets.
    hold_head(&mut scene, head, 0.15, 120);
    let lying = scene.systems.body_solver.torso_pitch(avatar);
    assert!(lying > 1.0, "{lying}");
    // While the blend runs out, the torso pitch fades with the weight
    // instead of snapping upright under still-kneeling legs.
    let mut previous = lying;
    for _ in 0..30 {
        hold_head(&mut scene, head, 1.644, 1);
        let weight = scene.systems.body_solver.weight(avatar);
        let pitch = scene.systems.body_solver.torso_pitch(avatar);
        assert!(
            pitch <= previous + 1e-4,
            "pitch only falls: {pitch} after {previous}"
        );
        if weight >= 0.5 {
            assert!(
                pitch > 0.4 * lying,
                "still pitched while kneeling: {pitch} at {weight}"
            );
        }
        previous = pitch;
    }
    assert_eq!(scene.systems.body_solver.torso_pitch(avatar), 0.0);
    hold_head(&mut scene, head, 1.644, 60);
    for slot in [HumanoidSlot::LeftFoot, HumanoidSlot::RightFoot] {
        let ankle = position(&scene, bone(&scene, avatar, slot));
        assert!(
            ankle[1] > ROOF,
            "ankle above the roof after standing: {ankle:?}"
        );
    }
}

#[test]
#[ignore]
fn measure_knee_depth_while_crawling() {
    for head_height in [0.6f32, 0.7, 0.8, 0.9] {
        let source = std::fs::read_to_string("examples/twilight_rooftop.mms").unwrap();
        let mut scene = Universe::new(World::default());
        scene
            .load_mms_source_at_path(&source, "examples/twilight_rooftop.mms")
            .unwrap();
        let avatar = named(&scene, "remote_avatar");
        let head = named(&scene, "remote_head_target");
        let root = named(&scene, "remote_root");
        hold_head(&mut scene, head, 1.644, 30);
        hold_head(&mut scene, head, head_height, 90);
        let thigh = scene
            .systems
            .upper_body_ik
            .spine(avatar)
            .unwrap()
            .thigh_length;
        let start = position(&scene, root);
        let (mut still_knee, mut still_hip) = (f32::MAX, f32::MAX);
        let (mut min_knee, mut max_knee, mut min_hip, mut max_hip) =
            (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        let mut swing_min_knee = f32::MAX;
        let mut planted_min_knee = f32::MAX;
        for frame in 0..=180 {
            if frame > 30 {
                let x = start[0] + 0.6 * (frame - 30) as f32 / 150.0;
                scene.systems.rx.push_intent_now(
                    root,
                    IntentValue::UpdateTransform {
                        component_id: root,
                        translation: [x, start[1], start[2]],
                        rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                        scale: [1.0; 3],
                    },
                );
            }
            hold_head(&mut scene, head, head_height, 1);
            let feet = scene.systems.lower_body_ik.feet(avatar).unwrap();
            for (i, (hip_slot, knee_slot)) in [
                (HumanoidSlot::LeftUpperLeg, HumanoidSlot::LeftLowerLeg),
                (HumanoidSlot::RightUpperLeg, HumanoidSlot::RightLowerLeg),
            ]
            .into_iter()
            .enumerate()
            {
                let knee = position(&scene, bone(&scene, avatar, knee_slot))[1] - ROOF;
                let hip = position(&scene, bone(&scene, avatar, hip_slot))[1] - ROOF;
                if frame <= 30 {
                    still_knee = still_knee.min(knee);
                    still_hip = still_hip.min(hip);
                } else {
                    min_knee = min_knee.min(knee);
                    max_knee = max_knee.max(knee);
                    min_hip = min_hip.min(hip);
                    max_hip = max_hip.max(hip);
                    if feet[i].phase == crate::engine::ecs::system::ik::lower_body::FootPhase::Swing
                    {
                        swing_min_knee = swing_min_knee.min(knee);
                    } else {
                        planted_min_knee = planted_min_knee.min(knee);
                    }
                }
            }
        }
        eprintln!(
            "MEASURE head {head_height:.2} pitch {:.2} | hip target {:.3} | still: hip {still_hip:.3} knee {still_knee:.3} | crawl: hip {min_hip:.3}..{max_hip:.3} knee {min_knee:.3}..{max_knee:.3} (planted min {planted_min_knee:.3}, swing min {swing_min_knee:.3})",
            scene.systems.body_solver.torso_pitch(avatar),
            thigh + 0.06,
        );
    }
}

#[test]
fn posture_changes_are_observable_events() {
    use crate::engine::ecs::{EventSignal, SignalKind};
    use std::sync::{Arc, Mutex};
    let source = std::fs::read_to_string("examples/twilight_rooftop.mms").unwrap();
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(&source, "examples/twilight_rooftop.mms")
        .unwrap();
    let avatar = named(&scene, "remote_avatar");
    let head = named(&scene, "remote_head_target");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    scene.systems.rx.add_handler_closure(
        SignalKind::PostureChanged,
        avatar,
        move |_, _, signal| {
            if let Some(EventSignal::PostureChanged {
                avatar,
                mode,
                previous,
                pose,
                reason,
            }) = &signal.event
            {
                sink.lock()
                    .unwrap()
                    .push((*avatar, *mode, *previous, *pose, *reason));
            }
        },
    );
    hold_head(&mut scene, head, 1.644, 30);
    hold_head(&mut scene, head, 0.7, 90);
    hold_head(&mut scene, head, 1.644, 90);
    let seen = seen.lock().unwrap().clone();
    assert_eq!(
        seen,
        vec![
            (
                avatar,
                "horizontal",
                "vertical",
                "hands_and_knees",
                "head_height"
            ),
            (avatar, "vertical", "horizontal", "standing", "head_height"),
        ]
    );
}

#[test]
fn low_hips_sit_back_behind_the_knees_instead_of_sinking_them() {
    let source = std::fs::read_to_string("examples/twilight_rooftop.mms").unwrap();
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(&source, "examples/twilight_rooftop.mms")
        .unwrap();
    let avatar = named(&scene, "remote_avatar");
    let head = named(&scene, "remote_head_target");
    let root = named(&scene, "remote_root");
    hold_head(&mut scene, head, 1.644, 30);
    // 0.6 m: the torso pitch is clamped and the hips sit about 10 cm below
    // one thigh length.
    hold_head(&mut scene, head, 0.6, 90);
    assert_eq!(scene.systems.body_solver.mode(avatar), BodyMode::Horizontal);
    let start = position(&scene, root);
    for frame in 0..=150 {
        let x = start[0] + 0.6 * frame as f32 / 150.0;
        scene.systems.rx.push_intent_now(
            root,
            IntentValue::UpdateTransform {
                component_id: root,
                translation: [x, start[1], start[2]],
                rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0; 3],
            },
        );
        hold_head(&mut scene, head, 0.6, 1);
        for (hip_slot, knee_slot) in [
            (HumanoidSlot::LeftUpperLeg, HumanoidSlot::LeftLowerLeg),
            (HumanoidSlot::RightUpperLeg, HumanoidSlot::RightLowerLeg),
        ] {
            let knee = position(&scene, bone(&scene, avatar, knee_slot));
            let hip = position(&scene, bone(&scene, avatar, hip_slot));
            assert!(
                knee[1] > ROOF + 0.03,
                "frame {frame}: knee above the roof: {knee:?}"
            );
            assert!(hip[1] < ROOF + 0.45, "hips really are low: {hip:?}");
        }
    }
}

#[test]
fn body_solver_flowchart_example_has_a_card_for_every_note() {
    let source = std::fs::read_to_string("examples/body-solver-flowchart.mms").unwrap();
    let notes =
        std::fs::read_to_string("assets/components/diagrams/body_solver_notes.mms").unwrap();
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(&source, "examples/body-solver-flowchart.mms")
        .unwrap();
    for _ in 0..3 {
        scene.update(1.0 / 60.0, &Default::default());
    }
    let ids: Vec<&str> = notes
        .lines()
        .filter_map(|line| line.strip_prefix("        ")?.strip_suffix(" = {"))
        .filter(|id| !id.starts_with(' '))
        .collect();
    assert_eq!(ids.len(), 20, "{ids:?}");
    for id in ids {
        let name = format!("body_card_{id}");
        assert!(
            scene
                .world
                .all_components()
                .any(|c| scene.world.component_label(c) == Some(name.as_str())),
            "missing card {name}"
        );
    }
    let arrows = scene
        .world
        .all_components()
        .filter(|&c| scene.world.component_label(c) == Some("body_flow_arrowhead"))
        .count();
    assert_eq!(arrows, 16, "every wire has an arrowhead");
}

#[test]
fn mounted_mitten_reacquires_ground_after_falling_from_the_roof() {
    use crate::engine::ecs::system::external_xr_frame::ExternalXrFrame;

    let mut scene = Universe::new(World::default());
    let path = "examples/twilight_rooftop.mms";
    scene
        .load_mms_source_at_path(&std::fs::read_to_string(path).unwrap(), path)
        .unwrap();
    let view = openxr::View {
        pose: openxr::Posef {
            position: openxr::Vector3f {
                x: 0.0,
                y: 1.65,
                z: 0.0,
            },
            ..openxr::Posef::IDENTITY
        },
        fov: Default::default(),
    };
    scene.systems.xr.set_external_frame(Some(ExternalXrFrame {
        views: Some([view; 2]),
        ..Default::default()
    }));
    let advance = |scene: &mut Universe, frames| {
        for _ in 0..frames {
            scene.update(1.0 / 30.0, &Default::default());
        }
    };
    let translate = |scene: &mut Universe, component_id, translation| {
        scene.command_queue.push_intent_now(
            component_id,
            IntentValue::UpdateTransform {
                component_id,
                translation,
                rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0; 3],
            },
        );
    };
    advance(&mut scene, 8);
    let avatar = named(&scene, "local_avatar");
    assert_eq!(scene.systems.body_solver.floor(avatar), Some(ROOF));
    let root = named(&scene, "local_root");
    let eye = position(&scene, named(&scene, "local_xr_camera_anchor"));
    let door = position(&scene, named(&scene, "pickup_entry_frame"));
    let start = position(&scene, root);
    let mut at_door = std::array::from_fn(|i| start[i] + door[i] - eye[i]);
    at_door[1] += 0.55;
    translate(&mut scene, root, at_door);
    advance(&mut scene, 2);

    let pointer = named(&scene, "local_left_pointer");
    let truck = named(&scene, "pickup_truck");
    let hit = scene.world.find_component(truck, "renderable").unwrap();
    assert!(scene.systems.attachment.try_mount_from_hit(
        &mut scene.world,
        pointer,
        hit,
        &mut scene.command_queue,
    ));
    advance(&mut scene, 5);
    // Move past the parapet, then let the truck's native gravity do the drop.
    let mut over_edge = scene
        .world
        .get_component_by_id_as::<TransformComponent>(truck)
        .unwrap()
        .translation();
    over_edge[0] += 38.0;
    translate(&mut scene, truck, over_edge);
    advance(&mut scene, 120);
    let head = bone(&scene, avatar, HumanoidSlot::Head);
    let neck = bone(&scene, avatar, HumanoidSlot::Neck);
    let neck_length = |scene: &Universe| {
        let head = position(scene, head);
        let neck = position(scene, neck);
        (0..3)
            .map(|i| (head[i] - neck[i]).powi(2))
            .sum::<f32>()
            .sqrt()
    };
    assert!(position(&scene, head)[1] < 2.0);
    assert_eq!(scene.systems.body_solver.floor(avatar), None);
    assert_eq!(scene.systems.body_solver.mode(avatar), BodyMode::Vertical);
    assert!(neck_length(&scene) < 0.15);
    assert!(scene.systems.attachment.try_dismount_for_pointer(
        &mut scene.world,
        pointer,
        &mut scene.command_queue,
    ));
    advance(&mut scene, 15);
    let floor = scene.systems.body_solver.floor(avatar).unwrap();
    assert!((floor + 0.23).abs() < 0.01, "ground surface at {floor}");
    assert_eq!(scene.systems.body_solver.mode(avatar), BodyMode::Vertical);
    assert!(neck_length(&scene) < 0.15);
}
