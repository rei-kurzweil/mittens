//! Body solver in a real scene: the twilight rooftop's remote avatar is driven
//! by a plain head target while no peer is connected, so lowering that target
//! stands in for a player getting down on hands and knees.
use crate::engine::Universe;
use crate::engine::ecs::component::{AvatarControlComponent, HumanoidSlot, TransformComponent};
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
}

#[test]
fn vertical_posture_never_leaves_vertical() {
    let source = std::fs::read_to_string("examples/twilight_rooftop.mms")
        .unwrap()
        .replace("posture(\"auto\")", "posture(\"vertical\")");
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(&source, "examples/twilight_rooftop.mms")
        .unwrap();
    let avatar = named(&scene, "remote_avatar");
    let head = named(&scene, "remote_head_target");
    hold_head(&mut scene, head, 0.55, 90);
    assert_eq!(scene.systems.body_solver.mode(avatar), BodyMode::Vertical);
    assert_eq!(scene.systems.body_solver.torso_pitch(avatar), 0.0);
}
