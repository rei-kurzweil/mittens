use crate::engine::ecs::component::{
    BoneRestPoseComponent, ComponentRef, GLTFComponent, QueryRootMode, SecondaryMotionComponent,
    SpringBoneComponent, SpringJointComponent, TransformComponent, resolve_component_ref,
};
use crate::engine::ecs::{ComponentId, World};
use crate::utils::math::{
    mat_to_quat, quat_conjugate, quat_mul, quat_rotate_vec3, shortest_arc_quat, vec3_add, vec3_len,
    vec3_normalize, vec3_scale, vec3_sub,
};
use std::collections::{HashMap, HashSet};

const STEP: f32 = 1.0 / 60.0;

#[derive(Debug, Clone)]
struct JointConfig {
    id: ComponentId,
    rest_rotation: [f32; 4],
    stiffness: f32,
    drag: f32,
    gravity: [f32; 3],
}
#[derive(Debug, Clone)]
struct ChainState {
    gltf: ComponentId,
    joints: Vec<JointConfig>,
    previous: Vec<[f32; 3]>,
    current: Vec<[f32; 3]>,
    lengths: Vec<f32>,
    accumulator: f32,
    enabled: bool,
}

#[derive(Debug, Default)]
pub struct SecondaryMotionSystem {
    states: HashMap<ComponentId, ChainState>,
    warned: HashSet<ComponentId>,
    failures: HashMap<ComponentId, String>,
    debug_frames: u64,
}

impl SecondaryMotionSystem {
    pub fn reset(&mut self, target: ComponentId) {
        self.states
            .retain(|chain, state| *chain != target && state.gltf != target);
    }

    pub fn tick(&mut self, world: &mut World, dt: f32) {
        self.debug_frames = self.debug_frames.wrapping_add(1);
        let mut max_correction_radians = 0.0f32;
        let roots: Vec<_> = world
            .all_components()
            .filter(|id| {
                world
                    .get_component_by_id_as::<SecondaryMotionComponent>(*id)
                    .is_some()
            })
            .collect();
        let mut live = HashSet::new();
        for root in roots {
            let Some(gltf_id) = nearest_gltf(world, root) else {
                continue;
            };
            let mut owned = HashSet::new();
            let chains: Vec<_> = world
                .children_of(root)
                .iter()
                .copied()
                .filter(|id| {
                    world
                        .get_component_by_id_as::<SpringBoneComponent>(*id)
                        .is_some()
                })
                .collect();
            for chain_id in chains {
                live.insert(chain_id);
                if !self.states.contains_key(&chain_id) {
                    match bind_chain(world, gltf_id, chain_id, &mut owned) {
                        Ok(state) => {
                            self.states.insert(chain_id, state);
                            self.warned.remove(&chain_id);
                            self.failures.remove(&chain_id);
                        }
                        Err(error) => {
                            self.failures.insert(chain_id, error.clone());
                            if self.warned.insert(chain_id) {
                                eprintln!(
                                    "[SecondaryMotion] chain {chain_id:?}: {error}; will retry after respawn"
                                );
                            }
                        }
                    }
                }
                let Some(state) = self.states.get_mut(&chain_id) else {
                    continue;
                };
                if state
                    .joints
                    .iter()
                    .any(|j| world.get_component_record(j.id).is_none())
                {
                    self.states.remove(&chain_id);
                    continue;
                }
                let enabled = world
                    .get_component_by_id_as::<SpringBoneComponent>(chain_id)
                    .map(|c| c.enabled)
                    .unwrap_or(false);
                if !enabled {
                    state.enabled = false;
                    continue;
                }
                if !state.enabled || !dt.is_finite() || dt > 0.25 {
                    reset_state(world, state);
                    state.enabled = true;
                    continue;
                }
                state.accumulator = (state.accumulator + dt.max(0.0)).min(STEP * 4.0);
                while state.accumulator >= STEP {
                    simulate_step(world, state);
                    state.accumulator -= STEP;
                }
                max_correction_radians = max_correction_radians.max(apply_rotations(world, state));
            }
        }
        self.states.retain(|id, _| live.contains(id));
        self.failures.retain(|id, _| live.contains(id));
        if self.debug_frames % 120 == 0 && std::env::var_os("CAT_DEBUG_SECONDARY_MOTION").is_some()
        {
            eprintln!(
                "[SecondaryMotion][debug] bound_chains={} failed_chains={} max_rotation_correction_deg={:.2}",
                self.states.len(),
                self.failures.len(),
                max_correction_radians.to_degrees()
            );
            for (chain_id, error) in self.failures.iter().take(3) {
                let name = world
                    .get_component_by_id_as::<SpringBoneComponent>(*chain_id)
                    .map(|chain| chain.stable_name.as_str())
                    .unwrap_or("<removed>");
                eprintln!("[SecondaryMotion][debug] failed '{name}': {error}");
            }
        }
    }
}

fn nearest_gltf(world: &World, mut id: ComponentId) -> Option<ComponentId> {
    for _ in 0..64 {
        id = world.parent_of(id)?;
        if world.get_component_by_id_as::<GLTFComponent>(id).is_some() {
            return Some(id);
        }
    }
    None
}
fn descendant(world: &World, mut node: ComponentId, ancestor: ComponentId) -> bool {
    for _ in 0..64 {
        if node == ancestor {
            return true;
        }
        let Some(p) = world.parent_of(node) else {
            return false;
        };
        node = p;
    }
    false
}
fn pos(world: &World, id: ComponentId) -> Option<[f32; 3]> {
    let m = world
        .get_component_by_id_as::<TransformComponent>(id)?
        .transform
        .matrix_world;
    Some([m[3][0], m[3][1], m[3][2]])
}
fn rest(world: &World, id: ComponentId) -> Option<BoneRestPoseComponent> {
    world.children_of(id).iter().find_map(|c| {
        world
            .get_component_by_id_as::<BoneRestPoseComponent>(*c)
            .copied()
    })
}

fn ref_surface(reference: &ComponentRef) -> String {
    match reference {
        ComponentRef::Guid(guid) => format!("@uuid:{guid}"),
        ComponentRef::Query(query) => query.clone(),
    }
}

fn resolve_in_gltf(
    world: &World,
    gltf_id: ComponentId,
    reference: &ComponentRef,
) -> Result<ComponentId, String> {
    let gltf = world
        .get_component_by_id_as::<GLTFComponent>(gltf_id)
        .ok_or("owning GLTF disappeared")?;
    let instance_nodes: HashSet<_> = gltf.spawned_node_transforms.iter().copied().collect();
    if instance_nodes.is_empty() {
        return Err("owning GLTF has not spawned its node transforms yet".into());
    }
    let anchor = world
        .parent_of(gltf_id)
        .ok_or("owning GLTF has no transform anchor")?;

    let id = match reference {
        ComponentRef::Guid(guid) => world.component_id_by_guid(*guid),
        ComponentRef::Query(query) if !query.starts_with('/') && !query.starts_with("../") => {
            // Imported nodes can be reparented after spawning (AvatarControl
            // splices the head subtree, including hair). Match against the
            // GLTF instance's recorded node set instead of assuming those
            // nodes remain reachable below the original anchor.
            let matches: Vec<_> = instance_nodes
                .iter()
                .copied()
                .filter(|id| world.component_matches_selector(*id, query))
                .collect();
            if matches.len() != 1 {
                return Err(format!(
                    "query '{}' matched {} nodes in the owning GLTF instance (expected exactly one)",
                    query,
                    matches.len()
                ));
            }
            matches.first().copied()
        }
        ComponentRef::Query(_) => resolve_component_ref(
            world,
            reference,
            Some(anchor),
            QueryRootMode::SelfSubtree,
        ),
    }
    .ok_or_else(|| format!("reference '{}' did not resolve", ref_surface(reference)))?;

    if !instance_nodes.contains(&id) {
        return Err(format!(
            "reference '{}' resolved outside the owning GLTF instance",
            ref_surface(reference)
        ));
    }
    Ok(id)
}

fn bind_chain(
    world: &World,
    gltf_id: ComponentId,
    chain_id: ComponentId,
    owned: &mut HashSet<ComponentId>,
) -> Result<ChainState, String> {
    if let Some(anchor) = world
        .parent_of(gltf_id)
        .and_then(|id| world.get_component_by_id_as::<TransformComponent>(id))
    {
        let m = anchor.transform.matrix_world;
        let sx = vec3_len([m[0][0], m[0][1], m[0][2]]);
        let sy = vec3_len([m[1][0], m[1][1], m[1][2]]);
        let sz = vec3_len([m[2][0], m[2][1], m[2][2]]);
        let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[1][0] * (m[0][1] * m[2][2] - m[0][2] * m[2][1])
            + m[2][0] * (m[0][1] * m[1][2] - m[0][2] * m[1][1]);
        if det <= 0.0 || (sx - sy).abs() > 1e-4 || (sx - sz).abs() > 1e-4 {
            return Err("non-uniform or negative GLTF instance scale is unsupported".into());
        }
    }
    let chain = world
        .get_component_by_id_as::<SpringBoneComponent>(chain_id)
        .ok_or("missing SpringBone")?;
    if world
        .get_component_by_id_as::<GLTFComponent>(gltf_id)
        .is_none()
    {
        return Err("owning GLTF disappeared".into());
    }
    if let Some(center) = &chain.center {
        resolve_in_gltf(world, gltf_id, center).map_err(|error| format!("center {error}"))?;
    }
    let joint_components: Vec<_> = world
        .children_of(chain_id)
        .iter()
        .filter_map(|id| {
            world
                .get_component_by_id_as::<SpringJointComponent>(*id)
                .map(|j| (*id, j))
        })
        .collect();
    if joint_components.is_empty() {
        return Err("has no SpringJoint children".into());
    }
    let mut joints = Vec::new();
    for (_, j) in joint_components {
        let id = resolve_in_gltf(world, gltf_id, &j.node)?;
        if !owned.insert(id) {
            return Err(format!(
                "joint '{}' overlaps another chain",
                ref_surface(&j.node)
            ));
        }
        let r = rest(world, id)
            .ok_or_else(|| format!("joint '{}' has no rest pose", ref_surface(&j.node)))?;
        joints.push(JointConfig {
            id,
            rest_rotation: r.rotation,
            stiffness: j.stiffness,
            drag: j.drag_force,
            gravity: vec3_scale(vec3_normalize(j.gravity_dir), j.gravity_power),
        });
    }
    for pair in joints.windows(2) {
        if !descendant(world, pair[1].id, pair[0].id) {
            return Err("joint list is reordered or non-ancestral".into());
        }
    }
    let virtual_ratio = chain.virtual_end_length_ratio;
    if joints.len() == 1 && virtual_ratio.is_none() {
        return Err("a one-joint chain requires a virtual endpoint".into());
    }
    let mut current = Vec::new();
    let mut lengths = Vec::new();
    for i in 0..joints.len() {
        let head = pos(world, joints[i].id).ok_or("joint transform missing")?;
        let tail = if i + 1 < joints.len() {
            pos(world, joints[i + 1].id).unwrap()
        } else {
            let ratio = virtual_ratio.unwrap_or(1.0);
            let prev = pos(world, joints[i - 1].id).unwrap();
            vec3_add(
                head,
                vec3_scale(
                    vec3_normalize(vec3_sub(head, prev)),
                    vec3_len(vec3_sub(head, prev)) * ratio,
                ),
            )
        };
        lengths.push(vec3_len(vec3_sub(tail, head)));
        current.push(tail);
    }
    Ok(ChainState {
        gltf: gltf_id,
        joints,
        previous: current.clone(),
        current,
        lengths,
        accumulator: 0.0,
        enabled: chain.enabled,
    })
}
fn reset_state(world: &World, s: &mut ChainState) {
    for i in 0..s.joints.len() {
        let head = pos(world, s.joints[i].id).unwrap_or([0.0; 3]);
        let tail = if i + 1 < s.joints.len() {
            pos(world, s.joints[i + 1].id).unwrap_or(head)
        } else if i > 0 {
            let prev = pos(world, s.joints[i - 1].id).unwrap_or(head);
            vec3_add(
                head,
                vec3_scale(vec3_normalize(vec3_sub(head, prev)), s.lengths[i]),
            )
        } else {
            head
        };
        s.current[i] = tail;
        s.previous[i] = tail;
    }
    s.accumulator = 0.0;
}
fn simulate_step(world: &World, s: &mut ChainState) {
    for i in 0..s.joints.len() {
        // A joint's simulated tail is the next joint's simulated head. Using
        // primary-pose heads for every segment breaks chain continuity and
        // creates the characteristic curl/flicker feedback.
        let primary_head = pos(world, s.joints[i].id).unwrap_or(s.current[i]);
        let head = if i == 0 {
            primary_head
        } else {
            s.current[i - 1]
        };
        let inertia = vec3_scale(
            vec3_sub(s.current[i], s.previous[i]),
            1.0 - s.joints[i].drag,
        );
        let rest_tail = if i + 1 < s.joints.len() {
            let primary_tail = pos(world, s.joints[i + 1].id).unwrap_or(s.current[i]);
            vec3_add(
                head,
                vec3_scale(
                    vec3_normalize(vec3_sub(primary_tail, primary_head)),
                    s.lengths[i],
                ),
            )
        } else if i > 0 {
            let prev = pos(world, s.joints[i - 1].id).unwrap_or(head);
            vec3_add(
                head,
                vec3_scale(vec3_normalize(vec3_sub(head, prev)), s.lengths[i]),
            )
        } else {
            s.current[i]
        };
        let stiffness = vec3_scale(
            vec3_sub(rest_tail, s.current[i]),
            s.joints[i].stiffness * STEP,
        );
        let next = vec3_add(
            vec3_add(s.current[i], inertia),
            vec3_add(stiffness, vec3_scale(s.joints[i].gravity, STEP * STEP)),
        );
        s.previous[i] = s.current[i];
        let direction = vec3_normalize(vec3_sub(next, head));
        s.current[i] = vec3_add(head, vec3_scale(direction, s.lengths[i]));
    }
}
fn apply_rotations(world: &mut World, s: &ChainState) -> f32 {
    let mut max_correction = 0.0f32;
    let mut previous_joint_world_q = None;
    for i in 0..s.joints.len() {
        let id = s.joints[i].id;
        let Some(parent) = world.parent_of(id) else {
            continue;
        };
        let parent_q = if i > 0 && parent == s.joints[i - 1].id {
            previous_joint_world_q.unwrap_or([0.0, 0.0, 0.0, 1.0])
        } else {
            world
                .get_component_by_id_as::<TransformComponent>(parent)
                .map(|t| mat_to_quat(t.transform.matrix_world))
                .unwrap_or([0.0, 0.0, 0.0, 1.0])
        };
        let head = if i == 0 {
            let Some(head) = pos(world, id) else { continue };
            head
        } else {
            s.current[i - 1]
        };
        let desired_local = quat_rotate_vec3(
            quat_conjugate(parent_q),
            vec3_normalize(vec3_sub(s.current[i], head)),
        );
        let rest_dir = if i + 1 < s.joints.len() {
            rest(world, s.joints[i + 1].id)
                // Child translation is in the joint's local frame. Convert
                // it through the immutable rest rotation before comparing it
                // with a direction expressed in the parent frame.
                .map(|r| quat_rotate_vec3(s.joints[i].rest_rotation, vec3_normalize(r.translation)))
                .unwrap_or(desired_local)
        } else if i > 0 {
            rest(world, id)
                .map(|r| vec3_normalize(r.translation))
                .unwrap_or(desired_local)
        } else {
            desired_local
        };
        let correction = shortest_arc_quat(rest_dir, desired_local);
        max_correction = max_correction.max(2.0 * correction[3].abs().clamp(0.0, 1.0).acos());
        let rotation = quat_mul(correction, s.joints[i].rest_rotation);
        previous_joint_world_q = Some(quat_mul(parent_q, rotation));
        if let Some(t) = world.get_component_by_id_as_mut::<TransformComponent>(id) {
            t.transform.rotation = rotation;
            t.transform.recompute_model();
        }
    }
    max_correction
}
