use crate::engine::ecs::component::{
    AvatarControlComponent, ControllerHand, ControllerXRComponent, QuatTemporalFilterComponent,
    QuatYawFollowComponent, TransformComponent, TransformForkTRSComponent,
    TransformMapRotationComponent, TransformMergeTRSComponent, TransformPipelineComponent,
    TransformPipelineOutputComponent,
};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};

#[derive(Debug, Default)]
pub struct AvatarControlSystem;

impl AvatarControlSystem {
    pub fn new() -> Self {
        Self
    }

    pub fn tick(&mut self, world: &mut World, emit: &mut dyn SignalEmitter, dt_sec: f32) {
        let ids: Vec<ComponentId> = world
            .all_components()
            .filter(|&id| {
                world
                    .get_component_by_id_as::<AvatarControlComponent>(id)
                    .is_some()
            })
            .collect();

        for id in ids {
            tick_one(id, world, emit, dt_sec);
        }
    }
}

fn tick_one(id: ComponentId, world: &mut World, emit: &mut dyn SignalEmitter, _dt_sec: f32) {
    // --- Init phase ---
    let needs_init = {
        let Some(c) = world.get_component_by_id_as::<AvatarControlComponent>(id) else {
            return;
        };
        c.splice_head.is_none()
    };

    if needs_init {
        try_init_splices(id, world, emit);
        return; // regular tick runs next frame after Attach intents are flushed
    }

    // --- Regular tick: head rotation only ---
    // Body rotation is handled by the body pipeline (YawFollow op in TransformPipelineSystem).
    let (forward_plus_z, splice_head_id) = {
        let Some(c) = world.get_component_by_id_as::<AvatarControlComponent>(id) else {
            return;
        };
        let Some(splice_head_id) = c.splice_head else { return };
        (c.forward_plus_z, splice_head_id)
    };

    // driven_t is the parent of AvatarControlComponent.
    let Some(driven_t_id) = world.parent_of(id) else { return };
    let driven_world_rot = {
        let Some(t) = world.get_component_by_id_as::<TransformComponent>(driven_t_id) else {
            return;
        };
        mat_to_quat(t.transform.matrix_world)
    };

    // neck_parent is the parent of splice_head (stable after init).
    let Some(neck_parent_id) = world.parent_of(splice_head_id) else { return };
    let neck_parent_world_rot = {
        let Some(t) = world.get_component_by_id_as::<TransformComponent>(neck_parent_id) else {
            return;
        };
        mat_to_quat(t.transform.matrix_world)
    };

    // --- Head rotation ---
    // For VR (-Z forward): multiply by quat_rotation_y(π) to bake the VRM/OpenXR handedness flip.
    // For desktop (+Z forward): both input and VRM face +Z — no correction needed.
    let handedness_correction = if forward_plus_z { 0.0 } else { std::f32::consts::PI };
    let head_world_rot = quat_mul(driven_world_rot, quat_rotation_y(handedness_correction));
    let splice_local_rot = quat_mul(quat_inverse(neck_parent_world_rot), head_world_rot);
    emit.push_intent_now(
        splice_head_id,
        IntentValue::UpdateTransform {
            component_ids: vec![splice_head_id],
            translation: [0.0, 0.0, 0.0],
            rotation_quat_xyzw: splice_local_rot,
            scale: [1.0, 1.0, 1.0],
        },
    );
}

/// First-time setup: splice bones, create body pipeline, and (optionally) hand smoothing pipelines.
///
/// Controllers are discovered by topology: any `ControllerXRComponent` that is a
/// **direct child** of this `AvatarControlComponent` is treated as a hand driver.
/// Its `hand` field (`Left` / `Right`) determines which hand bone it drives.
///
/// Body pipeline created here reads `driven_t`'s world matrix, strips pitch/roll via `YawFollow`,
/// and writes the result to `model_root` (which is re-parented under the pipeline output).
fn try_init_splices(id: ComponentId, world: &mut World, emit: &mut dyn SignalEmitter) {
    let (head_bone_name, left_hand_bone, right_hand_bone,
         body_yaw_threshold, body_yaw_rate, forward_plus_z,
         initial_body_yaw, hand_rotation_smoothing, skip_body_pipeline) = {
        let Some(c) = world.get_component_by_id_as::<AvatarControlComponent>(id) else {
            return;
        };
        (
            c.head_bone.clone(),
            c.left_hand_bone.clone(),
            c.right_hand_bone.clone(),
            c.body_yaw_threshold,
            c.body_yaw_rate,
            c.forward_plus_z,
            c.initial_body_yaw,
            c.hand_rotation_smoothing,
            c.skip_body_pipeline,
        )
    };

    // Find model_root: first TransformComponent child of AvatarControlComponent.
    let Some(model_root_id) = world
        .children_of(id)
        .iter()
        .copied()
        .find(|&ch| world.get_component_by_id_as::<TransformComponent>(ch).is_some())
    else {
        return;
    };

    // Discover hand controllers by topology: direct ControllerXRComponent children.
    let left_ctrl = world
        .children_of(id)
        .iter()
        .copied()
        .find(|&ch| {
            world
                .get_component_by_id_as::<ControllerXRComponent>(ch)
                .map(|c| c.hand == ControllerHand::Left)
                .unwrap_or(false)
        });
    let right_ctrl = world
        .children_of(id)
        .iter()
        .copied()
        .find(|&ch| {
            world
                .get_component_by_id_as::<ControllerXRComponent>(ch)
                .map(|c| c.hand == ControllerHand::Right)
                .unwrap_or(false)
        });

    // Head bone is required — retry next tick if GLTF hasn't spawned yet.
    let head_selector = format!("[name='{}']", head_bone_name);
    let Some(head_bone_id) = world.find_component(model_root_id, &head_selector) else {
        return;
    };
    let Some(head_parent_id) = world.parent_of(head_bone_id) else { return };
    let head_splice_id = world.add_component(TransformComponent::new());

    // Resolve hand splices (raw driver = controller's driven_t or plain TC).
    let left  = resolve_hand_splice(world, model_root_id, left_hand_bone.as_deref(),  left_ctrl);
    let right = resolve_hand_splice(world, model_root_id, right_hand_bone.as_deref(), right_ctrl);

    // Store runtime IDs (body_pipeline_id stored after pipeline creation below).
    if let Some(c) = world.get_component_by_id_as_mut::<AvatarControlComponent>(id) {
        c.splice_head    = Some(head_splice_id);
        c.displaced_head = Some(head_bone_id);
        if let Some((_, driver, bone)) = left  { c.splice_left_hand  = Some(driver); c.displaced_left_hand  = Some(bone); }
        if let Some((_, driver, bone)) = right { c.splice_right_hand = Some(driver); c.displaced_right_hand = Some(bone); }
    }

    // -----------------------------------------------------------------------
    // Body pipeline: created as a child of AVC; model_root re-parented under it.
    //
    // Topology:
    //   AVC
    //     └── body_pipeline  (TransformPipelineComponent)
    //           TransformForkTRSComponent
    //             TransformMapRotationComponent
    //               QuatYawFollowComponent { threshold, rate, initial_yaw, forward_plus_z }
    //             TransformMergeTRSComponent
    //           TransformPipelineOutputComponent
    //             model_root  ← re-parented here
    // -----------------------------------------------------------------------
    if !skip_body_pipeline {
        let body_pipeline_id  = world.add_component(TransformPipelineComponent::new());
        let fork_id           = world.add_component(TransformForkTRSComponent::new());
        let map_rot_id        = world.add_component(TransformMapRotationComponent::new());
        let yaw_follow_id     = world.add_component(
            QuatYawFollowComponent::new(body_yaw_threshold, body_yaw_rate)
                .with_initial_yaw(initial_body_yaw)
                .with_forward_plus_z_if(forward_plus_z),
        );
        let merge_id          = world.add_component(TransformMergeTRSComponent::new());
        let pipeline_output_id = world.add_component(TransformPipelineOutputComponent::new());

        // Wire internal pipeline structure (all new, uninitialized).
        let _ = world.set_parent(fork_id,           Some(body_pipeline_id));
        let _ = world.set_parent(map_rot_id,         Some(fork_id));
        let _ = world.set_parent(yaw_follow_id,      Some(map_rot_id));
        let _ = world.set_parent(merge_id,           Some(fork_id));
        let _ = world.set_parent(pipeline_output_id, Some(body_pipeline_id));

        if let Some(c) = world.get_component_by_id_as_mut::<AvatarControlComponent>(id) {
            c.body_pipeline_id = Some(body_pipeline_id);
        }

        // Attach pipeline to AVC (initializes the pipeline tree).
        emit_attach(emit, id, body_pipeline_id);
        // Re-parent model_root under the pipeline output.
        emit_attach(emit, pipeline_output_id, model_root_id);
    }

    // -----------------------------------------------------------------------
    // Head splice: splice_head under neck_parent, head bone under splice_head.
    // -----------------------------------------------------------------------
    emit_attach(emit, head_parent_id, head_splice_id);
    emit_attach(emit, head_splice_id, head_bone_id);

    // -----------------------------------------------------------------------
    // Hand splices.
    // For each hand:
    //   - Re-parent controller (or plain-TC splice) under bone's original parent.
    //   - If hand_rotation_smoothing is Some: create a smoothing pipeline under the
    //     raw driver (controller_driven_t), displace bone under smoothed_t.
    //   - If None: displace bone directly under the raw driver.
    // -----------------------------------------------------------------------
    for hand in [left, right].into_iter().flatten() {
        let (bone_parent, raw_driver, bone) = hand;
        let splice_root = world.parent_of(raw_driver).filter(|&p| p != bone_parent).unwrap_or(raw_driver);
        emit_attach(emit, bone_parent, splice_root);

        if let Some(smoothing_factor) = hand_rotation_smoothing {
            // Create smoothing pipeline under raw_driver.
            let hp_id     = world.add_component(TransformPipelineComponent::new());
            let hfork_id  = world.add_component(TransformForkTRSComponent::new());
            let hmrot_id  = world.add_component(TransformMapRotationComponent::new());
            let hfilt_id  = world.add_component(
                QuatTemporalFilterComponent::new().with_smoothing_factor(smoothing_factor),
            );
            let hmerge_id  = world.add_component(TransformMergeTRSComponent::new());
            let hout_id    = world.add_component(TransformPipelineOutputComponent::new());
            let smoothed_t = world.add_component(TransformComponent::new());

            let _ = world.set_parent(hfork_id,  Some(hp_id));
            let _ = world.set_parent(hmrot_id,  Some(hfork_id));
            let _ = world.set_parent(hfilt_id,  Some(hmrot_id));
            let _ = world.set_parent(hmerge_id, Some(hfork_id));
            let _ = world.set_parent(hout_id,   Some(hp_id));
            let _ = world.set_parent(smoothed_t, Some(hout_id));

            emit_attach(emit, raw_driver, hp_id);
            emit_attach(emit, smoothed_t, bone);
        } else {
            emit_attach(emit, raw_driver, bone);
        }
    }
}

/// Find a hand bone by name and determine its raw driver node.
///
/// Returns `(bone_original_parent, raw_driver, bone_id)` or `None` if the bone
/// wasn't found (model may not have this joint — silently skip).
fn resolve_hand_splice(
    world: &mut World,
    model_root: ComponentId,
    bone_name: Option<&str>,
    controller: Option<ComponentId>,
) -> Option<(ComponentId, ComponentId, ComponentId)> {
    let bone_name = bone_name?;
    let sel = format!("[name='{}']", bone_name);
    let bone = world.find_component(model_root, &sel)?;
    let bone_parent = world.parent_of(bone)?;

    let driver = if let Some(ctrl) = controller {
        world
            .children_of(ctrl)
            .iter()
            .copied()
            .find(|&ch| world.get_component_by_id_as::<TransformComponent>(ch).is_some())
            .unwrap_or_else(|| world.add_component(TransformComponent::new()))
    } else {
        world.add_component(TransformComponent::new())
    };

    Some((bone_parent, driver, bone))
}

fn emit_attach(emit: &mut dyn SignalEmitter, parent: ComponentId, child: ComponentId) {
    emit.push_intent_now(parent, IntentValue::Attach { parents: vec![parent], child });
}

// ---------------------------------------------------------------------------
// Math helpers
// ---------------------------------------------------------------------------

fn mat_to_quat(m: [[f32; 4]; 4]) -> [f32; 4] {
    fn col_len(m: [[f32; 4]; 4], c: usize) -> f32 {
        (m[c][0] * m[c][0] + m[c][1] * m[c][1] + m[c][2] * m[c][2]).sqrt().max(1e-9)
    }
    let s0 = col_len(m, 0).recip();
    let s1 = col_len(m, 1).recip();
    let s2 = col_len(m, 2).recip();

    let r00 = m[0][0]*s0; let r10 = m[0][1]*s0; let r20 = m[0][2]*s0;
    let r01 = m[1][0]*s1; let r11 = m[1][1]*s1; let r21 = m[1][2]*s1;
    let r02 = m[2][0]*s2; let r12 = m[2][1]*s2; let r22 = m[2][2]*s2;

    let trace = r00 + r11 + r22;
    if trace > 0.0 {
        let s = 0.5 / (trace + 1.0).sqrt();
        normalise_quat([(r21-r12)*s, (r02-r20)*s, (r10-r01)*s, 0.25/s])
    } else if r00 > r11 && r00 > r22 {
        let s = 2.0 * (1.0 + r00 - r11 - r22).sqrt();
        normalise_quat([0.25*s, (r01+r10)/s, (r02+r20)/s, (r21-r12)/s])
    } else if r11 > r22 {
        let s = 2.0 * (1.0 + r11 - r00 - r22).sqrt();
        normalise_quat([(r01+r10)/s, 0.25*s, (r12+r21)/s, (r02-r20)/s])
    } else {
        let s = 2.0 * (1.0 + r22 - r00 - r11).sqrt();
        normalise_quat([(r02+r20)/s, (r12+r21)/s, 0.25*s, (r10-r01)/s])
    }
}

fn normalise_quat(q: [f32; 4]) -> [f32; 4] {
    let len2 = q[0]*q[0] + q[1]*q[1] + q[2]*q[2] + q[3]*q[3];
    if len2 < 1e-12 { return [0.0, 0.0, 0.0, 1.0]; }
    let inv = len2.sqrt().recip();
    [q[0]*inv, q[1]*inv, q[2]*inv, q[3]*inv]
}

fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let (ax, ay, az, aw) = (a[0], a[1], a[2], a[3]);
    let (bx, by, bz, bw) = (b[0], b[1], b[2], b[3]);
    [
        aw*bx + ax*bw + ay*bz - az*by,
        aw*by - ax*bz + ay*bw + az*bx,
        aw*bz + ax*by - ay*bx + az*bw,
        aw*bw - ax*bx - ay*by - az*bz,
    ]
}

fn quat_inverse(q: [f32; 4]) -> [f32; 4] { [-q[0], -q[1], -q[2], q[3]] }

fn quat_rotation_y(yaw: f32) -> [f32; 4] {
    let half = yaw * 0.5;
    [0.0, half.sin(), 0.0, half.cos()]
}
