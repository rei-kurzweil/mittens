//! bisket-vr-debug — headless verification harness for head-driven AVC.
//!
//! See `examples/bisket-vr-debug.mms` for scene setup.
//!
//! What this binary does:
//!   1. Loads the scene (two bisket avatars: one bare-FK reference, one
//!      AVC-driven).
//!   2. Ticks the engine enough to spawn armatures and let AVC wire its
//!      splices + IK chains.
//!   3. Runs the original spine probes against scripted head poses.
//!   4. Resolves the live arm IK chain(s), then scripts wrist/pronation
//!      probes by mutating the actual runtime hand targets directly.
//!   5. Optionally compares authored hand target offsets and
//!      `copy_end_rotation` modes without changing engine behavior outside
//!      this harness process.
//!
//! Run with `cargo run --release --example bisket-vr-debug`.
//! Useful flags:
//!   --compare-copy-end-rotation
//!   --neutralize-hand-offsets
//!   --compare-hand-offsets

use std::env;

use mittens_engine::engine::ecs::component::{
    AvatarControlComponent, IKChainComponent, IKSolver, TransformComponent,
};
use mittens_engine::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use mittens_engine::engine::user_input::InputState;
use mittens_engine::utils::math::{
    mat_to_quat, mat4_identity, mat4_inverse, mat4_mul_vec4, quat_conjugate, quat_from_axis_angle,
    quat_mul, quat_rotate_vec3, quat_rotation_y, quat_to_axis_angle, vec3_len, vec3_normalize,
    vec3_sub,
};
use mittens_engine::{engine, scripting, utils};

const SPINE_BONES: &[&str] = &[
    "J_Bip_C_Hips",
    "J_Bip_C_Spine",
    "J_Bip_C_Chest",
    "J_Bip_C_UpperChest",
    "J_Bip_C_Neck",
    "J_Bip_C_Head",
];

const BONE_LENGTH_TOL: f32 = 0.005;
const SPLICE_TARGET_TOL: f32 = 0.010;
const MONOTONIC_Y_TOL: f32 = 0.005;
const SETTLE_TICKS_PER_POSE: usize = 8;
const WRIST_TWIST_RAD: f32 = 1.35;

#[derive(Clone, Copy)]
struct Pose {
    name: &'static str,
    description: &'static str,
    t: [f32; 3],
    rot: [f32; 4],
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ArmSide {
    Left,
    Right,
}

impl ArmSide {
    fn as_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}

#[derive(Clone, Copy)]
enum HandOffsetMode {
    Authored,
    Neutralized,
}

impl HandOffsetMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Authored => "authored_offsets",
            Self::Neutralized => "neutralized_offsets",
        }
    }
}

#[derive(Clone, Copy)]
enum CopyEndRotationMode {
    SolverDefault,
    ForcedOff,
}

impl CopyEndRotationMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::SolverDefault => "copy_end_rotation=solver_default",
            Self::ForcedOff => "copy_end_rotation=forced_off",
        }
    }
}

#[derive(Clone, Copy)]
struct ArmProbePose {
    name: &'static str,
    description: &'static str,
    left_twist_rad: f32,
    right_twist_rad: f32,
}

#[derive(Clone, Copy)]
struct HarnessOptions {
    compare_copy_end_rotation: bool,
    compare_hand_offsets: bool,
    neutralize_hand_offsets: bool,
}

#[derive(Clone, Copy)]
struct ArmChainRuntime {
    side: ArmSide,
    ik_chain_id: ComponentId,
    upper_arm: ComponentId,
    lower_arm: ComponentId,
    hand: ComponentId,
    target: ComponentId,
    authored_target_local_rotation: [f32; 4],
    rest_hand_world_pos: [f32; 3],
    original_copy_end_rotation: bool,
}

#[derive(Clone, Copy)]
struct ArmPassBaseline {
    hand_world_pos: [f32; 3],
    target_world_rot: [f32; 4],
    forearm_axis_world: [f32; 3],
}

#[derive(Clone, Copy)]
struct NodeRotationSample {
    world: [f32; 4],
    local: [f32; 4],
}

#[derive(Clone, Copy)]
struct ArmPoseSample {
    upper_arm: NodeRotationSample,
    lower_arm: NodeRotationSample,
    hand: NodeRotationSample,
    target: NodeRotationSample,
    lower_to_hand_deg: f32,
    lower_to_target_deg: f32,
    hand_to_target_deg: f32,
}

#[derive(Clone, Copy)]
struct MirrorSignature {
    lower_to_hand_deg: f32,
    lower_to_target_deg: f32,
    hand_to_target_deg: f32,
}

fn main() {
    mittens_engine::example_support::ensure_model_assets();
    utils::logger::init();
    let opts = parse_args();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let source = include_str!("bisket-vr-debug.mms");
    let output = scripting::MeowMeowRunner::eval_with_world_at_path(
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
    tick_gltf(&mut universe);
    drain(&mut universe);

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
    println!(
        "[debug] options: compare_copy_end_rotation={} compare_hand_offsets={} neutralize_hand_offsets={}",
        opts.compare_copy_end_rotation, opts.compare_hand_offsets, opts.neutralize_hand_offsets
    );

    let head_target_offset_world = head_target_offset_in_target_local(&universe.world, avc_id);
    println!(
        "[debug] head_target_offset (target-local) = {:?}",
        head_target_offset_world
    );

    settle_world(&mut universe, 10);

    let (ref_bones, avc_bones) = resolve_spine_bones(&universe.world, avc_id);
    println!(
        "[debug] resolved {} spine bones in both subtrees",
        SPINE_BONES.len()
    );

    let avc_head_bone = avc_bones[SPINE_BONES
        .iter()
        .position(|n| *n == "J_Bip_C_Head")
        .unwrap()];
    let splice_head = universe
        .world
        .parent_of(avc_head_bone)
        .expect("head_bone has no parent — splice didn't run?");

    run_spine_probes(
        &mut universe,
        driven_t,
        splice_head,
        &ref_bones,
        &avc_bones,
        head_target_offset_world,
    );

    let arms = resolve_arm_chains(&universe.world, avc_id);
    print_arm_startup_report(&universe.world, &arms);
    run_arm_probes(&mut universe, driven_t, &arms, opts);

    println!("\n[debug] done. Exiting (no window loop).");
}

fn parse_args() -> HarnessOptions {
    let mut opts = HarnessOptions {
        compare_copy_end_rotation: false,
        compare_hand_offsets: false,
        neutralize_hand_offsets: false,
    };
    for arg in env::args().skip(1) {
        match arg.as_str() {
            "--compare-copy-end-rotation" => opts.compare_copy_end_rotation = true,
            "--compare-hand-offsets" => opts.compare_hand_offsets = true,
            "--neutralize-hand-offsets" => opts.neutralize_hand_offsets = true,
            other => {
                eprintln!("[debug] ignoring unknown arg: {other}");
            }
        }
    }
    opts
}

fn run_spine_probes(
    universe: &mut engine::Universe,
    driven_t: ComponentId,
    splice_head: ComponentId,
    ref_bones: &[ComponentId],
    avc_bones: &[ComponentId],
    head_target_offset_world: [f32; 3],
) {
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

    let ref_y_z: Vec<[f32; 2]> = ref_bones
        .iter()
        .map(|&id| world_yz(&universe.world, id))
        .collect();

    for pose in &poses {
        set_local_transform(
            &mut universe.command_queue,
            driven_t,
            pose.t,
            pose.rot,
            [1.0, 1.0, 1.0],
        );
        settle_world(universe, SETTLE_TICKS_PER_POSE);

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

        println!("\n================================================================");
        println!("spine pose: {}", pose.name);
        println!("  {}", pose.description);
        println!(
            "  driven_t world pos: [{:+.3}, {:+.3}, {:+.3}] (model offset +1.2 on X)",
            driven_pos[0], driven_pos[1], driven_pos[2]
        );

        println!("\nspine bones — model-local Y/Z (REF vs AVC)");
        println!(
            "  {:<22}  {:>7}  {:>7}    {:>7}  {:>7}     {:>+7}  {:>+7}",
            "bone", "ref_y", "ref_z", "avc_y", "avc_z", "Δy", "Δz",
        );
        for (i, name) in SPINE_BONES.iter().enumerate() {
            let r = ref_y_z[i];
            let a = avc_y_z[i];
            println!(
                "  {:<22}  {:>+7.3}  {:>+7.3}    {:>+7.3}  {:>+7.3}     {:>+7.3}  {:>+7.3}",
                name,
                r[0],
                r[1],
                a[0],
                a[1],
                a[0] - r[0],
                a[1] - r[1],
            );
        }

        println!("\ninvariants");

        let ref_seg_lens = segment_lengths(ref_bones, &universe.world);
        let avc_seg_lens = segment_lengths(avc_bones, &universe.world);
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

        let hips_idx = SPINE_BONES
            .iter()
            .position(|n| *n == "J_Bip_C_Hips")
            .unwrap();
        let hips_xz = [avc_pos[hips_idx][0], avc_pos[hips_idx][2]];
        let dx = hips_xz[0] - splice_pos[0];
        let dz = hips_xz[1] - splice_pos[2];
        let xz_drift = (dx * dx + dz * dz).sqrt();
        const HIPS_UNDER_HEAD_TOL: f32 = 0.050;
        println!(
            "  hips_under_head  splice_xz=[{:+.3},{:+.3}]  hips_xz=[{:+.3},{:+.3}]  drift={:.4}m  {}",
            splice_pos[0],
            splice_pos[2],
            hips_xz[0],
            hips_xz[1],
            xz_drift,
            if xz_drift <= HIPS_UNDER_HEAD_TOL {
                "ok"
            } else {
                "FAIL"
            },
        );

        let predicted_splice_world = {
            let off_world = quat_rotate_vec3(driven_rot, head_target_offset_world);
            [
                driven_pos[0] + off_world[0],
                driven_pos[1] + off_world[1],
                driven_pos[2] + off_world[2],
            ]
        };
        let splice_drift = vec3_dist(splice_pos, predicted_splice_world);
        println!(
            "  splice_head  expected=[{:+.3},{:+.3},{:+.3}]  actual=[{:+.3},{:+.3},{:+.3}]  drift={:.4}m  {}",
            predicted_splice_world[0],
            predicted_splice_world[1],
            predicted_splice_world[2],
            splice_pos[0],
            splice_pos[1],
            splice_pos[2],
            splice_drift,
            if splice_drift <= SPLICE_TARGET_TOL {
                "ok"
            } else {
                "FAIL"
            },
        );
    }
}

fn resolve_spine_bones(
    world: &World,
    _avc_id: ComponentId,
) -> (Vec<ComponentId>, Vec<ComponentId>) {
    let mut ref_bones = Vec::with_capacity(SPINE_BONES.len());
    let mut avc_bones = Vec::with_capacity(SPINE_BONES.len());
    for name in SPINE_BONES {
        let all = find_components_by_name(world, name);
        let (avc_side, ref_side): (Vec<_>, Vec<_>) = all
            .into_iter()
            .partition(|&id| world_pos(world, id)[0] > 0.0);
        let avc = avc_side
            .first()
            .copied()
            .unwrap_or_else(|| panic!("bone {} not found on +X AVC side", name));
        let reference = ref_side
            .first()
            .copied()
            .unwrap_or_else(|| panic!("bone {} not found on -X reference side", name));
        ref_bones.push(reference);
        avc_bones.push(avc);
    }
    (ref_bones, avc_bones)
}

fn resolve_arm_chains(world: &World, avc_id: ComponentId) -> Vec<ArmChainRuntime> {
    let avc = world
        .get_component_by_id_as::<AvatarControlComponent>(avc_id)
        .expect("avc missing");
    let hand_map = [
        (
            ArmSide::Left,
            avc.left_hand_bone
                .as_deref()
                .expect("left_hand_bone config missing"),
        ),
        (
            ArmSide::Right,
            avc.right_hand_bone
                .as_deref()
                .expect("right_hand_bone config missing"),
        ),
    ];

    hand_map
        .into_iter()
        .map(|(side, hand_name)| {
            let hand_id = find_components_by_name(world, hand_name)
                .into_iter()
                .find(|&id| is_descendant_of(world, id, avc_id))
                .unwrap_or_else(|| panic!("{} bone not found under AVC subtree", hand_name));
            let (ik_chain_id, solver, target_id) = world
                .all_components()
                .find_map(|id| {
                    let ik = world.get_component_by_id_as::<IKChainComponent>(id)?;
                    if !is_descendant_of(world, id, avc_id) {
                        return None;
                    }
                    if ik.end_effector_id != hand_id {
                        return None;
                    }
                    Some((id, ik.solver, ik.target_id))
                })
                .unwrap_or_else(|| panic!("{} arm IK chain not found", side.as_str()));
            let (upper_arm, lower_arm, copy_end_rotation) = match solver {
                IKSolver::TwoBoneIK {
                    root_joint_id,
                    mid_joint_id,
                    copy_end_rotation,
                    ..
                } => (root_joint_id, mid_joint_id, copy_end_rotation),
                _ => panic!("{} arm chain resolved to non-TwoBoneIK", side.as_str()),
            };
            let authored_target_local_rotation = local_rot(world, target_id);
            let rest_hand_world_pos = world_pos(world, hand_id);

            ArmChainRuntime {
                side,
                ik_chain_id,
                upper_arm,
                lower_arm,
                hand: hand_id,
                target: target_id,
                authored_target_local_rotation,
                rest_hand_world_pos,
                original_copy_end_rotation: copy_end_rotation,
            }
        })
        .collect()
}

fn print_arm_startup_report(world: &World, arms: &[ArmChainRuntime]) {
    println!("\n================================================================");
    println!("arm chain startup");
    for arm in arms {
        let upward = upward_chain_labels(world, arm.hand, Some(arm.upper_arm), 8);
        let skipped = skipped_chain_labels(world, arm.hand, arm.lower_arm, arm.upper_arm);
        println!(
            "  [{}] ik_chain={} target={} upper={} lower={} hand={}",
            arm.side.as_str(),
            describe_component(world, arm.ik_chain_id),
            describe_component(world, arm.target),
            describe_component(world, arm.upper_arm),
            describe_component(world, arm.lower_arm),
            describe_component(world, arm.hand),
        );
        println!("    hand→upper chain: {}", upward.join(" <- "));
        if skipped.is_empty() {
            println!("    skipped between hand and upper: none (direct upper->lower->hand)");
        } else {
            println!(
                "    skipped between hand and upper: {}",
                skipped.join(" <- ")
            );
        }
        println!(
            "    target parent chain: {}",
            upward_chain_labels(world, arm.target, None, 4).join(" <- ")
        );
    }
}

fn run_arm_probes(
    universe: &mut engine::Universe,
    driven_t: ComponentId,
    arms: &[ArmChainRuntime],
    opts: HarnessOptions,
) {
    let hand_offset_modes: Vec<HandOffsetMode> = if opts.compare_hand_offsets {
        vec![HandOffsetMode::Authored, HandOffsetMode::Neutralized]
    } else if opts.neutralize_hand_offsets {
        vec![HandOffsetMode::Neutralized]
    } else {
        vec![HandOffsetMode::Authored]
    };
    let copy_modes: Vec<CopyEndRotationMode> = if opts.compare_copy_end_rotation {
        vec![
            CopyEndRotationMode::SolverDefault,
            CopyEndRotationMode::ForcedOff,
        ]
    } else {
        vec![CopyEndRotationMode::SolverDefault]
    };

    let poses = [
        ArmProbePose {
            name: "neutral_rest",
            description: "both hand targets at rest hand position and pass baseline rotation",
            left_twist_rad: 0.0,
            right_twist_rad: 0.0,
        },
        ArmProbePose {
            name: "left_palm_down",
            description: "left hand target pronated around the current forearm axis",
            left_twist_rad: -WRIST_TWIST_RAD,
            right_twist_rad: 0.0,
        },
        ArmProbePose {
            name: "left_palm_up",
            description: "left hand target supinated around the current forearm axis",
            left_twist_rad: WRIST_TWIST_RAD,
            right_twist_rad: 0.0,
        },
        ArmProbePose {
            name: "right_palm_down",
            description: "right hand target pronated around the current forearm axis",
            left_twist_rad: 0.0,
            right_twist_rad: WRIST_TWIST_RAD,
        },
        ArmProbePose {
            name: "right_palm_up",
            description: "right hand target supinated around the current forearm axis",
            left_twist_rad: 0.0,
            right_twist_rad: -WRIST_TWIST_RAD,
        },
    ];

    set_local_transform(
        &mut universe.command_queue,
        driven_t,
        [0.0, 1.55, 0.0],
        ident_quat(),
        [1.0, 1.0, 1.0],
    );
    settle_world(universe, SETTLE_TICKS_PER_POSE);

    for hand_mode in hand_offset_modes {
        for copy_mode in &copy_modes {
            println!("\n================================================================");
            println!("arm pass: {} | {}", hand_mode.as_str(), copy_mode.as_str());

            apply_pass_settings(&mut universe.world, arms, hand_mode, *copy_mode);
            for arm in arms {
                reset_arm_target_to_pass_baseline(universe, *arm, hand_mode);
            }
            settle_world(universe, SETTLE_TICKS_PER_POSE);

            let baselines: Vec<(ArmSide, ArmPassBaseline)> = arms
                .iter()
                .map(|arm| {
                    (
                        arm.side,
                        ArmPassBaseline {
                            hand_world_pos: arm.rest_hand_world_pos,
                            target_world_rot: world_rot(&universe.world, arm.target),
                            forearm_axis_world: forearm_axis_world(
                                &universe.world,
                                arm.lower_arm,
                                arm.hand,
                            ),
                        },
                    )
                })
                .collect();

            for pose in &poses {
                for arm in arms {
                    let baseline = baselines
                        .iter()
                        .find(|(side, _)| *side == arm.side)
                        .map(|(_, b)| *b)
                        .unwrap();
                    let twist_rad = match arm.side {
                        ArmSide::Left => pose.left_twist_rad,
                        ArmSide::Right => pose.right_twist_rad,
                    };
                    set_arm_target_world_pose(
                        universe,
                        *arm,
                        baseline.hand_world_pos,
                        quat_mul(
                            quat_from_axis_angle(baseline.forearm_axis_world, twist_rad),
                            baseline.target_world_rot,
                        ),
                    );
                }
                settle_world(universe, SETTLE_TICKS_PER_POSE);

                let left = arms
                    .iter()
                    .find(|arm| matches!(arm.side, ArmSide::Left))
                    .copied()
                    .map(|arm| sample_arm_pose(&universe.world, arm))
                    .expect("left arm sample missing");
                let right = arms
                    .iter()
                    .find(|arm| matches!(arm.side, ArmSide::Right))
                    .copied()
                    .map(|arm| sample_arm_pose(&universe.world, arm))
                    .expect("right arm sample missing");

                println!("\npose: {}", pose.name);
                println!("  {}", pose.description);
                print_arm_pose_sample(&universe.world, ArmSide::Left, &left);
                print_arm_pose_sample(&universe.world, ArmSide::Right, &right);

                if pose.name == "left_palm_down" || pose.name == "right_palm_down" {
                    continue;
                }
            }

            let left_down = sample_signature_for_pose(
                universe,
                arms,
                &baselines,
                "left_palm_down",
                -WRIST_TWIST_RAD,
                0.0,
            );
            let right_down = sample_signature_for_pose(
                universe,
                arms,
                &baselines,
                "right_palm_down",
                0.0,
                WRIST_TWIST_RAD,
            );
            let left_up = sample_signature_for_pose(
                universe,
                arms,
                &baselines,
                "left_palm_up",
                WRIST_TWIST_RAD,
                0.0,
            );
            let right_up = sample_signature_for_pose(
                universe,
                arms,
                &baselines,
                "right_palm_up",
                0.0,
                -WRIST_TWIST_RAD,
            );

            println!("\nmirror summary");
            print_mirror_compare("palm_down", left_down, right_down);
            print_mirror_compare("palm_up", left_up, right_up);
        }
    }
}

fn sample_signature_for_pose(
    universe: &mut engine::Universe,
    arms: &[ArmChainRuntime],
    baselines: &[(ArmSide, ArmPassBaseline)],
    _pose_name: &str,
    left_twist_rad: f32,
    right_twist_rad: f32,
) -> MirrorSignature {
    for arm in arms {
        let baseline = baselines
            .iter()
            .find(|(side, _)| *side == arm.side)
            .map(|(_, b)| *b)
            .unwrap();
        let twist = match arm.side {
            ArmSide::Left => left_twist_rad,
            ArmSide::Right => right_twist_rad,
        };
        set_arm_target_world_pose(
            universe,
            *arm,
            baseline.hand_world_pos,
            quat_mul(
                quat_from_axis_angle(baseline.forearm_axis_world, twist),
                baseline.target_world_rot,
            ),
        );
    }
    settle_world(universe, SETTLE_TICKS_PER_POSE);

    let active_arm = if left_twist_rad.abs() > right_twist_rad.abs() {
        arms.iter()
            .find(|arm| matches!(arm.side, ArmSide::Left))
            .copied()
            .unwrap()
    } else {
        arms.iter()
            .find(|arm| matches!(arm.side, ArmSide::Right))
            .copied()
            .unwrap()
    };
    let sample = sample_arm_pose(&universe.world, active_arm);
    MirrorSignature {
        lower_to_hand_deg: sample.lower_to_hand_deg,
        lower_to_target_deg: sample.lower_to_target_deg,
        hand_to_target_deg: sample.hand_to_target_deg,
    }
}

fn apply_pass_settings(
    world: &mut World,
    arms: &[ArmChainRuntime],
    hand_mode: HandOffsetMode,
    copy_mode: CopyEndRotationMode,
) {
    for arm in arms {
        if let Some(tc) = world.get_component_by_id_as_mut::<TransformComponent>(arm.target) {
            tc.transform.rotation = match hand_mode {
                HandOffsetMode::Authored => arm.authored_target_local_rotation,
                HandOffsetMode::Neutralized => ident_quat(),
            };
            tc.transform.recompute_model();
        }
        if let Some(ik) = world.get_component_by_id_as_mut::<IKChainComponent>(arm.ik_chain_id) {
            if let IKSolver::TwoBoneIK {
                root_joint_id,
                mid_joint_id,
                pole_direction,
                copy_end_rotation: enabled,
            } = &mut ik.solver
            {
                let _ = (root_joint_id, mid_joint_id, pole_direction);
                *enabled = match copy_mode {
                    CopyEndRotationMode::SolverDefault => arm.original_copy_end_rotation,
                    CopyEndRotationMode::ForcedOff => false,
                };
            }
        }
    }
}

fn reset_arm_target_to_pass_baseline(
    universe: &mut engine::Universe,
    arm: ArmChainRuntime,
    hand_mode: HandOffsetMode,
) {
    let world_rot = match hand_mode {
        HandOffsetMode::Authored => world_rot(&universe.world, arm.target),
        HandOffsetMode::Neutralized => world_rot(&universe.world, arm.target),
    };
    set_arm_target_world_pose(universe, arm, arm.rest_hand_world_pos, world_rot);
}

fn set_arm_target_world_pose(
    universe: &mut engine::Universe,
    arm: ArmChainRuntime,
    desired_world_pos: [f32; 3],
    desired_world_rot: [f32; 4],
) {
    let (local_translation, local_rotation, local_scale) = world_pose_to_local(
        &universe.world,
        arm.target,
        desired_world_pos,
        desired_world_rot,
    );
    set_local_transform(
        &mut universe.command_queue,
        arm.target,
        local_translation,
        local_rotation,
        local_scale,
    );
}

fn sample_arm_pose(world: &World, arm: ArmChainRuntime) -> ArmPoseSample {
    let upper_arm = sample_node_rotation(world, arm.upper_arm);
    let lower_arm = sample_node_rotation(world, arm.lower_arm);
    let hand = sample_node_rotation(world, arm.hand);
    let target = sample_node_rotation(world, arm.target);
    ArmPoseSample {
        upper_arm,
        lower_arm,
        hand,
        target,
        lower_to_hand_deg: quat_delta_deg(lower_arm.world, hand.world),
        lower_to_target_deg: quat_delta_deg(lower_arm.world, target.world),
        hand_to_target_deg: quat_delta_deg(hand.world, target.world),
    }
}

fn sample_node_rotation(world: &World, id: ComponentId) -> NodeRotationSample {
    NodeRotationSample {
        world: world_rot(world, id),
        local: local_rot(world, id),
    }
}

fn print_arm_pose_sample(world: &World, side: ArmSide, sample: &ArmPoseSample) {
    println!("  [{}]", side.as_str());
    print_node_rot("upper_arm", sample.upper_arm);
    print_node_rot("lower_arm", sample.lower_arm);
    print_node_rot("hand", sample.hand);
    print_node_rot("target", sample.target);
    println!(
        "    deltas: lower→hand={:>6.2}°  lower→target={:>6.2}°  hand→target={:>6.2}°",
        sample.lower_to_hand_deg, sample.lower_to_target_deg, sample.hand_to_target_deg
    );
    let _ = world;
}

fn print_node_rot(label: &str, sample: NodeRotationSample) {
    println!(
        "    {:<9} world={}  local={}",
        label,
        fmt_quat(sample.world),
        fmt_quat(sample.local)
    );
}

fn print_mirror_compare(name: &str, left: MirrorSignature, right: MirrorSignature) {
    println!(
        "  {:<10} lower→hand L/R={:>6.2}°/{:>6.2}°  lower→target L/R={:>6.2}°/{:>6.2}°  hand→target L/R={:>6.2}°/{:>6.2}°",
        name,
        left.lower_to_hand_deg,
        right.lower_to_hand_deg,
        left.lower_to_target_deg,
        right.lower_to_target_deg,
        left.hand_to_target_deg,
        right.hand_to_target_deg,
    );
}

fn upward_chain_labels(
    world: &World,
    start: ComponentId,
    stop_at: Option<ComponentId>,
    max_hops: usize,
) -> Vec<String> {
    let mut out = vec![describe_component(world, start)];
    let mut cur = start;
    for _ in 0..max_hops {
        let Some(parent) = world.parent_of(cur) else {
            break;
        };
        out.push(describe_component(world, parent));
        if Some(parent) == stop_at {
            break;
        }
        cur = parent;
    }
    out
}

fn skipped_chain_labels(
    world: &World,
    hand: ComponentId,
    lower: ComponentId,
    upper: ComponentId,
) -> Vec<String> {
    let mut skipped = Vec::new();
    let mut cur = hand;
    let mut seen_lower = false;
    for _ in 0..8 {
        let Some(parent) = world.parent_of(cur) else {
            break;
        };
        if parent == lower {
            seen_lower = true;
        } else if parent == upper {
            break;
        } else if seen_lower {
            skipped.push(describe_component(world, parent));
        }
        cur = parent;
    }
    skipped
}

fn describe_component(world: &World, id: ComponentId) -> String {
    let label = world
        .component_label(id)
        .or_else(|| world.component_name(id));
    format!("{}({id:?})", label.unwrap_or("?"))
}

fn set_local_transform(
    emit: &mut dyn SignalEmitter,
    id: ComponentId,
    translation: [f32; 3],
    rotation_quat_xyzw: [f32; 4],
    scale: [f32; 3],
) {
    emit.push_intent_now(
        id,
        IntentValue::UpdateTransform {
            component_ids: vec![id],
            translation,
            rotation_quat_xyzw,
            scale,
        },
    );
}

fn world_pose_to_local(
    world: &World,
    id: ComponentId,
    desired_world_pos: [f32; 3],
    desired_world_rot: [f32; 4],
) -> ([f32; 3], [f32; 4], [f32; 3]) {
    let scale = world
        .get_component_by_id_as::<TransformComponent>(id)
        .map(|t| t.transform.scale)
        .unwrap_or([1.0, 1.0, 1.0]);

    let parent_world_mat = parent_world_matrix(world, id).unwrap_or(mat4_identity());
    let parent_world_rot = parent_world_rotation(world, id).unwrap_or(ident_quat());
    let inv_parent = mat4_inverse(parent_world_mat).unwrap_or(mat4_identity());
    let local_pos4 = mat4_mul_vec4(
        inv_parent,
        [
            desired_world_pos[0],
            desired_world_pos[1],
            desired_world_pos[2],
            1.0,
        ],
    );
    let local_rot = quat_mul(quat_conjugate(parent_world_rot), desired_world_rot);
    (
        [local_pos4[0], local_pos4[1], local_pos4[2]],
        local_rot,
        scale,
    )
}

fn parent_world_matrix(world: &World, id: ComponentId) -> Option<[[f32; 4]; 4]> {
    let parent = world.parent_of(id)?;
    world
        .get_component_by_id_as::<TransformComponent>(parent)
        .map(|t| t.transform.matrix_world)
}

fn parent_world_rotation(world: &World, id: ComponentId) -> Option<[f32; 4]> {
    let parent = world.parent_of(id)?;
    world
        .get_component_by_id_as::<TransformComponent>(parent)
        .map(|t| mat_to_quat(t.transform.matrix_world))
}

fn forearm_axis_world(world: &World, lower_arm: ComponentId, hand: ComponentId) -> [f32; 3] {
    let axis = vec3_sub(world_pos(world, hand), world_pos(world, lower_arm));
    if vec3_len(axis) > 1e-6 {
        vec3_normalize(axis)
    } else {
        [0.0, 0.0, 1.0]
    }
}

fn quat_delta_deg(from: [f32; 4], to: [f32; 4]) -> f32 {
    let (_, angle) = quat_to_axis_angle(quat_mul(quat_conjugate(from), to));
    angle.abs().to_degrees()
}

fn fmt_quat(q: [f32; 4]) -> String {
    format!("[{:+.3},{:+.3},{:+.3},{:+.3}]", q[0], q[1], q[2], q[3])
}

fn ident_quat() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
}

fn drain(universe: &mut engine::Universe) {
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.render_assets,
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

fn settle_world(universe: &mut engine::Universe, ticks: usize) {
    let input = InputState::default();
    for _ in 0..ticks {
        universe.systems.tick(
            &mut universe.world,
            &mut universe.visuals,
            &mut universe.render_assets,
            &input,
            &mut universe.command_queue,
            1.0 / 60.0,
        );
        drain(universe);
    }
}

fn find_components_by_name(world: &World, name: &str) -> Vec<ComponentId> {
    world
        .all_components()
        .filter(|&id| world.component_label(id).map_or(false, |n| n == name))
        .collect()
}

fn is_descendant_of(world: &World, child: ComponentId, ancestor: ComponentId) -> bool {
    let mut cur = child;
    for _ in 0..64 {
        let Some(parent) = world.parent_of(cur) else {
            return false;
        };
        if parent == ancestor {
            return true;
        }
        cur = parent;
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
        .map(|t| mat_to_quat(t.transform.matrix_world))
        .unwrap_or(ident_quat())
}

fn local_rot(world: &World, id: ComponentId) -> [f32; 4] {
    world
        .get_component_by_id_as::<TransformComponent>(id)
        .map(|t| t.transform.rotation)
        .unwrap_or(ident_quat())
}

fn world_yz(world: &World, id: ComponentId) -> [f32; 2] {
    let p = world_pos(world, id);
    [p[1], p[2]]
}

fn vec3_dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = vec3_sub(a, b);
    vec3_len(d)
}

fn segment_lengths(chain: &[ComponentId], world: &World) -> Vec<f32> {
    chain
        .windows(2)
        .map(|pair| vec3_dist(world_pos(world, pair[0]), world_pos(world, pair[1])))
        .collect()
}

fn head_target_offset_in_target_local(world: &World, avc_id: ComponentId) -> [f32; 3] {
    let avc = world
        .get_component_by_id_as::<AvatarControlComponent>(avc_id)
        .expect("avc missing");
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
                .get_component_by_id_as::<mittens_engine::engine::ecs::component::Camera3DComponent>(gc)
                .is_some()
                || world
                    .get_component_by_id_as::<mittens_engine::engine::ecs::component::CameraXRComponent>(gc)
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
    quat_rotate_vec3(quat_rotation_y(offset_yaw), neg_eye)
}
