//! bisket-vr-debug — headless verification harness for head-driven AVC.
//!
//! See `examples/bisket-vr-debug.mms` for scene setup.
//!
//! What this binary does:
//!   1. Loads the scene (two bisket avatars: one bare-FK reference, one
//!      AVC-driven).
//!   2. Ticks the engine enough to spawn armatures and let AVC wire its
//!      splices + spine FABRIK chain.
//!   3. Scripts the AVC avatar's `driven_t` through a sequence of poses
//!      that simulate HMD movement (pitch, yaw, lean, walk).
//!   4. For each pose, samples world Y/Z of the spine bones on both
//!      avatars and prints a diff table to the console.
//!   5. Also checks a handful of invariants (bone-length preservation,
//!      monotonic Y from hips → head, splice_head landing near the
//!      offset-compensated target).
//!
//! Run with `cargo run --release --example bisket-vr-debug`.  No
//! windowing loop — exits after printing the report.

use cat_engine::engine::ecs::component::{AvatarControlComponent, TransformComponent};
use cat_engine::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use cat_engine::engine::user_input::InputState;
use cat_engine::utils::math::{quat_from_axis_angle, quat_rotate_vec3};
use cat_engine::{engine, meow_meow, utils};

// Spine bones sampled at each pose, ordered root → end.
const SPINE_BONES: &[&str] = &[
    "J_Bip_C_Hips",
    "J_Bip_C_Spine",
    "J_Bip_C_Chest",
    "J_Bip_C_UpperChest",
    "J_Bip_C_Neck",
    "J_Bip_C_Head",
];

// Tolerances (metres) for invariant checks.
const BONE_LENGTH_TOL: f32 = 0.005; // 5 mm
const SPLICE_TARGET_TOL: f32 = 0.010; // 1 cm
const MONOTONIC_Y_TOL: f32 = 0.005; // 5 mm

// Frames ticked per pose to let IK + transform propagation settle.
const SETTLE_TICKS_PER_POSE: usize = 8;

struct Pose {
    name: &'static str,
    description: &'static str,
    t: [f32; 3],
    rot: [f32; 4],
}

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // --- Load scene ---
    let source = include_str!("bisket-vr-debug.mms");
    let output = meow_meow::MeowMeowRunner::eval_with_world_at_path(
        source,
        Some("examples/bisket-vr-debug.mms"),
        &mut universe.world,
        &mut universe.systems.rx,
        &mut universe.command_queue,
    );
    for e in &output.errors {
        eprintln!("[mms] {e}");
    }
    println!("[mms] {} intent(s)", output.intents.len());
    for intent in output.intents {
        universe
            .command_queue
            .push_intent_now(ComponentId::default(), intent);
    }
    drain(&mut universe);

    // --- Tick GLTF so armatures spawn before we go looking for bones ---
    tick_gltf(&mut universe);
    drain(&mut universe);

    // --- Find AVC + the two avatars' subtrees ---
    let avc_id = universe
        .world
        .all_components()
        .find(|&id| {
            universe
                .world
                .get_component_by_id_as::<AvatarControlComponent>(id)
                .is_some()
        })
        .expect("AvatarControlComponent not found — scene didn't load?");
    let driven_t = universe
        .world
        .parent_of(avc_id)
        .expect("AVC has no parent (driven_t)");

    println!("[debug] avc_id={:?} driven_t={:?}", avc_id, driven_t);

    // Capture eye_offset BEFORE AVC init runs — once init fires, AVC reparents
    // the T-wrapped camera out from under itself (over to the head bone), so
    // querying `children_of(avc)` post-init returns no camera wrapper.
    let head_target_offset_world = head_target_offset_in_target_local(&universe.world, avc_id);
    println!(
        "[debug] head_target_offset (target-local) = {:?}",
        head_target_offset_world
    );

    // Settle a few frames so AVC init splices fire and FABRIK chain wires up.
    let input = InputState::default();
    for _ in 0..10 {
        universe.systems.tick(
            &mut universe.world,
            &mut universe.visuals,
            &universe.render_assets,
            &input,
            &mut universe.command_queue,
            1.0 / 60.0,
        );
        drain(&mut universe);
    }

    // --- Resolve bones on both avatars ---
    let mut ref_bones: Vec<ComponentId> = Vec::with_capacity(SPINE_BONES.len());
    let mut avc_bones: Vec<ComponentId> = Vec::with_capacity(SPINE_BONES.len());
    for name in SPINE_BONES {
        let all = find_components_by_name(&universe.world, name);
        let (under_avc, not_under_avc): (Vec<_>, Vec<_>) = all
            .into_iter()
            .partition(|&id| is_descendant_of(&universe.world, id, avc_id));
        let avc = under_avc
            .first()
            .copied()
            .unwrap_or_else(|| panic!("bone {} not found under AVC subtree", name));
        let r = not_under_avc
            .first()
            .copied()
            .unwrap_or_else(|| panic!("bone {} not found outside AVC subtree", name));
        ref_bones.push(r);
        avc_bones.push(avc);
    }
    println!(
        "[debug] resolved {} spine bones in both subtrees",
        SPINE_BONES.len()
    );

    // splice_head: the runtime-created TC injected between neck and head_bone.
    // It's the parent of head_bone in the AVC subtree.
    let avc_head_bone = avc_bones[SPINE_BONES
        .iter()
        .position(|n| *n == "J_Bip_C_Head")
        .unwrap()];
    let splice_head = universe
        .world
        .parent_of(avc_head_bone)
        .expect("head_bone has no parent — splice didn't run?");

    // --- Define poses to scrub through ---
    let poses = [
        Pose {
            name: "rest",
            description: "driven_t at HMD height (1.55m), identity rotation",
            t: [0.0, 1.55, 0.0],
            rot: ident_quat(),
        },
        Pose {
            name: "pitch_up_30",
            description: "head pitched up 30° around X",
            t: [0.0, 1.55, 0.0],
            rot: quat_from_axis_angle([1.0, 0.0, 0.0], 0.5236),
        },
        Pose {
            name: "pitch_dn_30",
            description: "head pitched down 30° around X",
            t: [0.0, 1.55, 0.0],
            rot: quat_from_axis_angle([1.0, 0.0, 0.0], -0.5236),
        },
        Pose {
            name: "yaw_right_45",
            description: "head yawed right 45° around Y",
            t: [0.0, 1.55, 0.0],
            rot: quat_from_axis_angle([0.0, 1.0, 0.0], 0.7854),
        },
        Pose {
            name: "lean_forward",
            description: "driven_t translated +0.2m on Z (head leans forward over toes)",
            t: [0.0, 1.45, 0.20],
            rot: ident_quat(),
        },
        Pose {
            name: "crouch",
            description: "driven_t lowered to 1.10m (player crouches)",
            t: [0.0, 1.10, 0.0],
            rot: ident_quat(),
        },
        Pose {
            name: "walk_forward_0.5m",
            description: "driven_t walks +0.5m on -Z (OpenXR forward = -Z)",
            t: [0.0, 1.55, -0.5],
            rot: ident_quat(),
        },
        Pose {
            name: "walk_strafe_right_0.3m",
            description: "driven_t side-step +0.3m on X",
            t: [0.3, 1.55, 0.0],
            rot: ident_quat(),
        },
    ];

    // --- Sample baseline (ref) bone positions once at rest.  They never change. ---
    let ref_y_z: Vec<[f32; 2]> = ref_bones
        .iter()
        .map(|&id| world_yz(&universe.world, id))
        .collect();

    // --- Run each pose ---
    for pose in &poses {
        // Drive driven_t.
        universe.command_queue.push_intent_now(
            driven_t,
            IntentValue::UpdateTransform {
                component_ids: vec![driven_t],
                translation: pose.t,
                rotation_quat_xyzw: pose.rot,
                scale: [1.0, 1.0, 1.0],
            },
        );
        // Settle.
        for _ in 0..SETTLE_TICKS_PER_POSE {
            universe.systems.tick(
                &mut universe.world,
                &mut universe.visuals,
                &universe.render_assets,
                &input,
                &mut universe.command_queue,
                1.0 / 60.0,
            );
            drain(&mut universe);
        }

        // Sample AVC bones + splice_head + driven_t world pose.
        let avc_y_z: Vec<[f32; 2]> = avc_bones
            .iter()
            .map(|&id| world_yz(&universe.world, id))
            .collect();
        let avc_pos: Vec<[f32; 3]> = avc_bones
            .iter()
            .map(|&id| world_pos(&universe.world, id))
            .collect();
        let splice_pos = world_pos(&universe.world, splice_head);
        let driven_pos = world_pos(&universe.world, driven_t);
        let driven_rot = world_rot(&universe.world, driven_t);

        // --- Header ---
        println!("\n================================================================");
        println!("pose: {}", pose.name);
        println!("  {}", pose.description);
        println!(
            "  driven_t world pos: [{:+.3}, {:+.3}, {:+.3}] (model offset +1.2 on X)",
            driven_pos[0], driven_pos[1], driven_pos[2]
        );

        // --- Diff table (Y/Z; X is symmetric and not informative for spine work) ---
        // Reference avatar sits at x=-1.2; AVC at x=+1.2.  Subtract the
        // model-root X offsets so we compare local-space Y/Z.
        println!("\nspine bones — model-local Y/Z (REF vs AVC)");
        println!(
            "  {:<22}  {:>7}  {:>7}    {:>7}  {:>7}     {:>+7}  {:>+7}",
            "bone", "ref_y", "ref_z", "avc_y", "avc_z", "Δy", "Δz",
        );
        for (i, name) in SPINE_BONES.iter().enumerate() {
            let r = ref_y_z[i];
            let a = avc_y_z[i];
            let dy = a[0] - r[0];
            let dz = a[1] - r[1];
            println!(
                "  {:<22}  {:>+7.3}  {:>+7.3}    {:>+7.3}  {:>+7.3}     {:>+7.3}  {:>+7.3}",
                name, r[0], r[1], a[0], a[1], dy, dz,
            );
        }

        // --- Invariants ---
        println!("\ninvariants");

        // (1) Bone-segment lengths along chain should be preserved (FABRIK keeps them).
        let ref_seg_lens = segment_lengths(&ref_bones, &universe.world);
        let avc_seg_lens = segment_lengths(&avc_bones, &universe.world);
        let mut bone_length_ok = true;
        for i in 0..ref_seg_lens.len() {
            let drift = (avc_seg_lens[i] - ref_seg_lens[i]).abs();
            let tag = if drift <= BONE_LENGTH_TOL {
                "ok"
            } else {
                "FAIL"
            };
            if drift > BONE_LENGTH_TOL {
                bone_length_ok = false;
            }
            println!(
                "  bone_length  {:<14}→{:<14}  ref={:.4}  avc={:.4}  drift={:+.4}  {}",
                SPINE_BONES[i],
                SPINE_BONES[i + 1],
                ref_seg_lens[i],
                avc_seg_lens[i],
                avc_seg_lens[i] - ref_seg_lens[i],
                tag,
            );
        }
        if bone_length_ok {
            println!(
                "  → all spine bone-lengths preserved within {:.0}mm",
                BONE_LENGTH_TOL * 1000.0
            );
        }

        // (2) Monotonic Y hips → neck (no kinks/folding in spine).  Head is
        // intentionally below neck on Y by `eye_offset.y` — the head bone
        // pivot sits at skull base, while the HMD is at eye height — so we
        // stop the check at neck.
        let mut mono_ok = true;
        let mut prev_y = f32::NEG_INFINITY;
        for (i, name) in SPINE_BONES.iter().enumerate() {
            if *name == "J_Bip_C_Head" {
                break;
            }
            let y = avc_pos[i][1];
            if y + MONOTONIC_Y_TOL < prev_y {
                println!(
                    "  monotonic_y  FAIL at {}: y={:+.3} drops below prev {:+.3}",
                    name, y, prev_y
                );
                mono_ok = false;
            }
            prev_y = y;
        }
        if mono_ok {
            println!(
                "  monotonic_y  ok (hips → neck all non-decreasing within {:.0}mm)",
                MONOTONIC_Y_TOL * 1000.0
            );
        }

        // (3) Body sits directly UNDER head: hips world XZ ≈ splice_head XZ.
        //
        // This is the key fix for the "body 4-7 cm forward of head" symptom.
        // Hips translate-follows driven_t.xz instantly (Pass on T channel),
        // and AVC adds an xz compensation to model_root.local so the body
        // shifts back by R(driven_rot) * head_target_offset.xz — matching
        // where the head pivot lands.  If these diverge, the avatar will
        // visibly lean forward/back at the hips.
        let hips_idx = SPINE_BONES
            .iter()
            .position(|n| *n == "J_Bip_C_Hips")
            .unwrap();
        let hips_xz = [avc_pos[hips_idx][0], avc_pos[hips_idx][2]];
        let dx = hips_xz[0] - splice_pos[0];
        let dz = hips_xz[1] - splice_pos[2];
        let xz_drift = (dx * dx + dz * dz).sqrt();
        // 5 cm — passes at rest + pure translation; rotation poses can drift
        // up to ~`eye_offset.xz` because the body is statically compensated
        // for the rest HMD orientation, while head moves around its pivot as
        // the HMD rotates.  That drift is anatomical, not a bug.
        const HIPS_UNDER_HEAD_TOL: f32 = 0.050;
        let xz_tag = if xz_drift <= HIPS_UNDER_HEAD_TOL {
            "ok"
        } else {
            "FAIL"
        };
        println!(
            "  hips_under_head  splice_xz=[{:+.3},{:+.3}]  hips_xz=[{:+.3},{:+.3}]  drift={:.4}m  {}",
            splice_pos[0], splice_pos[2], hips_xz[0], hips_xz[1], xz_drift, xz_tag,
        );

        // (4) splice_head world pos ≈ driven_t world pos + R(driven_rot) * offset.
        let predicted_splice_world = {
            let off_world = quat_rotate_vec3(driven_rot, head_target_offset_world);
            [
                driven_pos[0] + off_world[0],
                driven_pos[1] + off_world[1],
                driven_pos[2] + off_world[2],
            ]
        };
        let splice_drift = vec3_dist(splice_pos, predicted_splice_world);
        let splice_tag = if splice_drift <= SPLICE_TARGET_TOL {
            "ok"
        } else {
            "FAIL"
        };
        println!(
            "  splice_head  expected=[{:+.3},{:+.3},{:+.3}]  actual=[{:+.3},{:+.3},{:+.3}]  drift={:.4}m  {}",
            predicted_splice_world[0],
            predicted_splice_world[1],
            predicted_splice_world[2],
            splice_pos[0],
            splice_pos[1],
            splice_pos[2],
            splice_drift,
            splice_tag,
        );
    }

    println!("\n[debug] done. Exiting (no window loop).");
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn ident_quat() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
}

fn drain(universe: &mut engine::Universe) {
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &universe.render_assets,
        &mut universe.command_queue,
    );
}

fn tick_gltf(universe: &mut engine::Universe) {
    let systems = &mut universe.systems;
    systems.gltf.tick_with_queue(
        &mut universe.world,
        &mut universe.visuals,
        &mut systems.skinned_mesh,
        &mut universe.command_queue,
        0.0,
    );
}

fn find_components_by_name(world: &World, name: &str) -> Vec<ComponentId> {
    // `component_name` returns the engine type (e.g. "transform"); user-facing
    // labels (which is how GLTF stores bone names) come from `component_label`.
    world
        .all_components()
        .filter(|&id| world.component_label(id).map_or(false, |n| n == name))
        .collect()
}

fn is_descendant_of(world: &World, child: ComponentId, ancestor: ComponentId) -> bool {
    let mut cur = child;
    for _ in 0..64 {
        let Some(p) = world.parent_of(cur) else {
            return false;
        };
        if p == ancestor {
            return true;
        }
        cur = p;
    }
    false
}

fn world_pos(world: &World, id: ComponentId) -> [f32; 3] {
    world
        .get_component_by_id_as::<TransformComponent>(id)
        .map(|t| {
            let m = t.transform.matrix_world;
            [m[3][0], m[3][1], m[3][2]]
        })
        .unwrap_or([0.0; 3])
}

fn world_rot(world: &World, id: ComponentId) -> [f32; 4] {
    world
        .get_component_by_id_as::<TransformComponent>(id)
        .map(|t| cat_engine::utils::math::mat_to_quat(t.transform.matrix_world))
        .unwrap_or([0.0, 0.0, 0.0, 1.0])
}

fn world_yz(world: &World, id: ComponentId) -> [f32; 2] {
    let p = world_pos(world, id);
    [p[1], p[2]]
}

fn vec3_dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn segment_lengths(chain: &[ComponentId], world: &World) -> Vec<f32> {
    chain
        .windows(2)
        .map(|pair| vec3_dist(world_pos(world, pair[0]), world_pos(world, pair[1])))
        .collect()
}

/// Reconstruct the head IK `target_position_offset` from the AVC subtree.
///
/// AVC computes it as `R(rot_y(offset_yaw)) * -eye_offset_head_local`, where
/// `eye_offset_head_local` is the camera-wrapper T's translation, and
/// `offset_yaw = 0` when `forward_plus_z`, else `π`.  We don't have direct
/// access here, so we replicate the math from the AVC component.
fn head_target_offset_in_target_local(world: &World, avc_id: ComponentId) -> [f32; 3] {
    let avc = world
        .get_component_by_id_as::<AvatarControlComponent>(avc_id)
        .expect("avc missing");
    // Find first T-wrapped camera child (matches the AVC discovery logic).
    let mut eye_offset = [0.0, 0.0, 0.0];
    for &ch in world.children_of(avc_id) {
        let is_t = world
            .get_component_by_id_as::<TransformComponent>(ch)
            .is_some();
        if !is_t {
            continue;
        }
        let wraps_cam = world.children_of(ch).iter().any(|&gc| {
            world
                .get_component_by_id_as::<cat_engine::engine::ecs::component::Camera3DComponent>(gc)
                .is_some()
                || world
                    .get_component_by_id_as::<cat_engine::engine::ecs::component::CameraXRComponent>(gc)
                    .is_some()
        });
        if wraps_cam {
            eye_offset = world
                .get_component_by_id_as::<TransformComponent>(ch)
                .unwrap()
                .transform
                .translation;
            break;
        }
    }
    let offset_yaw = if avc.forward_plus_z {
        0.0
    } else {
        std::f32::consts::PI
    };
    let neg_eye = [-eye_offset[0], -eye_offset[1], -eye_offset[2]];
    quat_rotate_vec3(
        cat_engine::utils::math::quat_rotation_y(offset_yaw),
        neg_eye,
    )
}
