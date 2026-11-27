//! Body solver in a real scene: the twilight rooftop's remote avatar is driven
//! by a plain head target while no peer is connected, so lowering that target
//! stands in for a player getting down on hands and knees.
use crate::engine::Universe;
use crate::engine::ecs::component::{
    AvatarControlComponent, BodyPosture, CollisionDetail, HumanoidSlot, TransformComponent,
};
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::system::ik::BodyMode;
use crate::engine::ecs::system::ik::lower_body::FootPhase;
use crate::engine::ecs::system::ik::upper_body::UpperBodyStatus;
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, SignalEmitter, World};

const ROOF: f32 = 16.0;

fn twilight_rooftop_with_lumae() -> String {
    std::fs::read_to_string("examples/twilight_rooftop.mms")
        .unwrap()
        .replace(
            "let client_avatar = \"mitten\"",
            "let client_avatar = \"lumae\"",
        )
}

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
    let mut source = twilight_rooftop_with_lumae();
    source.push_str(
        r#"
        let flight_test = T { name = "flight_test" }
        on(flight_test, "DataEvent", fn(event) { remote.set_remote_flying(event == "Fly") })
        flight_test
        "#,
    );
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

    // Leave the head still at crawling height. The peer's flight state has
    // to override the local collision support even that close to the roof.
    let flight_test = named(&scene, "flight_test");
    scene.command_queue.push_event(
        flight_test,
        EventSignal::DataEvent {
            name: "Fly".into(),
            payload: None,
        },
    );
    hold_head(&mut scene, head, 0.7, 40);
    assert_eq!(scene.systems.body_solver.mode(avatar), BodyMode::Vertical);
    assert!(
        scene
            .systems
            .lower_body_ik
            .feet(avatar)
            .unwrap()
            .iter()
            .all(|foot| foot.phase == FootPhase::Airborne)
    );

    // Stand back up: blends out to vertical and the torso returns to rest.
    scene.command_queue.push_event(
        flight_test,
        EventSignal::DataEvent {
            name: "Land".into(),
            payload: None,
        },
    );
    hold_head(&mut scene, head, 1.644, 90);
    assert_eq!(scene.systems.body_solver.mode(avatar), BodyMode::Vertical);
    assert_eq!(scene.systems.body_solver.torso_pitch(avatar), 0.0);
    assert!(
        scene
            .systems
            .lower_body_ik
            .feet(avatar)
            .unwrap()
            .iter()
            .all(|foot| foot.phase != FootPhase::Airborne)
    );
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
    let source = twilight_rooftop_with_lumae();
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
    let source = twilight_rooftop_with_lumae();
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
    let source = twilight_rooftop_with_lumae();
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
    let source = twilight_rooftop_with_lumae();
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
    let source = twilight_rooftop_with_lumae();
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
        let source = twilight_rooftop_with_lumae();
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
    let source = twilight_rooftop_with_lumae();
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
    let source = twilight_rooftop_with_lumae();
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
fn crawling_with_a_head_drop_keeps_the_knees_below_the_hips() {
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(
            &twilight_rooftop_with_lumae(),
            "examples/twilight_rooftop.mms",
        )
        .unwrap();
    let avatar = named(&scene, "remote_avatar");
    let head = named(&scene, "remote_head_target");
    let root = named(&scene, "remote_root");
    hold_head(&mut scene, head, 1.644, 30);
    let legs = [
        [HumanoidSlot::LeftUpperLeg, HumanoidSlot::LeftLowerLeg],
        [HumanoidSlot::RightUpperLeg, HumanoidSlot::RightLowerLeg],
    ]
    .map(|leg| leg.map(|slot| bone(&scene, avatar, slot)));
    let start = position(&scene, root);
    let move_root = |scene: &mut Universe, distance: f32, scale: f32| {
        let mut translation = start;
        translation[2] -= distance * scale;
        scene.systems.rx.push_intent_now(
            root,
            IntentValue::UpdateTransform {
                component_id: root,
                translation,
                rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                scale: [scale; 3],
            },
        );
    };
    for scale in [1.0, 0.1] {
        move_root(&mut scene, 0.0, scale);
        hold_head(&mut scene, head, 1.644, 60);
        hold_head(&mut scene, head, 0.7, 60);
        for frame in 1..=90 {
            move_root(&mut scene, 0.006 * frame as f32, scale);
            hold_head(&mut scene, head, 0.7 - 0.006 * frame as f32, 1);
            assert_eq!(scene.systems.body_solver.mode(avatar), BodyMode::Horizontal);
            for leg in legs {
                let [hip, knee] = leg.map(|joint| position(&scene, joint));
                assert!(
                    knee[1] < hip[1],
                    "scale {scale}, frame {frame}: knee {knee:?} above hip {hip:?}"
                );
            }
        }
    }
}

#[test]
fn avatar_overlays_track_selected_gait_and_retained_floor() {
    use crate::engine::ecs::component::TextComponent;

    let mut source = twilight_rooftop_with_lumae();
    source.push_str("\nT { EditorUI.panels([{ panel = \"settings\" }]) Editor.active() }\n");
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(&source, "examples/twilight_rooftop.mms")
        .unwrap();
    let head = named(&scene, "remote_head_target");
    hold_head(&mut scene, head, 1.644, 20);
    scene
        .systems
        .editor_context
        .shared_state()
        .lock()
        .unwrap()
        .selected_component = Some(head);
    let support = named(&scene, "editor_settings_body_support_visibility");
    let walking = named(&scene, "editor_settings_walking_gait_visibility");
    let click_toggle = |scene: &mut Universe, button| {
        scene.command_queue.push_event(
            button,
            EventSignal::Click {
                raycaster: button,
                renderable: button,
                hit_point: [0.; 3],
                screen_pos_px: None,
            },
        );
    };
    let text = |scene: &Universe, name| {
        scene
            .world
            .get_component_by_id_as::<TextComponent>(named(scene, name))
            .unwrap()
            .text
            .clone()
    };
    let outline_beams = |scene: &Universe| {
        let outline = named(scene, "body_support_surface_outline");
        scene
            .world
            .children_of(outline)
            .iter()
            .filter(|&&id| {
                scene
                    .world
                    .get_component_by_id_as::<TransformComponent>(id)
                    .is_some_and(|transform| transform.transform.scale[2] > 0.)
            })
            .count()
    };
    click_toggle(&mut scene, support);
    click_toggle(&mut scene, walking);
    hold_head(&mut scene, head, 1.644, 12);
    let standing = text(&scene, "body_support_debug_readout");
    assert!(standing.contains("FLOOR 16.00 m | LIVE"), "{standing}");
    assert!(
        standing.is_ascii() && standing.lines().count() == 3,
        "{standing}"
    );
    let readout = named(&scene, "body_support_debug_readout");
    let label_transform = scene.world.parent_of(readout).unwrap();
    let actual_scale = scene
        .world
        .get_component_by_id_as::<TransformComponent>(label_transform)
        .unwrap()
        .transform
        .scale[0];
    let standing_height = scene
        .systems
        .upper_body_ik
        .spine(named(&scene, "remote_avatar"))
        .unwrap()
        .head_rest_height;
    assert!(
        (actual_scale - standing_height * 0.06).abs() < 1e-4,
        "font scale {actual_scale}"
    );
    assert_eq!(
        outline_beams(&scene),
        21,
        "12 bounds, 8 corner marks, and one probe"
    );
    let root = named(&scene, "remote_root");
    let start = position(&scene, root);
    let mut saw_swing = false;
    for frame in 1..=24 {
        scene.systems.rx.push_intent_now(
            root,
            IntentValue::UpdateTransform {
                component_id: root,
                translation: [start[0] + frame as f32 * 0.025, start[1], start[2]],
                rotation_quat_xyzw: [0., 0., 0., 1.],
                scale: [1.; 3],
            },
        );
        hold_head(&mut scene, head, 1.644, 1);
        let gait = text(&scene, "walking_gait_debug_readout");
        assert!(gait.contains("remote_avatar"), "{gait}");
        saw_swing |= gait.contains("effective/planned") && gait.contains("Body Z");
    }
    assert!(saw_swing, "gait readout shows a tracked swing");
    click_toggle(&mut scene, walking);
    hold_head(&mut scene, head, 0.7, 38);
    let crawl = text(&scene, "body_support_debug_readout");
    assert!(
        crawl.starts_with("CRAWL |") && crawl.contains("BASE ") && crawl.contains(" | HEAD +"),
        "{crawl}"
    );

    scene.systems.rx.push_intent_now(
        root,
        IntentValue::UpdateTransform {
            component_id: root,
            translation: [1000., ROOF, 0.],
            rotation_quat_xyzw: [0., 0., 0., 1.],
            scale: [1.; 3],
        },
    );
    hold_head(&mut scene, head, 0.7, 12);
    let remembered = text(&scene, "body_support_debug_readout");
    assert!(
        remembered.contains("FLOOR 16.00 m | REMEMBERED"),
        "{remembered}"
    );
    assert_eq!(
        outline_beams(&scene),
        0,
        "retained floor has no live outline"
    );
    click_toggle(&mut scene, support);
    hold_head(&mut scene, head, 0.7, 4);
    assert!(!scene.world.all_components().any(|id| matches!(
        scene.world.component_label(id),
        Some("body_support_debug_readout" | "walking_gait_debug_readout")
    )));
}

fn two_floor_posture_scene(avatar: &str, extra: &str) -> Universe {
    let mut source = r#"
        import { make_rooftop_player } from "../assets/components/characters/rooftop_player.mms"
        XR.off()
        T.position(0.0, 15.5, 0.0) { Zone.cube([8.0, 0.5, 8.0]) { Collidable.static() } }
        T.position(9.0, 15.25, 0.0) { Zone.cube([1.0, 0.25, 2.0]) { Collidable.static() } }
        T.position(10.5, 14.75, 0.0) { Zone.cube([0.5, 0.25, 2.0]) { Collidable.static() } }
        T.position(11.5, 14.25, 0.0) { Zone.cube([0.5, 0.25, 2.0]) { Collidable.static() } }
        T.position(20.0, 13.5, 0.0) { Zone.cube([8.0, 0.5, 8.0]) { Collidable.static() } }
        let remote = make_rooftop_player({
            prefix = "remote" avatar = "lumae" local_player = false use_xr = false use_desktop = false
            shield_mount = true
        })
        remote.root.update_transform([0.0, 16.0, 0.0])
        remote.root
        T.position(0.0, 18.0, 4.0) { C3D { Pointer { Raycast.event_driven().min_distance(0.75) {} } } }
        T { EditorUI.panels([{ panel = "settings" }]) Editor.active() }
    "#
    .replace("avatar = \"lumae\"", &format!("avatar = \"{avatar}\""));
    source.push_str(extra);
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(&source, "examples/twilight_rooftop.mms")
        .unwrap();
    scene
}

#[test]
fn crawl_reacquires_floor_after_landing_below_remembered_floor() {
    let mut scene = two_floor_posture_scene("lumae", "");
    let head = named(&scene, "remote_head_target");
    let avatar = named(&scene, "remote_avatar");
    hold_head(&mut scene, head, 1.644, 28);
    hold_head(&mut scene, head, 0.7, 54);
    assert_eq!(scene.systems.body_solver.mode(avatar), BodyMode::Horizontal);
    assert_eq!(scene.systems.body_solver.floor(avatar), Some(ROOF));

    let root = named(&scene, "remote_root");
    scene.systems.rx.push_intent_now(
        root,
        IntentValue::UpdateTransform {
            component_id: root,
            translation: [20.0, 14.0, 0.0],
            rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        },
    );
    // Arrive with the tracked head at standing height and the body still
    // pitched for crawl. The old floor would put the head below the exit.
    hold_head(&mut scene, head, 1.644, 12);
    assert_eq!(scene.systems.body_solver.floor(avatar), Some(14.0));
    assert_eq!(scene.systems.body_solver.mode(avatar), BodyMode::Vertical);
}

#[test]
fn scene_owned_collision_recovers_from_crawl_after_stair_drop() {
    let miku_model = std::path::Path::new(
        "assets/models/local/third_party/TDA_Hatsune_Miku/tda_miku_with_bones.glb",
    );
    for (model, standing_head) in [("lumae", 1.644), ("miku", 1.59)] {
        if model == "miku" && !miku_model.is_file() {
            continue;
        }
        let mut scene = two_floor_posture_scene(model, "");
        let head = named(&scene, "remote_head_target");
        let avatar = named(&scene, "remote_avatar");
        let root = named(&scene, "remote_root");
        // Always exercise the scene-owned collision path, including on hosts
        // that lack the optional Miku model.
        scene
            .world
            .get_component_by_id_as_mut::<AvatarControlComponent>(avatar)
            .unwrap()
            .collision_enabled = false;
        let move_root = |scene: &mut Universe, translation| {
            scene.systems.rx.push_intent_now(
                root,
                IntentValue::UpdateTransform {
                    component_id: root,
                    translation,
                    rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                    scale: [1.0; 3],
                },
            );
        };
        hold_head(&mut scene, head, standing_head, 28);
        hold_head(&mut scene, head, 0.68, 54);
        assert_eq!(
            scene.systems.body_solver.mode(avatar),
            BodyMode::Horizontal,
            "{model}"
        );
        assert_eq!(
            scene.systems.body_solver.floor(avatar),
            Some(ROOF),
            "{model}"
        );

        move_root(&mut scene, [10.5, 15.8, 0.0]);
        hold_head(&mut scene, head, 0.68, 3);
        move_root(&mut scene, [17.0, 14.7, 0.0]);
        hold_head(&mut scene, head, standing_head, 3);
        assert_eq!(
            scene.systems.body_solver.floor(avatar),
            Some(ROOF),
            "{model}: midair"
        );
        move_root(&mut scene, [20.0, 14.0, 0.0]);
        hold_head(&mut scene, head, standing_head, 16);
        assert_eq!(
            scene.systems.body_solver.floor(avatar),
            Some(14.0),
            "{model}: landed"
        );
        assert_eq!(
            scene.systems.body_solver.mode(avatar),
            BodyMode::Vertical,
            "{model}"
        );
        assert_eq!(
            position(&scene, root)[1],
            14.0,
            "{model}: locomotion stays authoritative"
        );
    }
}

#[test]
fn worn_shield_follows_torso_and_mms_reports_solved_posture() {
    use crate::engine::ecs::component::TextComponent;

    let mut scene = two_floor_posture_scene(
        "lumae",
        r#"
        import { energy_shield } from "../assets/components/energy_sheild.mms"
        let body = remote.avatar()
        let initial = body.posture_state()
        let label = Text { name = "posture_test_report" initial.mode + "/" + initial.pose }
        label
        on(body, "PostureChanged", fn(event) {
            let current = event.avatar.posture_state()
            label.set_text(event.previous + ">" + event.mode + "/" + event.pose
                + "/" + current.mode + "/" + current.pose)
        })
        let shield = energy_shield("posture_test_shield", 2.0, { worn = true })
        shield.update_transform([3.0, 0.0, 0.0], [Math.pi / 2.0, 0.0, 0.0])
        remote.shield_mount.attach(shield)
        shield.set_worn(true, true)
        let events = T { name = "posture_test_shield_events" }
        events
        on(events, "DataEvent", fn(event) {
            if event == "Hit" { shield.take_damage(1.0) }
            if event == "Swap" { remote.select_avatar("mitten", fn(message, success) {}) }
        })
        "#,
    );
    let head = named(&scene, "remote_head_target");
    let avatar = named(&scene, "remote_avatar");
    let mount = named(&scene, "remote_shield_mount");
    let events = named(&scene, "posture_test_shield_events");
    let report = named(&scene, "posture_test_report");
    let event = |scene: &mut Universe, name: &str| {
        scene.command_queue.push_event(
            events,
            EventSignal::DataEvent {
                name: name.into(),
                payload: None,
            },
        );
    };
    let text = |scene: &Universe| {
        scene
            .world
            .get_component_by_id_as::<TextComponent>(report)
            .unwrap()
            .text
            .clone()
    };
    let shield_axis = |scene: &mut Universe, height| {
        event(scene, "Hit");
        hold_head(scene, head, height, 2);
        let shell = named(scene, "posture_test_shield_shell");
        let proxy = named(scene, "posture_test_shield_bullet_proxy");
        let axis = |id| {
            let column = TransformSystem::world_model(&scene.world, id).unwrap()[1];
            crate::utils::math::vec3_normalize([column[0], column[1], column[2]])
        };
        assert_eq!(position(scene, shell), position(scene, mount));
        assert_eq!(position(scene, proxy), position(scene, mount));
        let up = axis(shell);
        assert!(crate::utils::math::vec3_dot(up, axis(proxy)) > 0.999);
        up
    };

    hold_head(&mut scene, head, 1.644, 28);
    assert_eq!(
        scene.world.parent_of(mount),
        Some(bone(&scene, avatar, HumanoidSlot::UpperChest))
    );
    assert_eq!(text(&scene), "vertical/standing");
    assert!(shield_axis(&mut scene, 1.644)[1] > 0.999);
    hold_head(&mut scene, head, 0.68, 60);
    assert_eq!(
        text(&scene),
        "vertical>horizontal/hands_and_knees/horizontal/hands_and_knees"
    );
    assert!(shield_axis(&mut scene, 0.68)[1].abs() < 0.5);
    hold_head(&mut scene, head, 1.644, 40);
    assert_eq!(
        text(&scene),
        "horizontal>vertical/standing/vertical/standing"
    );
    assert!(shield_axis(&mut scene, 1.644)[1] > 0.999);

    let shield = named(&scene, "posture_test_shield");
    event(&mut scene, "Swap");
    for _ in 0..40 {
        scene.update(1.0 / 60.0, &Default::default());
    }
    let next_avatar = named(&scene, "remote_avatar");
    assert_ne!(next_avatar, avatar);
    assert_eq!(named(&scene, "remote_shield_mount"), mount);
    assert_eq!(scene.world.parent_of(shield), Some(mount));
    let spine = scene.systems.upper_body_ik.spine(next_avatar).unwrap();
    assert_eq!(
        scene.world.parent_of(mount),
        Some(spine.bones[spine.count - 1])
    );
}

#[test]
fn paw_capture_hud_marks_the_displayed_posture_snapshot() {
    use crate::engine::ecs::component::{DataComponent, DataValue, RenderableComponent};
    use crate::engine::ecs::system::BvhSystem;

    let mut scene = two_floor_posture_scene("lumae", "");
    let head = named(&scene, "remote_head_target");
    hold_head(&mut scene, head, 1.644, 24);
    scene
        .systems
        .editor_context
        .shared_state()
        .lock()
        .unwrap()
        .selected_component = Some(head);
    let click = |scene: &mut Universe, target| {
        scene.command_queue.push_event(
            target,
            EventSignal::Click {
                raycaster: target,
                renderable: target,
                hit_point: [0.0; 3],
                screen_pos_px: None,
            },
        );
    };
    let exists = |scene: &Universe, name| {
        scene
            .world
            .all_components()
            .any(|id| scene.world.component_label(id) == Some(name))
    };
    assert!(!exists(&scene, "paw_capture_hud"));
    scene.visuals.set_xr_camera(Vec::new());
    let toggle = named(&scene, "editor_settings_paw_capture_visibility");
    click(&mut scene, toggle);
    hold_head(&mut scene, head, 1.644, 2);
    assert!(!exists(&scene, "body_support_debug_readout"));
    let hud = named(&scene, "paw_capture_hud");
    let button = named(&scene, "paw_capture_button");
    let hit = BvhSystem::find_raycastable_for_renderable(&scene.world, button).unwrap();
    assert!(hit.pointer_events.captures_click() && !hit.pointer_events.captures_drag());
    let origin = [0.0, 18.0, 4.0];
    let offset = crate::utils::math::vec3_sub(position(&scene, button), origin);
    let distance = crate::utils::math::vec3_len(offset);
    assert!((offset[2] + 2.0).abs() < 0.001);
    let hits = scene.systems.bvh.raycast_renderables_candidates(
        origin,
        offset.map(|axis| axis / distance),
        distance + 0.1,
        0,
    );
    assert!(hits.iter().any(|(id, _)| *id == button));
    let hud_pads = scene
        .world
        .owned_subtree(hud)
        .into_iter()
        .filter(|&id| {
            id != button
                && scene
                    .world
                    .get_component_by_id_as::<RenderableComponent>(id)
                    .is_some()
        })
        .collect::<Vec<_>>();
    assert_eq!(hud_pads.len(), 5);
    assert!(
        hud_pads
            .iter()
            .all(|&id| !BvhSystem::renderable_is_raycastable(&scene.world, id))
    );

    scene
        .systems
        .editor_context
        .shared_state()
        .lock()
        .unwrap()
        .active_editor = None;
    click(&mut scene, button);
    // Change tracked height in the click's update. The capture belongs to the
    // last published frame rather than the new decision inputs.
    hold_head(&mut scene, head, 0.7, 1);
    let marker = named(&scene, "paw_capture_marker");
    let record = scene
        .world
        .children_of(marker)
        .iter()
        .find_map(|&id| scene.world.get_component_by_id_as::<DataComponent>(id))
        .unwrap();
    assert_eq!(
        record.get("origin"),
        Some(&DataValue::Text("by_paw".into()))
    );
    let Some(DataValue::Text(snapshot)) = record.get("snapshot") else {
        panic!("missing paw snapshot");
    };
    let snapshot: serde_json::Value = serde_json::from_str(snapshot).unwrap();
    assert_eq!(snapshot["avatar"], "remote_avatar");
    assert_eq!(snapshot["posture"], "standing");
    assert!(snapshot["decision_reason"].as_str().is_some());
    assert_eq!(snapshot["floor_source"], "live");
    assert!((snapshot["floor_y"].as_f64().unwrap() - f64::from(ROOF)).abs() < 0.001);
    assert!(snapshot["head"][1].as_f64().unwrap() > f64::from(ROOF) + 1.2);
    let rgba = snapshot["color"].as_array().unwrap();
    assert!(rgba[..3].iter().any(|channel| channel == &1.0));
    assert!(
        rgba[..3]
            .iter()
            .any(|channel| channel.as_f64().unwrap() < 0.2)
    );
    let marker_meshes = scene
        .world
        .owned_subtree(marker)
        .into_iter()
        .filter_map(|id| {
            let renderable = scene
                .world
                .get_component_by_id_as::<RenderableComponent>(id)?;
            assert!(!BvhSystem::renderable_is_raycastable(&scene.world, id));
            let frame = TransformSystem::world_model(&scene.world, id).unwrap();
            assert!(frame[3][1] > ROOF + 0.07, "buried paw pad: {frame:?}");
            assert!(
                frame[2][2] > 0.0 && frame[2][1].abs() < 1e-5,
                "paw faces the view"
            );
            Some(renderable.renderable.mesh)
        })
        .collect::<Vec<_>>();
    assert_eq!(marker_meshes.len(), 5);
    for id in hud_pads {
        let pad = scene
            .world
            .get_component_by_id_as::<RenderableComponent>(id)
            .unwrap();
        assert!(marker_meshes.contains(&pad.renderable.mesh));
    }

    click(&mut scene, toggle);
    hold_head(&mut scene, head, 0.7, 1);
    assert!(!exists(&scene, "paw_capture_hud"));
    assert!(exists(&scene, "paw_capture_marker"));
}

#[test]
fn downed_pose_releases_the_crawl_ankle_rotation() {
    use crate::engine::ecs::component::BoneRestPoseComponent;

    let mut source = twilight_rooftop_with_lumae();
    source.push_str(
        r#"
        import { lie_down } from "../assets/components/characters/downed.mms"
        let down_test = T { name = "down_test" }
        on(down_test, "DataEvent", fn(event) { lie_down(remote, "back", null) })
        down_test
    "#,
    );
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(&source, "examples/twilight_rooftop.mms")
        .unwrap();
    let avatar = named(&scene, "remote_avatar");
    let head = named(&scene, "remote_head_target");
    hold_head(&mut scene, head, 1.644, 30);
    hold_head(&mut scene, head, 0.7, 90);
    let feet =
        [HumanoidSlot::LeftFoot, HumanoidSlot::RightFoot].map(|slot| bone(&scene, avatar, slot));
    let dot_with_rest = |scene: &Universe, foot| {
        let rest = scene
            .world
            .children_of(foot)
            .iter()
            .find_map(|&child| {
                scene
                    .world
                    .get_component_by_id_as::<BoneRestPoseComponent>(child)
            })
            .unwrap()
            .rotation;
        let rotation = scene
            .world
            .get_component_by_id_as::<TransformComponent>(foot)
            .unwrap()
            .transform
            .rotation;
        rotation
            .into_iter()
            .zip(rest)
            .map(|(a, b)| a * b)
            .sum::<f32>()
            .abs()
    };
    for foot in feet {
        assert!(
            dot_with_rest(&scene, foot) < 0.99,
            "crawl rotates the ankle"
        );
    }
    scene.command_queue.push_event(
        named(&scene, "down_test"),
        EventSignal::DataEvent {
            name: "Down".into(),
            payload: None,
        },
    );
    scene.update(1.0 / 60.0, &Default::default());
    for foot in feet {
        assert!(
            dot_with_rest(&scene, foot) > 0.999,
            "downed ankle uses its rest rotation"
        );
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
