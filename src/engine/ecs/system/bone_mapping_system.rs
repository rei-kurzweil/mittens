use crate::engine::ecs::component::TransformComponent;
use crate::engine::ecs::{ComponentId, World};
use crate::utils::math::{vec3_len, vec3_sub};

/// Stateless utility for resolving semantic bone landmarks to ComponentIds in a live skeleton.
///
/// All functions are free (associated) functions — no instance state.
/// Called once during `AvatarControlSystem::try_init_splices`; returns resolved IDs
/// that AVC uses to wire IK chains.
pub struct BoneMappingSystem;

/// Resolved upper-arm → lower-arm → hand chain for TwoBoneIK setup.
pub struct ResolvedArmChain {
    pub upper_arm: ComponentId,
    pub lower_arm: ComponentId,
    pub hand: ComponentId,
}

/// Resolved spine chain for FABRIK setup.
///
/// `chain` is ordered hips → ... → head (root-first FABRIK convention).
/// Intermediate joints (spine/chest/upper_chest/neck) are whatever TC ancestors
/// the topology walk produced between hips and head — count varies by rig.
pub struct ResolvedSpineChain {
    pub hips: ComponentId,
    pub head: ComponentId,
    pub chain: Vec<ComponentId>,
}

impl BoneMappingSystem {
    /// Resolve a 2-bone arm chain from (optional) explicit names + topology fallback.
    ///
    /// Resolution order for each joint:
    ///   1. Explicit name, if provided — look up by `#name` MMQ selector under `model_root`.
    ///   2. Topology derivation — walk TC parent chain from the joint below, using
    ///      `tc_ancestor_at_distance` with the given `min_bone_length` threshold.
    ///
    /// `min_bone_length`: if `Some(d)`, topology derivation skips TC ancestors closer
    /// than `d` metres to the previous anchor, filtering out short helper bones.
    /// Pass `None` to always use the immediate TC parent.
    ///
    /// Returns `None` if `hand_name` is not found under `model_root`.
    pub fn resolve_arm_chain(
        world: &World,
        model_root: ComponentId,
        hand_name: &str,
        lower_arm_name: Option<&str>,
        upper_arm_name: Option<&str>,
        min_bone_length: Option<f32>,
    ) -> Option<ResolvedArmChain> {
        let hand = world.find_component(model_root, &format!("#{}", hand_name))?;

        let lower_arm = if let Some(name) = lower_arm_name {
            world.find_component(model_root, &format!("#{}", name))?
        } else {
            Self::tc_ancestor_at_distance(world, hand, min_bone_length)?
        };

        let upper_arm = if let Some(name) = upper_arm_name {
            world.find_component(model_root, &format!("#{}", name))?
        } else {
            Self::tc_ancestor_at_distance(world, lower_arm, min_bone_length)?
        };

        Some(ResolvedArmChain { upper_arm, lower_arm, hand })
    }

    /// Resolve a spine chain from head bone up to (optionally named) hips bone.
    ///
    /// Walks UP from `head_id` via `tc_ancestor_at_distance` (threshold ~0.03m to
    /// skip helper bones), collecting TC joints.  Stops when it hits `hips_name`
    /// (by component name) if provided, or after at most 8 hops otherwise.
    ///
    /// Returns the chain in HIPS → HEAD order (FABRIK convention: root first).
    /// Returns `None` if the walk produces fewer than 2 joints.
    pub fn resolve_spine_chain(
        world: &World,
        model_root: ComponentId,
        head_id: ComponentId,
        hips_name: Option<&str>,
        min_bone_length: Option<f32>,
    ) -> Option<ResolvedSpineChain> {
        let hips_id = hips_name.and_then(|n| world.find_component(model_root, &format!("#{}", n)));
        let mut up: Vec<ComponentId> = vec![head_id];
        let mut cur = head_id;
        for _ in 0..8 {
            let parent = Self::tc_ancestor_at_distance(world, cur, min_bone_length)?;
            up.push(parent);
            // Stop if we hit the named hips.
            if Some(parent) == hips_id { break; }
            // Or stop if we've stepped above model_root.
            if parent == model_root { break; }
            cur = parent;
        }
        if up.len() < 2 { return None; }
        up.reverse();
        let hips = *up.first().unwrap();
        let head = *up.last().unwrap();
        Some(ResolvedSpineChain { hips, head, chain: up })
    }

    /// Walk upward from `start`, returning the nearest TC ancestor whose world position
    /// is at least `min_dist` metres away from `start`.
    ///
    /// If `min_dist` is `None`, returns the immediate TC parent (no distance filtering).
    /// Returns `None` if no TC parent is found, or if the walk exceeds 32 steps.
    pub fn tc_ancestor_at_distance(
        world: &World,
        start: ComponentId,
        min_dist: Option<f32>,
    ) -> Option<ComponentId> {
        let anchor_pos = tc_world_pos(world, start)?;
        let mut cur = start;

        for _ in 0..32 {
            let parent = world.parent_of(cur)?;
            // Parent must be a TC to count as an arm joint.
            if world.get_component_by_id_as::<TransformComponent>(parent).is_none() {
                return None;
            }
            if let Some(min_d) = min_dist {
                let parent_pos = tc_world_pos(world, parent)?;
                let dist = vec3_len(vec3_sub(parent_pos, anchor_pos));
                if dist < min_d {
                    // Too close — this is a helper bone; keep walking.
                    cur = parent;
                    continue;
                }
            }
            return Some(parent);
        }
        None
    }

    /// Find the first TC ancestor of `start` that has >= `min_tc_children` TC children.
    ///
    /// Used for shoulder girdle detection (first 3-way split above hips) and hip detection.
    /// Returns `None` if no such ancestor exists within 32 steps.
    pub fn find_branching_ancestor(
        world: &World,
        start: ComponentId,
        min_tc_children: usize,
    ) -> Option<ComponentId> {
        let mut cur = start;
        for _ in 0..32 {
            let parent = world.parent_of(cur)?;
            if world.get_component_by_id_as::<TransformComponent>(parent).is_none() {
                return None;
            }
            let tc_child_count = world
                .children_of(parent)
                .iter()
                .filter(|&&ch| {
                    world
                        .get_component_by_id_as::<TransformComponent>(ch)
                        .is_some()
                })
                .count();
            if tc_child_count >= min_tc_children {
                return Some(parent);
            }
            cur = parent;
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn tc_world_pos(world: &World, id: ComponentId) -> Option<[f32; 3]> {
    world
        .get_component_by_id_as::<TransformComponent>(id)
        .map(|t| {
            let m = t.transform.matrix_world;
            [m[3][0], m[3][1], m[3][2]]
        })
}

