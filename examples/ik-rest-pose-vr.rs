//! VR comparison of imported T-pose arms with simulated A-pose arms.
//! Run: cargo run --release --example ik-rest-pose-vr
//! Defaults to Rei(mu) with simulated A-pose arms. Use --model bisket or
//! --rest-pose t for the original imported rest pose.
//! Press right-controller B at each prompted hand position. Samples are JSONL.

use mittens_engine::engine::ecs::component::{
    AmplitudeComponent, AudioInputComponent, AvatarControlComponent, BoneRestPoseComponent,
    CameraXRComponent, ColorComponent, ControllerHand, ControllerXRComponent, EmissiveComponent,
    GLTFComponent, GrabbableComponent, HTCEyeTrackingComponent, IKChainComponent, IKSolver,
    InputXRGamepadComponent, MirrorComponent, MorphTargetMapComponent, PointLightComponent,
    RaycastableComponent, RestAttachmentComponent, ReturnToRestWhenStillComponent,
    SecondaryMotionComponent, TextComponent, TransformComponent, XrButtonControl, XrComponent,
};
use mittens_engine::engine::ecs::{
    ComponentId, EventSignal, IntentValue, Signal, SignalEmitter, SignalKind, World,
};
use mittens_engine::utils::math::{mat_to_quat, quat_rotate_vec3};
use mittens_engine::{engine, utils};
use serde_json::{Value, json};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const POSES: [&str; 4] = [
    "hands_out_front",
    "hands_against_chest",
    "hands_at_lap_hips",
    "hands_at_sides_outward",
];

struct CaptureState {
    next: usize,
    model: String,
    pose: String,
    output: PathBuf,
    source: PathBuf,
    diagnostics: Option<DiagnosticBindings>,
}

#[derive(Clone, Copy)]
struct DiagnosticArm {
    chain: ComponentId,
    root: ComponentId,
    target: ComponentId,
    pole: [f32; 3],
    panel: ComponentId,
    label: ComponentId,
    color: ComponentId,
}

#[derive(Clone, Copy)]
struct DiagnosticBindings {
    model_root: ComponentId,
    head: ComponentId,
    camera_anchor: ComponentId,
    camera_marker: ComponentId,
    arms: [DiagnosticArm; 2],
}

static STATE: OnceLock<Mutex<CaptureState>> = OnceLock::new();

fn named(world: &World, name: &str) -> Option<ComponentId> {
    world
        .all_components()
        .find(|&id| world.component_label(id) == Some(name))
}

fn descendant<T: 'static>(world: &World, root: ComponentId) -> Option<ComponentId>
where
    T: mittens_engine::engine::ecs::component::Component,
{
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        if world.get_component_by_id_as::<T>(id).is_some() {
            return Some(id);
        }
        pending.extend(world.children_of(id));
    }
    None
}

fn position(world: &World, id: ComponentId) -> [f32; 3] {
    let m = world
        .get_component_by_id_as::<TransformComponent>(id)
        .expect("sampled joint transform")
        .transform
        .matrix_world;
    [m[3][0], m[3][1], m[3][2]]
}

fn rotation(world: &World, id: ComponentId) -> [f32; 4] {
    mat_to_quat(
        world
            .get_component_by_id_as::<TransformComponent>(id)
            .expect("sampled joint transform")
            .transform
            .matrix_world,
    )
}

fn rest_rotation(world: &World, id: ComponentId) -> Option<[f32; 4]> {
    let child = world.children_of(id).iter().copied().find(|&child| {
        world
            .get_component_by_id_as::<BoneRestPoseComponent>(child)
            .is_some()
    })?;
    Some(
        world
            .get_component_by_id_as::<BoneRestPoseComponent>(child)?
            .rotation,
    )
}

fn rest_pose(world: &World, id: ComponentId) -> Option<Value> {
    let child = world.children_of(id).iter().copied().find(|&child| {
        world
            .get_component_by_id_as::<BoneRestPoseComponent>(child)
            .is_some()
    })?;
    let rest = world.get_component_by_id_as::<BoneRestPoseComponent>(child)?;
    Some(json!({
        "translation": rest.translation,
        "rotation": rest.rotation,
        "scale": rest.scale,
    }))
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn len(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}

fn angle(a: [f32; 3], b: [f32; 3]) -> Option<f32> {
    let denom = len(a) * len(b);
    (denom > 1e-7).then(|| (dot(a, b) / denom).clamp(-1.0, 1.0).acos().to_degrees())
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn diagnostic_bindings(world: &World) -> Option<DiagnosticBindings> {
    let model_root = world
        .all_components()
        .find(|&id| world.get_component_by_id_as::<GLTFComponent>(id).is_some())
        .and_then(|gltf| world.parent_of(gltf))?;
    let arm = |side: &str, panel_side: &str| {
        let hand = named(world, &format!("J_Bip_{side}_Hand"))?;
        let (chain_id, chain) = world
            .all_components()
            .filter_map(|id| {
                world
                    .get_component_by_id_as::<IKChainComponent>(id)
                    .map(|chain| (id, chain))
            })
            .find(|(_, chain)| chain.end_effector_id == hand)?;
        let IKSolver::TwoBoneIK {
            root_joint_id,
            pole_direction,
            ..
        } = chain.solver
        else {
            return None;
        };
        let label = named(world, &format!("ik_{panel_side}_normal_text"))?;
        Some(DiagnosticArm {
            chain: chain_id,
            root: root_joint_id,
            target: chain.target_id,
            pole: pole_direction,
            panel: named(world, &format!("ik_{panel_side}_normal_mount"))?,
            label,
            color: descendant::<ColorComponent>(world, label)?,
        })
    };
    Some(DiagnosticBindings {
        model_root,
        head: named(world, "J_Bip_C_Head")?,
        camera_anchor: named(world, "ik_readout_camera_anchor")?,
        camera_marker: named(world, "ik_readout_camera_world")?,
        arms: [arm("L", "left")?, arm("R", "right")?],
    })
}

fn move_world_root(world: &World, emit: &mut dyn SignalEmitter, id: ComponentId, at: [f32; 3]) {
    let Some(transform) = world.get_component_by_id_as::<TransformComponent>(id) else {
        return;
    };
    emit.push_intent_now(
        id,
        IntentValue::UpdateTransform {
            component_id: id,
            translation: at,
            rotation_quat_xyzw: transform.transform.rotation,
            scale: transform.transform.scale,
        },
    );
}

fn on_frame(world: &mut World, emit: &mut dyn SignalEmitter, signal: &Signal) {
    if !matches!(signal.event, Some(EventSignal::FrameTick { .. })) {
        return;
    }
    let bindings = {
        let mut state = STATE
            .get()
            .expect("capture state")
            .lock()
            .expect("capture lock");
        if state.diagnostics.is_none() {
            state.diagnostics = diagnostic_bindings(world);
        }
        state.diagnostics
    };
    let Some(bindings) = bindings else { return };

    let camera = position(world, bindings.camera_anchor);
    let head = position(world, bindings.head);
    let model_rotation = rotation(world, bindings.model_root);
    let model_forward = quat_rotate_vec3(model_rotation, [0.0, 0.0, 1.0]);
    let model_right = quat_rotate_vec3(model_rotation, [1.0, 0.0, 0.0]);
    let inverse_model_rotation = [
        -model_rotation[0],
        -model_rotation[1],
        -model_rotation[2],
        model_rotation[3],
    ];
    move_world_root(world, emit, bindings.camera_marker, camera);

    for (index, arm) in bindings.arms.iter().enumerate() {
        let root = position(world, arm.root);
        let target = position(world, arm.target);
        let reach = sub(target, root);
        let pole_world = quat_rotate_vec3(model_rotation, arm.pole);
        let normal_world = cross(reach, pole_world);
        let panel_at = [
            head[0]
                + model_forward[0] * 1.2
                + model_right[0] * if index == 0 { -0.42 } else { 0.42 },
            head[1] + 0.32,
            head[2]
                + model_forward[2] * 1.2
                + model_right[2] * if index == 0 { -0.42 } else { 0.42 },
        ];
        move_world_root(world, emit, arm.panel, panel_at);

        // atan2(Y, X) is the signed azimuth around body-local +Z: +X is 0.
        // A nearly parallel reach and pole has no reliable bend plane.
        let azimuth = |normal_world: [f32; 3]| -> Option<f32> {
            let local = quat_rotate_vec3(inverse_model_rotation, normal_world);
            (local[0].hypot(local[1]) > 1e-6).then(|| local[1].atan2(local[0]).to_degrees())
        };
        let raw = azimuth(normal_world)
            .map(|angle| format!("{angle:+.0}"))
            .unwrap_or_else(|| "--".to_string());
        let solved = world
            .get_component_by_id_as::<IKChainComponent>(arm.chain)
            .and_then(|chain| chain.last_solved_plane_normal_world)
            .and_then(azimuth)
            .map(|angle| format!("{angle:+.0}"))
            .unwrap_or_else(|| "--".to_string());
        let clipped = world
            .get_component_by_id_as::<IKChainComponent>(arm.chain)
            .is_some_and(|chain| chain.last_bend_plane_clipped);
        let wanted_color = if clipped {
            [1.0, 0.20, 0.24, 1.0]
        } else {
            [1.0, 1.0, 1.0, 1.0]
        };
        if world
            .get_component_by_id_as::<ColorComponent>(arm.color)
            .is_some_and(|color| color.rgba != wanted_color)
        {
            emit.push_intent_now(
                arm.color,
                IntentValue::SetColor {
                    component_id: arm.color,
                    rgba: wanted_color,
                },
            );
        }
        let label = format!(
            "{} raw Z: {raw} deg\nIK Z: {solved} deg",
            if index == 0 { "L" } else { "R" }
        );
        if world
            .get_component_by_id_as::<TextComponent>(arm.label)
            .is_some_and(|text| text.text != label)
        {
            emit.push_intent_now(
                arm.label,
                IntentValue::SetText {
                    component_id: arm.label,
                    text: label,
                },
            );
        }
    }
}

fn sample_arm(world: &World, side: &str) -> Result<Value, String> {
    let bone = |suffix: &str| -> Result<ComponentId, String> {
        named(world, &format!("J_Bip_{side}_{suffix}"))
            .ok_or_else(|| format!("missing {side} {suffix}"))
    };
    let shoulder = bone("Shoulder")?;
    let upper = bone("UpperArm")?;
    let elbow = bone("LowerArm")?;
    let hand = bone("Hand")?;
    let ik = world
        .all_components()
        .filter_map(|id| world.get_component_by_id_as::<IKChainComponent>(id))
        .find(|ik| ik.end_effector_id == hand)
        .ok_or_else(|| format!("{side} arm IK not initialized"))?;
    let IKSolver::TwoBoneIK { pole_direction, .. } = ik.solver else {
        return Err(format!("{side} arm has wrong IK solver"));
    };
    let root = position(world, upper);
    let mid = position(world, elbow);
    let end = position(world, hand);
    let target = position(world, ik.target_id);
    let upper_axis = sub(mid, root);
    let lower_axis = sub(end, mid);
    let reach = sub(target, root);
    let pole_world = world
        .all_components()
        .find(|&id| world.get_component_by_id_as::<GLTFComponent>(id).is_some())
        .and_then(|gltf| world.parent_of(gltf))
        .map(|model_root| quat_rotate_vec3(rotation(world, model_root), pole_direction))
        .unwrap_or(pole_direction);
    let model_rotation = world
        .all_components()
        .find(|&id| world.get_component_by_id_as::<GLTFComponent>(id).is_some())
        .and_then(|gltf| world.parent_of(gltf))
        .map(|root| rotation(world, root))
        .unwrap_or([0.0, 0.0, 0.0, 1.0]);
    let inverse_model_rotation = [
        -model_rotation[0],
        -model_rotation[1],
        -model_rotation[2],
        model_rotation[3],
    ];
    let azimuth = |normal: [f32; 3]| {
        let local = quat_rotate_vec3(inverse_model_rotation, normal);
        (local[0].hypot(local[1]) > 1e-6).then(|| local[1].atan2(local[0]).to_degrees())
    };
    let raw_normal = cross(reach, pole_world);
    let pole_perp = if len(reach) > 1e-6 {
        let projection = dot(pole_world, reach) / dot(reach, reach);
        sub(
            pole_world,
            [
                reach[0] * projection,
                reach[1] * projection,
                reach[2] * projection,
            ],
        )
    } else {
        pole_world
    };
    let elbow_from_axis = if len(reach) > 1e-6 {
        let projection = dot(sub(mid, root), reach) / dot(reach, reach);
        sub(
            sub(mid, root),
            [
                reach[0] * projection,
                reach[1] * projection,
                reach[2] * projection,
            ],
        )
    } else {
        sub(mid, root)
    };
    Ok(json!({
        "side": side,
        "shoulder_world": position(world, shoulder),
        "upper_arm_world": root,
        "elbow_world": mid,
        "hand_world": end,
        "target_world": target,
        "upper_arm_rest_local": rest_pose(world, upper),
        "lower_arm_rest_local": rest_pose(world, elbow),
        "hand_rest_local": rest_pose(world, hand),
        "upper_arm_live_local_rotation": world.get_component_by_id_as::<TransformComponent>(upper).map(|t| t.transform.rotation),
        "lower_arm_live_local_rotation": world.get_component_by_id_as::<TransformComponent>(elbow).map(|t| t.transform.rotation),
        "upper_length_m": len(upper_axis),
        "lower_length_m": len(lower_axis),
        "pole_body_local": pole_direction,
        "pole_world": pole_world,
        "pole_projection_length": len(pole_perp),
        "raw_bend_normal_z_degrees": azimuth(raw_normal),
        "solved_bend_normal_z_degrees": ik.last_solved_plane_normal_world.and_then(azimuth),
        "elbow_vs_pole_degrees": angle(elbow_from_axis, pole_perp),
        "elbow_bend_degrees": angle(upper_axis, lower_axis),
        "hand_target_error_m": len(sub(end, target)),
    }))
}

fn on_b(world: &mut World, emit: &mut dyn SignalEmitter, signal: &Signal) {
    let Some(EventSignal::XrButtonDown {
        source_component,
        hand,
        control,
        ..
    }) = signal.event.as_ref()
    else {
        return;
    };
    if *hand != ControllerHand::Right
        || *control != XrButtonControl::ButtonB
        || world
            .get_component_by_id_as::<InputXRGamepadComponent>(*source_component)
            .is_none()
    {
        return;
    }
    let mut state = STATE
        .get()
        .expect("capture state")
        .lock()
        .expect("capture lock");
    if state.next == POSES.len() {
        println!(
            "[ik-rest-pose] Four poses recorded in {}",
            state.output.display()
        );
        return;
    }
    let left = match sample_arm(world, "L") {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[ik-rest-pose] {e}");
            return;
        }
    };
    let right = match sample_arm(world, "R") {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[ik-rest-pose] {e}");
            return;
        }
    };
    let row = state.next;
    let record = json!({
        "schema": 1,
        "model": state.model,
        "pose_mode": state.pose,
        "source_glb": state.source,
        "capture_index": row,
        "prompt": POSES[row],
        "unix_ms": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis(),
        "left": left,
        "right": right,
    });
    let save = OpenOptions::new()
        .append(true)
        .open(&state.output)
        .and_then(|mut file| writeln!(file, "{record}"));
    if let Err(error) = save {
        eprintln!("[ik-rest-pose] save failed: {error}");
        return;
    }
    if let Some(lamp) = named(world, &format!("capture_lamp_{row}"))
        .and_then(|id| descendant::<EmissiveComponent>(world, id))
    {
        emit.push_intent_now(
            lamp,
            IntentValue::SetEmissiveIntensity {
                component_id: lamp,
                intensity: 2.5,
            },
        );
    }
    state.next += 1;
    println!(
        "[ik-rest-pose] saved {} ({}/4) → {}",
        POSES[row],
        state.next,
        state.output.display()
    );
    if state.next < POSES.len() {
        println!("[ik-rest-pose] next: {}", POSES[state.next]);
    }
}

// Change glTF node rotations before GLTFSystem imports the asset. The imported
// BoneRestPoseComponent therefore reflects the simulated A pose. The binary
// mesh and inverse bind matrices remain unchanged, so this is an arm-pose
// diagnostic variant rather than a newly authored character asset.
fn make_a_pose(source: &Path, destination: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let data = fs::read(source)?;
    if &data[0..4] != b"glTF" {
        return Err("expected GLB".into());
    }
    let json_len = u32::from_le_bytes(data[12..16].try_into()?) as usize;
    let mut doc: Value = serde_json::from_slice(&data[20..20 + json_len])?;
    let rotations = [
        (
            "J_Bip_L_UpperArm",
            [-0.55057746, 0.060794175, 0.040250417, 0.83159393],
        ),
        (
            "J_Bip_R_UpperArm",
            [-0.58444995, -0.059080135, -0.04272674, 0.80814725],
        ),
    ];
    for (name, rotation) in rotations {
        let node = doc["nodes"]
            .as_array_mut()
            .ok_or("GLB has no nodes")?
            .iter_mut()
            .find(|node| node["name"] == name)
            .ok_or_else(|| format!("missing {name}"))?;
        let original = node["rotation"].as_array().ok_or("arm has no rotation")?;
        let start: Vec<f64> = original.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect();
        let end: Vec<f64> = rotation.iter().map(|&v| v as f64).collect();
        // The authored relaxed pose lowers these arms about 75–80 degrees.
        // A normalized halfway blend yields a more useful ~35-degree A pose.
        let sign = if start.iter().zip(&end).map(|(a, b)| a * b).sum::<f64>() < 0.0 {
            -1.0
        } else {
            1.0
        };
        let mut blended: Vec<f64> = start
            .iter()
            .zip(&end)
            .map(|(a, b)| 0.55 * a + 0.45 * sign * b)
            .collect();
        let length = blended.iter().map(|v| v * v).sum::<f64>().sqrt();
        for value in &mut blended {
            *value /= length;
        }
        node["rotation"] = json!(blended);
    }
    let mut json_bytes = serde_json::to_vec(&doc)?;
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    let trailing = &data[20 + json_len..];
    let total = 12 + 8 + json_bytes.len() + trailing.len();
    let mut output = Vec::with_capacity(total);
    output.extend_from_slice(&data[0..8]);
    output.extend_from_slice(&(total as u32).to_le_bytes());
    output.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    output.extend_from_slice(b"JSON");
    output.extend_from_slice(&json_bytes);
    output.extend_from_slice(trailing);
    fs::write(destination, output)?;
    Ok(())
}

fn main() {
    mittens_engine::example_support::ensure_model_assets();
    utils::logger::init();
    let mut model = "rei".to_string();
    let mut pose = "a".to_string();
    let mut validate = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--model" => model = args.next().expect("--model needs bisket or rei"),
            "--rest-pose" => pose = args.next().expect("--rest-pose needs t or a"),
            "--validate" => validate = true,
            _ => panic!("unknown option: {arg}"),
        }
    }
    let source = match model.as_str() {
        "bisket" => PathBuf::from("assets/models/bisket.glb"),
        "rei" => PathBuf::from("assets/models/rei(mu).glb"),
        _ => panic!("--model must be bisket or rei"),
    };
    let scene_model = match pose.as_str() {
        "t" => source.clone(),
        "a" => {
            let directory = PathBuf::from("target/ik-rest-pose");
            fs::create_dir_all(&directory).expect("create generated asset directory");
            let path = directory.join(format!("{model}-a.glb"));
            make_a_pose(&source, &path).expect("generate A-pose GLB");
            path
        }
        _ => panic!("--rest-pose must be t or a"),
    };
    // Keep recordings in data/, which is ignored by Git and survives cargo clean.
    let output_directory = PathBuf::from("data/ik-rest-pose");
    fs::create_dir_all(&output_directory).expect("create capture directory");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let output = output_directory.join(format!("{model}-{pose}-{stamp}.jsonl"));
    if !validate {
        fs::File::create(&output).expect("create capture file");
    }
    let scene = include_str!("ik-rest-pose-vr.mms")
        .replace("MODEL_PATH", &scene_model.to_string_lossy())
        .replace(
            "MODEL_IS_REI",
            if model == "rei" { "true" } else { "false" },
        )
        .replace(
            "MODEL_LABEL",
            if model == "rei" { "Rei(mu)" } else { "Bisket" },
        )
        .replace(
            "REST_POSE_LABEL",
            if pose == "a" {
                "simulated A rest pose"
            } else {
                "imported T rest pose"
            },
        );
    STATE
        .set(Mutex::new(CaptureState {
            next: 0,
            model,
            pose,
            output,
            source,
            diagnostics: None,
        }))
        .unwrap_or_else(|_| panic!("capture state already set"));

    let mut universe = engine::Universe::new(World::default());
    // Keep the scene's RuntimeSpec session alive, as the regular MMS `load`
    // path does. Voice, eye, and motion bindings depend on live evaluation.
    universe
        .load_mms_source_at_path(&scene, "examples/ik-rest-pose-vr.mms")
        .expect("load inspection scene");
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.render_assets,
        &mut universe.command_queue,
    );
    if validate {
        assert!(
            universe.world.all_components().any(|id| universe
                .world
                .get_component_by_id_as::<AudioInputComponent>(id)
                .is_some()),
            "voice capture source missing"
        );
        assert!(
            universe.world.all_components().any(|id| universe
                .world
                .get_component_by_id_as::<AmplitudeComponent>(id)
                .is_some()),
            "mouth amplitude source missing"
        );
        assert!(
            universe.world.all_components().any(|id| universe
                .world
                .get_component_by_id_as::<HTCEyeTrackingComponent>(id)
                .is_some_and(|tracker| !tracker.enable_pupil_direction_tracking)),
            "HTC blink tracking must leave pupil direction to ambient animation"
        );
        assert!(
            universe.world.all_components().any(|id| universe
                .world
                .get_component_by_id_as::<MorphTargetMapComponent>(id)
                .is_some()),
            "eye blink and mouth morph mapping missing"
        );
        assert!(
            universe.world.all_components().any(|id| universe
                .world
                .get_component_by_id_as::<SecondaryMotionComponent>(id)
                .is_some()),
            "standard secondary motion missing"
        );
        let bow_rest_constraints = universe
            .world
            .all_components()
            .filter(|&id| {
                universe
                    .world
                    .get_component_by_id_as::<ReturnToRestWhenStillComponent>(id)
                    .is_some()
            })
            .count();
        let is_rei = STATE.get().unwrap().lock().unwrap().model == "rei";
        assert_eq!(
            bow_rest_constraints,
            if is_rei { 2 } else { 0 },
            "Rei(mu) must have two bow ribbon return-to-rest constraints"
        );
        assert!(
            universe.world.all_components().any(|id| universe
                .world
                .get_component_by_id_as::<XrComponent>(id)
                .is_some_and(|xr| xr.enabled)),
            "scene must enable the OpenXR runtime"
        );
        assert!(
            universe.world.all_components().any(|id| universe
                .world
                .get_component_by_id_as::<CameraXRComponent>(id)
                .is_some_and(|camera| camera.enabled)),
            "scene must provide an enabled XR camera"
        );
        assert!(
            universe.world.all_components().any(|id| universe
                .world
                .get_component_by_id_as::<MirrorComponent>(id)
                .is_some()),
            "scene must show a mirror for arm inspection"
        );
        let mirror = named(&universe.world, "ik_rest_pose_mirror").expect("scene mirror");
        assert!(
            descendant::<GrabbableComponent>(&universe.world, mirror).is_some(),
            "mirror must be grabbable"
        );
        assert!(
            named(&universe.world, "ik_debug_legend").is_some(),
            "scene needs a debug line legend"
        );
        assert!(
            universe
                .systems
                .rx
                .has_global_handlers(SignalKind::FrameTick),
            "readout panels need a FrameTick look-at handler"
        );
        for side in ["left", "right"] {
            let panel = named(&universe.world, &format!("ik_{side}_normal_panel"))
                .expect("bend-plane readout panel");
            let mount = named(&universe.world, &format!("ik_{side}_normal_mount"))
                .expect("bend-plane readout mount");
            assert_eq!(universe.world.parent_of(panel), Some(mount));
            assert!(
                descendant::<GrabbableComponent>(&universe.world, panel).is_some(),
                "bend-plane readout panel must be grabbable"
            );
            assert!(
                descendant::<RaycastableComponent>(&universe.world, panel).is_some(),
                "bend-plane readout panel must be raycastable"
            );
            let text = named(&universe.world, &format!("ik_{side}_normal_text"))
                .expect("bend-plane readout text");
            assert!(
                descendant::<ColorComponent>(&universe.world, text).is_some(),
                "bend-plane readout text needs a tintable color"
            );
        }
        assert!(
            universe
                .world
                .all_components()
                .filter(|&id| universe
                    .world
                    .get_component_by_id_as::<PointLightComponent>(id)
                    .is_some())
                .count()
                >= 2,
            "scene must light both sides of the avatar"
        );
        universe.systems.gltf.tick_with_queue(
            &mut universe.world,
            &mut universe.visuals,
            &mut universe.systems.skinned_mesh,
            &mut universe.systems.renderable,
            &mut universe.command_queue,
            0.0,
        );
        universe.systems.process_commands(
            &mut universe.world,
            &mut universe.visuals,
            &mut universe.render_assets,
            &mut universe.command_queue,
        );
        if is_rei {
            for root in ["head_bow.001", "head_bow.009"] {
                assert!(
                    named(&universe.world, root).is_some(),
                    "Rei(mu) bow ribbon root {root} did not import"
                );
            }
        }
        let avc = universe
            .world
            .all_components()
            .find(|&id| {
                universe
                    .world
                    .get_component_by_id_as::<AvatarControlComponent>(id)
                    .is_some()
            })
            .expect("avatar control");
        let control = universe
            .world
            .get_component_by_id_as::<AvatarControlComponent>(avc)
            .expect("avatar control component");
        assert_eq!(
            control.left_arm_forbidden_bend_normal_z_degrees,
            vec![[-178.0, -115.0], [-100.0, -60.0]]
        );
        assert!(control.right_arm_forbidden_bend_normal_z_degrees.is_empty());
        for hand in [ControllerHand::Left, ControllerHand::Right] {
            let controller = universe
                .world
                .children_of(avc)
                .iter()
                .copied()
                .find(|&id| {
                    universe
                        .world
                        .get_component_by_id_as::<ControllerXRComponent>(id)
                        .is_some_and(|controller| controller.enabled && controller.hand == hand)
                })
                .expect("AVC needs a direct enabled XRHand on each side");
            let tracked_transform = universe
                .world
                .children_of(controller)
                .iter()
                .copied()
                .find(|&id| {
                    universe
                        .world
                        .get_component_by_id_as::<TransformComponent>(id)
                        .is_some()
                })
                .expect("XRHand needs a direct tracked Transform child for arm IK");
            assert!(
                descendant::<RestAttachmentComponent>(&universe.world, tracked_transform).is_some(),
                "tracked hand needs a rest attachment for pointer alignment"
            );
        }
        for side in ["L", "R"] {
            let upper = named(&universe.world, &format!("J_Bip_{side}_UpperArm"))
                .expect("imported upper-arm joint");
            assert!(
                rest_rotation(&universe.world, upper).is_some(),
                "missing rest snapshot"
            );
        }
        for index in 0..POSES.len() {
            let lamp =
                named(&universe.world, &format!("capture_lamp_{index}")).expect("capture lamp");
            let backing = named(&universe.world, &format!("capture_backing_{index}"))
                .expect("capture backing");
            assert_eq!(
                universe.world.parent_of(lamp),
                Some(backing),
                "capture lamp must share its backing circle's center"
            );
            let offset = universe
                .world
                .get_component_by_id_as::<TransformComponent>(lamp)
                .expect("lamp transform")
                .transform
                .translation;
            assert!(
                offset[0].abs() < 1e-5 && offset[1].abs() < 1e-5,
                "capture lamp must be centered on its backing circle"
            );
        }
        println!("[ik-rest-pose] scene and four lamps validated");
        return;
    }
    universe
        .systems
        .rx
        .add_global_handler(SignalKind::XrButtonDown, on_b);
    universe
        .systems
        .rx
        .add_global_handler(SignalKind::FrameTick, on_frame);
    {
        let state = STATE.get().unwrap().lock().unwrap();
        println!(
            "[ik-rest-pose] model={} pose={} output={}",
            state.model,
            state.pose,
            state.output.display()
        );
    }
    println!("[ik-rest-pose] first: {}", POSES[0]);
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
