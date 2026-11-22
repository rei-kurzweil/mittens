//! Manual rig report for the optional local TDA Miku GLB, which is not in the
//! repository. On a machine that has it:
//!
//!   cargo test --release miku_rig_report -- --ignored --nocapture > miku_rig.txt
//!
//! Loads the twilight rooftop with both players on Miku and prints, per
//! avatar: every humanoid slot's resolution, the upper/lower body and posture
//! status, and the skeleton tree with each name's raw bytes, so missing or
//! mis-encoded slots (upper chest, fingers, toes) can be mapped by hand.
use super::rocket_motion_tests::Uploader;
use crate::engine::Universe;
use crate::engine::ecs::component::{
    AvatarControlComponent, GLTFComponent, HumanoidSlot, TransformComponent,
};
use crate::engine::ecs::system::HumanoidSlotStatus;
use crate::engine::ecs::{ComponentId, World};

const MIKU_GLB: &str = "assets/models/local/third_party/TDA_Hatsune_Miku/tda_miku_with_bones.glb";

fn named(scene: &Universe, name: &str) -> Option<ComponentId> {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(name))
}

fn label(world: &World, id: ComponentId) -> String {
    let name = world.component_label(id).unwrap_or("");
    let bytes: Vec<String> = name.bytes().map(|b| format!("{b:02x}")).collect();
    format!("{name:?} [{}]", bytes.join(" "))
}

fn print_tree(world: &World, id: ComponentId, depth: usize, printed: &mut usize) {
    if *printed > 600 {
        return;
    }
    let is_transform = world.get_component_by_id_as::<TransformComponent>(id).is_some();
    let named = world.component_label(id).is_some_and(|n| !n.is_empty());
    let next = if is_transform && named {
        println!("{}{}", "  ".repeat(depth), label(world, id));
        *printed += 1;
        depth + 1
    } else {
        depth
    };
    for &child in world.children_of(id) {
        print_tree(world, child, next, printed);
    }
}

#[test]
#[ignore = "needs the local TDA Miku GLB; prints a report instead of asserting"]
fn miku_rig_report() {
    if !std::path::Path::new(MIKU_GLB).exists() {
        println!("missing {MIKU_GLB}");
        return;
    }
    let path = "examples/twilight_rooftop.mms";
    let source = std::fs::read_to_string(path)
        .unwrap()
        .replace("let host_avatar = \"mitten\"", "let host_avatar = \"miku\"")
        .replace("let client_avatar = \"mitten\"", "let client_avatar = \"miku\"")
        .replace("let use_xr = true", "let use_xr = false")
        .replace("let use_desktop = false", "let use_desktop = true");
    let mut scene = Universe::new(World::default());
    scene.load_mms_source_at_path(&source, path).unwrap();
    let mut uploader = Uploader::default();
    let avatars: Vec<(&str, ComponentId)> = ["local_avatar", "remote_avatar"]
        .into_iter()
        .filter_map(|name| named(&scene, name).map(|id| (name, id)))
        .collect();
    let gltf_of = |scene: &Universe, avatar: ComponentId| {
        scene
            .world
            .get_component_by_id_as::<AvatarControlComponent>(avatar)
            .and_then(|avc| avc.humanoid_map_gltf)
    };
    let spawned = |scene: &Universe| {
        avatars.iter().all(|&(_, avatar)| {
            gltf_of(scene, avatar).is_some_and(|gltf| {
                scene
                    .world
                    .get_component_by_id_as::<GLTFComponent>(gltf)
                    .is_some_and(|g| g.spawned)
            })
        })
    };
    let mut frames = 0;
    while frames < 600 && !spawned(&scene) {
        scene.systems.prepare_render(
            &mut scene.world,
            &mut scene.visuals,
            &mut scene.render_assets,
            &mut uploader,
            &mut scene.command_queue,
        );
        scene.update(1.0 / 60.0, &Default::default());
        frames += 1;
    }
    // Let binding, calibration and the first solves settle.
    for _ in 0..60 {
        scene.update(1.0 / 60.0, &Default::default());
    }
    println!("spawned after {frames} frames: {}", spawned(&scene));
    for &(name, avatar) in &avatars {
        println!("\n===== {name} =====");
        println!("upper body: {:?}", scene.systems.upper_body_ik.status(avatar));
        println!("lower body: {:?}", scene.systems.lower_body_ik.status(avatar));
        println!(
            "posture: {:?} floor {:?}",
            scene.systems.body_solver.mode(avatar),
            scene.systems.body_solver.floor(avatar)
        );
        let Some(gltf) = gltf_of(&scene, avatar) else {
            println!("no humanoid GLTF bound");
            continue;
        };
        let Some(report) = scene.systems.humanoid_bone_map.report(gltf) else {
            println!("no humanoid map report");
            continue;
        };
        println!("map valid: {} diagnostics: {:?}", report.valid, report.diagnostics);
        println!(
            "head ready {} arms ready {}/{} lower body ready {}",
            report.head_ready(),
            report.arm_ready(true),
            report.arm_ready(false),
            report.lower_body_ready()
        );
        for slot in HumanoidSlot::ALL {
            let Some(entry) = report.slot(slot) else {
                println!("  {slot:?}: no entry");
                continue;
            };
            let status = match &entry.status {
                HumanoidSlotStatus::Resolved(target) => {
                    let parent = scene
                        .world
                        .parent_of(target.component)
                        .map(|p| label(&scene.world, p))
                        .unwrap_or_default();
                    format!(
                        "resolved {} ({:?}) parent {parent}",
                        label(&scene.world, target.component),
                        target.kind
                    )
                }
                other => format!("{other:?}"),
            };
            println!(
                "  {slot:?}: {status} via {:?}{}",
                entry.provenance,
                entry
                    .diagnostic
                    .as_ref()
                    .map(|d| format!(" - {d}"))
                    .unwrap_or_default()
            );
        }
        println!("--- skeleton (named transforms under the GLTF) ---");
        let mut printed = 0;
        print_tree(&scene.world, gltf, 0, &mut printed);
    }
}
