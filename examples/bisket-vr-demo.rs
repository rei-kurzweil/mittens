use cat_engine::engine::ecs::component::{
    AvatarControlComponent, ColorComponent, EmissiveComponent, OverlayComponent,
    RaycastableComponent, RenderableComponent, TransformComponent,
};
use cat_engine::engine::ecs::{ComponentId, EventSignal, Signal, SignalEmitter, SignalKind, World};
use cat_engine::utils::math::{mat_to_quat, quat_rotate_vec3, quat_rotation_y};
use cat_engine::{engine, meow_meow, utils};
use std::sync::OnceLock;

// Bones the dump-button reports.  Spine first, then arms — same ordering as
// the bisket-vr-debug harness so the two outputs are comparable.
const DUMP_BONES: &[&str] = &[
    "J_Bip_C_Hips",
    "J_Bip_C_Spine",
    "J_Bip_C_Chest",
    "J_Bip_C_UpperChest",
    "J_Bip_C_Neck",
    "J_Bip_C_Head",
    "J_Bip_L_UpperArm",
    "J_Bip_L_LowerArm",
    "J_Bip_L_Hand",
    "J_Bip_R_UpperArm",
    "J_Bip_R_LowerArm",
    "J_Bip_R_Hand",
];

struct DumpTargets {
    /// (label, bone_component_id), filled at startup once the armature exists.
    bones: Vec<(String, ComponentId)>,
    /// driven_t = parent of AVC — HMD pose source.
    driven_t: ComponentId,
    /// splice_head — runtime AimConstraint sink (parent of head_bone).
    splice_head: ComponentId,
    /// Head bone for head-local camera offset checks.
    head_bone: ComponentId,
    /// Upper chest (if found) for torso tilt diagnostics.
    upper_chest: Option<ComponentId>,
    /// Transform wrapper parent of CameraXR (if authored as T { CXR }).
    camera_wrapper_t: Option<ComponentId>,
    /// Authored camera offset in head-local frame from the wrapper transform.
    authored_eye_offset_head_local: [f32; 3],
    /// OpenXR vs avatar forward-axis mapping used by AVC.
    head_ik_offset_yaw: f32,
}

static DUMP_TARGETS: OnceLock<DumpTargets> = OnceLock::new();

fn world_pos(world: &World, id: ComponentId) -> [f32; 3] {
    world
        .get_component_by_id_as::<TransformComponent>(id)
        .map(|t| {
            let m = t.transform.matrix_world;
            [m[3][0], m[3][1], m[3][2]]
        })
        .unwrap_or([0.0; 3])
}

fn world_rot_quat(world: &World, id: ComponentId) -> [f32; 4] {
    world
        .get_component_by_id_as::<TransformComponent>(id)
        .map(|t| mat_to_quat(t.transform.matrix_world))
        .unwrap_or([0.0, 0.0, 0.0, 1.0])
}

fn v3_len(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn deg(rad: f32) -> f32 {
    rad * (180.0 / std::f32::consts::PI)
}

fn on_dump_click(world: &mut World, _emit: &mut dyn SignalEmitter, env: &Signal) {
    if !matches!(env.event.as_ref(), Some(EventSignal::Click { .. })) {
        return;
    }
    let Some(targets) = DUMP_TARGETS.get() else {
        return;
    };

    let driven_p = world_pos(world, targets.driven_t);
    let driven_q = world_rot_quat(world, targets.driven_t);
    let splice_p = world_pos(world, targets.splice_head);
    let head_p = world_pos(world, targets.head_bone);
    let head_q = world_rot_quat(world, targets.head_bone);

    // Expected head pivot from HMD pose + authored eye offset convention.
    let neg_eye = [
        -targets.authored_eye_offset_head_local[0],
        -targets.authored_eye_offset_head_local[1],
        -targets.authored_eye_offset_head_local[2],
    ];
    let head_target_offset_local =
        quat_rotate_vec3(quat_rotation_y(targets.head_ik_offset_yaw), neg_eye);
    let head_target_offset_world = quat_rotate_vec3(driven_q, head_target_offset_local);
    let expected_splice = [
        driven_p[0] + head_target_offset_world[0],
        driven_p[1] + head_target_offset_world[1],
        driven_p[2] + head_target_offset_world[2],
    ];
    let splice_err = [
        splice_p[0] - expected_splice[0],
        splice_p[1] - expected_splice[1],
        splice_p[2] - expected_splice[2],
    ];

    // Camera-wrapper vs head local-offset consistency check.
    let mut cam_rel_report = String::from("camera_wrapper: <none>");
    if let Some(cam_t) = targets.camera_wrapper_t {
        let cam_p = world_pos(world, cam_t);
        let expected_cam_world_off =
            quat_rotate_vec3(head_q, targets.authored_eye_offset_head_local);
        let expected_cam = [
            head_p[0] + expected_cam_world_off[0],
            head_p[1] + expected_cam_world_off[1],
            head_p[2] + expected_cam_world_off[2],
        ];
        let cam_err = [
            cam_p[0] - expected_cam[0],
            cam_p[1] - expected_cam[1],
            cam_p[2] - expected_cam[2],
        ];
        cam_rel_report = format!(
            "camera_wrapper  pos=[{:+.3},{:+.3},{:+.3}]  expected_from_head=[{:+.3},{:+.3},{:+.3}]  err=[{:+.3},{:+.3},{:+.3}] |err|={:.4}m",
            cam_p[0],
            cam_p[1],
            cam_p[2],
            expected_cam[0],
            expected_cam[1],
            expected_cam[2],
            cam_err[0],
            cam_err[1],
            cam_err[2],
            v3_len(cam_err)
        );
    }

    // Torso tilt diagnostic (forward pitch of upper chest).
    let mut torso_report = String::from("upper_chest_tilt: <not found>");
    if let Some(upper_chest) = targets.upper_chest {
        let chest_q = world_rot_quat(world, upper_chest);
        let fwd = quat_rotate_vec3(chest_q, [0.0, 0.0, 1.0]);
        let pitch_deg = deg(fwd[1].atan2((fwd[0] * fwd[0] + fwd[2] * fwd[2]).sqrt()));
        torso_report = format!(
            "upper_chest_fwd=[{:+.3},{:+.3},{:+.3}] pitch={:+.2}deg",
            fwd[0], fwd[1], fwd[2], pitch_deg
        );
    }

    println!("\n================ BONE DUMP (click) ================");
    println!(
        "driven_t  pos=[{:+.3},{:+.3},{:+.3}]  rot_xyzw=[{:+.3},{:+.3},{:+.3},{:+.3}]",
        driven_p[0], driven_p[1], driven_p[2], driven_q[0], driven_q[1], driven_q[2], driven_q[3],
    );
    println!(
        "splice_head  pos=[{:+.3},{:+.3},{:+.3}]  expected=[{:+.3},{:+.3},{:+.3}]  err=[{:+.3},{:+.3},{:+.3}] |err|={:.4}m",
        splice_p[0],
        splice_p[1],
        splice_p[2],
        expected_splice[0],
        expected_splice[1],
        expected_splice[2],
        splice_err[0],
        splice_err[1],
        splice_err[2],
        v3_len(splice_err),
    );
    println!("{}", cam_rel_report);
    println!("{}", torso_report);
    println!(
        "\n  {:<22}  {:>7}  {:>7}  {:>7}     {:>+7}  {:>+7}  {:>+7}",
        "bone", "x", "y", "z", "Δx_to_drv", "Δy_to_drv", "Δz_to_drv"
    );
    let mut prev_pos: Option<[f32; 3]> = None;
    for (i, (label, bone_id)) in targets.bones.iter().enumerate() {
        let p = world_pos(world, *bone_id);
        println!(
            "  {:<22}  {:>+7.3}  {:>+7.3}  {:>+7.3}     {:>+7.3}  {:>+7.3}  {:>+7.3}",
            label,
            p[0],
            p[1],
            p[2],
            p[0] - driven_p[0],
            p[1] - driven_p[1],
            p[2] - driven_p[2],
        );
        // Segment length to previous bone (only meaningful within a chain —
        // spine bones are contiguous; arm bones are split per side, so we
        // insert a blank gap when the chain breaks).
        if let Some(prev) = prev_pos {
            let is_spine = label.starts_with("J_Bip_C_");
            let prev_was_spine = targets.bones[i - 1].0.starts_with("J_Bip_C_");
            let is_left_arm_step =
                label.starts_with("J_Bip_L_") && targets.bones[i - 1].0.starts_with("J_Bip_L_");
            let is_right_arm_step =
                label.starts_with("J_Bip_R_") && targets.bones[i - 1].0.starts_with("J_Bip_R_");
            if (is_spine && prev_was_spine) || is_left_arm_step || is_right_arm_step {
                let d = [(p[0] - prev[0]), (p[1] - prev[1]), (p[2] - prev[2])];
                let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                println!("    └─ segment from prev: len={:.4}m", len);
            }
        }
        prev_pos = Some(p);
    }
    println!("====================================================\n");
}

fn main() {
    utils::logger::init();

    let output = meow_meow::MeowMeowRunner::eval(include_str!("bisket-vr-demo.mms"));

    for error in &output.errors {
        eprintln!("[mms] {error}");
    }
    println!(
        "[mms] {} intent(s) from bisket-vr-demo.mms",
        output.intents.len()
    );

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let scope = engine::ecs::ComponentId::default();
    for intent in output.intents {
        universe.command_queue.push_intent_now(scope, intent);
    }

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &universe.render_assets,
        &mut universe.command_queue,
    );

    // Force the glTF subtree to spawn so the armature exists before we query for
    // bone markers (otherwise the bone components aren't in the world yet).
    {
        let systems = &mut universe.systems;
        systems.gltf.tick_with_queue(
            &mut universe.world,
            &mut universe.visuals,
            &mut systems.skinned_mesh,
            &mut universe.command_queue,
            0.0,
        );
    }
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &universe.render_assets,
        &mut universe.command_queue,
    );

    // Bone markers — small cubes attached as children of named bones, so they
    // visibly track the bone's world pose. Useful for debugging where each
    // bone actually sits relative to the XR camera and the overlay cube.
    let marker_joints: &[(&str, (f32, f32, f32, f32))] = &[
        ("[name='J_Bip_C_Head']", (0.85, 0.20, 0.85, 0.9)),
        ("[name='J_Bip_C_Neck']", (0.20, 0.85, 0.85, 0.9)),
        ("[name='J_Bip_C_UpperChest']", (0.20, 0.20, 0.85, 0.9)),
        ("[name='J_Sec_Hair1_08']", (1.00, 0.00, 0.00, 1.0)),
        ("[name='J_Sec_Hair2_08']", (1.00, 0.40, 0.40, 1.0)),
    ];

    // Find any global root to start the descendant search from. Scanning all
    // components is fine here — this only runs once at startup.
    let roots: Vec<engine::ecs::ComponentId> = universe
        .world
        .all_components()
        .filter(|&id| universe.world.parent_of(id).is_none())
        .collect();

    for &(selector, color) in marker_joints {
        let bone = roots
            .iter()
            .find_map(|&r| universe.find_component(r, selector));
        let Some(bone) = bone else {
            eprintln!("[bisket-vr-demo] bone not found: {selector}");
            continue;
        };
        // Topology:  bone -> OV -> marker_t -> marker_r (+C +EM +Raycastable)
        // OV is a phase marker: its subtree renders in the overlay pass (drawn
        // last, on top of everything). Without this the marker cube is occluded
        // by the avatar's head/body mesh in first-person VR.
        let marker_ov = universe.world.add_component(OverlayComponent::new());
        let marker_t = universe
            .world
            .add_component(TransformComponent::new().with_scale(0.025, 0.025, 0.025));
        let marker_r = universe.world.add_component(RenderableComponent::cube());
        let marker_c = universe
            .world
            .add_component(ColorComponent::rgba(color.0, color.1, color.2, color.3));
        let marker_rcast = universe
            .world
            .add_component(RaycastableComponent::enabled());
        let marker_em = universe.world.add_component(EmissiveComponent::on());
        let _ = universe.world.add_child(marker_r, marker_c);
        let _ = universe.world.add_child(marker_r, marker_em);
        let _ = universe.world.add_child(marker_r, marker_rcast);
        let _ = universe.world.add_child(marker_t, marker_r);
        let _ = universe.world.add_child(marker_ov, marker_t);
        let _ = universe.attach(bone, marker_ov);
    }

    // ----------------------------------------------------------------------
    // Dump button — click to print a bone snapshot for offline analysis.
    //
    // Topology: scene_root → OV → button_t → button_r (+ C, EM, Raycastable).
    // OV phase so the cube is visible through the avatar body in first-person VR.
    // Click handler walks the cached bone list, prints world pos + segment
    // lengths to stdout.  Compare with bisket-vr-debug harness output.
    // ----------------------------------------------------------------------
    {
        // Resolve AVC + driven_t for the dump.
        let avc_id_opt = universe.world.all_components().find(|&id| {
            universe
                .world
                .get_component_by_id_as::<AvatarControlComponent>(id)
                .is_some()
        });
        if let Some(avc_id) = avc_id_opt {
            let driven_t = universe.world.parent_of(avc_id).unwrap_or(avc_id);
            let head_ik_offset_yaw = universe
                .world
                .get_component_by_id_as::<AvatarControlComponent>(avc_id)
                .map(|c| {
                    if c.forward_plus_z {
                        0.0
                    } else {
                        std::f32::consts::PI
                    }
                })
                .unwrap_or(std::f32::consts::PI);

            // Find each bone under any root (model_root is unique per avatar here).
            let mut bones: Vec<(String, ComponentId)> = Vec::new();
            for name in DUMP_BONES {
                let sel = format!("[name='{}']", name);
                if let Some(bone) = roots.iter().find_map(|&r| universe.find_component(r, &sel)) {
                    bones.push((name.to_string(), bone));
                } else {
                    eprintln!("[dump] bone not found: {}", name);
                }
            }
            // splice_head = parent of head bone (runtime-inserted by AVC).
            let splice_head = bones
                .iter()
                .find(|(n, _)| n == "J_Bip_C_Head")
                .and_then(|(_, head)| universe.world.parent_of(*head))
                .unwrap_or(avc_id);

            let head_bone = bones
                .iter()
                .find(|(n, _)| n == "J_Bip_C_Head")
                .map(|(_, id)| *id)
                .unwrap_or(splice_head);

            let upper_chest = bones
                .iter()
                .find(|(n, _)| n == "J_Bip_C_UpperChest")
                .map(|(_, id)| *id);

            // Find CameraXR + its transform wrapper to recover authored eye offset.
            let cxr_id = universe.world.all_components().find(|&cid| {
                universe
                    .world
                    .get_component_by_id_as::<cat_engine::engine::ecs::component::CameraXRComponent>(cid)
                    .is_some()
            });
            let camera_wrapper_t = cxr_id.and_then(|cid| {
                universe.world.parent_of(cid).filter(|&p| {
                    universe
                        .world
                        .get_component_by_id_as::<TransformComponent>(p)
                        .is_some()
                })
            });
            let authored_eye_offset_head_local = camera_wrapper_t
                .and_then(|t| {
                    universe
                        .world
                        .get_component_by_id_as::<TransformComponent>(t)
                })
                .map(|t| t.transform.translation)
                .unwrap_or([0.0, 0.0, 0.0]);

            let _ = DUMP_TARGETS.set(DumpTargets {
                bones,
                driven_t,
                splice_head,
                head_bone,
                upper_chest,
                camera_wrapper_t,
                authored_eye_offset_head_local,
                head_ik_offset_yaw,
            });

            // Spawn the button.  Position chosen so it's reachable in-VR from
            // a standing pose: 0.35 m to the right, hip-height, slightly out.
            let btn_t = universe.world.add_component(
                TransformComponent::new()
                    .with_position(0.35, 1.05, -0.35)
                    .with_scale(0.06, 0.06, 0.06),
            );
            let btn_r = universe.world.add_component(RenderableComponent::cube());
            let btn_c = universe
                .world
                .add_component(ColorComponent::rgba(0.95, 0.20, 0.40, 1.0));
            let btn_em = universe.world.add_component(EmissiveComponent::on());
            let btn_rcast = universe
                .world
                .add_component(RaycastableComponent::enabled());
            let btn_ov = universe.world.add_component(OverlayComponent::new());
            let _ = universe.world.add_child(btn_r, btn_c);
            let _ = universe.world.add_child(btn_r, btn_em);
            let _ = universe.world.add_child(btn_r, btn_rcast);
            let _ = universe.world.add_child(btn_t, btn_r);
            let _ = universe.world.add_child(btn_ov, btn_t);

            // Attach to a stable scene root (the first global root we found).
            let scene_root = roots
                .iter()
                .copied()
                .find(|&r| {
                    universe
                        .world
                        .get_component_by_id_as::<TransformComponent>(r)
                        .is_some()
                        || universe.world.children_of(r).len() > 0
                })
                .unwrap_or_else(|| roots[0]);
            let _ = universe.attach(scene_root, btn_ov);

            // Wire Click handler.  Scope = the renderable (where the raycast hits).
            universe.add_signal_handler(SignalKind::Click, btn_r, on_dump_click);

            println!(
                "[dump] button spawned at [0.35, 1.05, -0.35] — click in VR or with desktop pointer to dump bones"
            );
        } else {
            eprintln!("[dump] AvatarControlComponent not found — skipping dump button");
        }
    }

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
