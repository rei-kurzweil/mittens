use std::collections::{HashMap, HashSet};

use crate::engine::ecs::component::{
    ColorComponent, EmissiveComponent, OverlayComponent, RaycastableComponent, RenderableComponent,
    SelectableComponent, SerializeComponent, TransformComponent,
};
use crate::engine::ecs::system::{SecondaryMotionChainSnapshot, SecondaryMotionSystem};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::graphics::RenderAssets;
use crate::utils::math::{
    shortest_arc_quat, vec3_add, vec3_len, vec3_normalize, vec3_scale, vec3_sub,
};

const COLLIDER_COLOR: [f32; 4] = [0.0, 0.9, 1.0, 1.0];
const SEGMENT_COLOR: [f32; 4] = [1.0, 0.0, 0.8, 1.0];
const ENDPOINT_COLOR: [f32; 4] = [1.0, 0.9, 0.0, 1.0];
const SEGMENT_THICKNESS: f32 = 0.006;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpringBoneVisualizationRequest {
    pub scope_roots: Vec<ComponentId>,
}

#[derive(Debug, Clone, Copy)]
enum MarkerKind {
    Collider,
    Segment,
    Endpoint,
}

#[derive(Debug, Default)]
pub struct SpringBoneVisualizationSystem {
    requests: HashMap<ComponentId, SpringBoneVisualizationRequest>,
    colliders: HashMap<(ComponentId, ComponentId), ComponentId>,
    segments: HashMap<(ComponentId, usize), ComponentId>,
    endpoints: HashMap<(ComponentId, usize), ComponentId>,
}

impl SpringBoneVisualizationSystem {
    pub fn set_request(&mut self, owner: ComponentId, scope_roots: Vec<ComponentId>) {
        self.requests
            .insert(owner, SpringBoneVisualizationRequest { scope_roots });
    }

    pub fn remove_request(&mut self, owner: ComponentId) {
        self.requests.remove(&owner);
    }

    pub fn requests(&self) -> &HashMap<ComponentId, SpringBoneVisualizationRequest> {
        &self.requests
    }

    pub fn tick_with_queue(
        &mut self,
        world: &mut World,
        secondary_motion: &SecondaryMotionSystem,
        assets: &mut RenderAssets,
        emit: &mut dyn SignalEmitter,
    ) {
        self.requests.retain(|owner, request| {
            if world.get_component_record(*owner).is_none() {
                return false;
            }
            request
                .scope_roots
                .retain(|root| world.get_component_record(*root).is_some());
            true
        });

        let visible: Vec<_> = secondary_motion
            .bound_snapshot(world)
            .into_iter()
            .filter(|chain| {
                self.requests.values().any(|request| {
                    request.scope_roots.iter().any(|root| {
                        is_descendant_or_self(world, *root, chain.gltf)
                            || is_descendant_or_self(world, *root, chain.chain)
                    })
                })
            })
            .collect();

        let wanted_colliders: HashSet<_> = visible
            .iter()
            .flat_map(|chain| chain.colliders.iter().map(|c| (c.config, c.target)))
            .collect();
        let wanted_segments: HashSet<_> = visible
            .iter()
            .flat_map(|chain| (0..chain.segments.len()).map(|index| (chain.chain, index)))
            .collect();
        reconcile_removed(&mut self.colliders, &wanted_colliders, emit);
        reconcile_removed(&mut self.segments, &wanted_segments, emit);
        reconcile_removed(&mut self.endpoints, &wanted_segments, emit);

        for chain in visible {
            self.update_chain(world, assets, emit, &chain);
        }
    }

    fn update_chain(
        &mut self,
        world: &mut World,
        assets: &mut RenderAssets,
        emit: &mut dyn SignalEmitter,
        chain: &SecondaryMotionChainSnapshot,
    ) {
        for collider in &chain.colliders {
            let key = (collider.config, collider.target);
            let marker = *self
                .colliders
                .entry(key)
                .or_insert_with(|| spawn_marker(world, assets, emit, MarkerKind::Collider));
            update_transform(
                emit,
                marker,
                collider.center,
                [0.0, 0.0, 0.0, 1.0],
                [collider.scaled_base_radius * 2.0; 3],
            );
        }

        for (index, segment) in chain.segments.iter().enumerate() {
            let key = (chain.chain, index);
            let segment_marker = *self
                .segments
                .entry(key)
                .or_insert_with(|| spawn_marker(world, assets, emit, MarkerKind::Segment));
            let delta = vec3_sub(segment.end, segment.head);
            let length = vec3_len(delta);
            let midpoint = vec3_scale(vec3_add(segment.head, segment.end), 0.5);
            let rotation = if length > f32::EPSILON {
                shortest_arc_quat([0.0, 0.0, 1.0], vec3_normalize(delta))
            } else {
                [0.0, 0.0, 0.0, 1.0]
            };
            update_transform(
                emit,
                segment_marker,
                midpoint,
                rotation,
                [SEGMENT_THICKNESS, SEGMENT_THICKNESS, length],
            );

            let endpoint_marker = *self
                .endpoints
                .entry(key)
                .or_insert_with(|| spawn_marker(world, assets, emit, MarkerKind::Endpoint));
            update_transform(
                emit,
                endpoint_marker,
                segment.end,
                [0.0, 0.0, 0.0, 1.0],
                [chain.scaled_hit_radius * 2.0; 3],
            );
        }
    }
}

fn reconcile_removed<K: Copy + Eq + std::hash::Hash>(
    markers: &mut HashMap<K, ComponentId>,
    wanted: &HashSet<K>,
    emit: &mut dyn SignalEmitter,
) {
    for key in markers.keys().copied().collect::<Vec<_>>() {
        if !wanted.contains(&key) {
            let root = markers.remove(&key).unwrap();
            emit.push_intent_now(
                root,
                IntentValue::RemoveSubtree {
                    component_ids: vec![root],
                },
            );
        }
    }
}

fn is_descendant_or_self(world: &World, ancestor: ComponentId, node: ComponentId) -> bool {
    let mut current = Some(node);
    while let Some(id) = current {
        if id == ancestor {
            return true;
        }
        current = world.parent_of(id);
    }
    false
}

fn update_transform(
    emit: &mut dyn SignalEmitter,
    marker: ComponentId,
    translation: [f32; 3],
    rotation_quat_xyzw: [f32; 4],
    scale: [f32; 3],
) {
    emit.push_intent_now(
        marker,
        IntentValue::UpdateTransform {
            component_ids: vec![marker],
            translation,
            rotation_quat_xyzw,
            scale,
        },
    );
}

fn spawn_marker(
    world: &mut World,
    assets: &mut RenderAssets,
    emit: &mut dyn SignalEmitter,
    kind: MarkerKind,
) -> ComponentId {
    let (label, renderable, color) = match kind {
        MarkerKind::Collider => (
            "spring_bone_collider_marker",
            RenderableComponent::wireframe_sphere(assets, 12, 24, 0.025),
            COLLIDER_COLOR,
        ),
        MarkerKind::Segment => (
            "spring_bone_segment_marker",
            RenderableComponent::cube(),
            SEGMENT_COLOR,
        ),
        MarkerKind::Endpoint => (
            "spring_bone_endpoint_marker",
            RenderableComponent::wireframe_icosahedron(assets, 1, 1.0, 0.04),
            ENDPOINT_COLOR,
        ),
    };
    let root = world.add_component_boxed_named(label, Box::new(TransformComponent::new()));
    let serialize = world.add_component(SerializeComponent::off());
    let selectable = world.add_component(SelectableComponent::off());
    let overlay = world.add_component(OverlayComponent::new());
    let renderable = world.add_component(renderable);
    let raycastable = world.add_component(RaycastableComponent::disabled());
    let color = world.add_component(ColorComponent::rgba(color[0], color[1], color[2], color[3]));
    let emissive = world.add_component(EmissiveComponent::new(1.0));
    let _ = world.add_child(root, serialize);
    let _ = world.add_child(root, selectable);
    let _ = world.add_child(root, overlay);
    let _ = world.add_child(overlay, renderable);
    let _ = world.add_child(renderable, raycastable);
    let _ = world.add_child(renderable, color);
    let _ = world.add_child(renderable, emissive);
    world.init_component_tree(root, emit);
    root
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::system::{
        SecondaryMotionColliderSnapshot, SecondaryMotionSegmentSnapshot, SystemWorld,
    };
    use crate::engine::ecs::CommandQueue;
    use crate::engine::graphics::VisualWorld;

    #[test]
    fn shared_colliders_are_deduplicated_and_endpoint_scale_uses_hit_radius() {
        let mut world = World::default();
        let gltf = world.add_component(TransformComponent::new());
        let chain_a = world.add_component(TransformComponent::new());
        let chain_b = world.add_component(TransformComponent::new());
        let collider_config = world.add_component(TransformComponent::new());
        let collider_target = world.add_component(TransformComponent::new());
        world.add_child(gltf, chain_a).unwrap();
        world.add_child(gltf, chain_b).unwrap();
        let collider = SecondaryMotionColliderSnapshot {
            config: collider_config,
            target: collider_target,
            center: [1.0, 2.0, 3.0],
            scaled_base_radius: 0.4,
        };
        let snapshot = |chain| SecondaryMotionChainSnapshot {
            gltf,
            chain,
            enabled: true,
            hit_radius: 0.1,
            scaled_hit_radius: 0.1,
            segments: vec![SecondaryMotionSegmentSnapshot {
                head: [0.0, 0.0, 0.0],
                end: [0.0, 1.0, 0.0],
            }],
            colliders: vec![collider.clone()],
        };
        let mut system = SpringBoneVisualizationSystem::default();
        let mut assets = RenderAssets::new();
        let mut queue = CommandQueue::new();
        system.update_chain(&mut world, &mut assets, &mut queue, &snapshot(chain_a));
        system.update_chain(&mut world, &mut assets, &mut queue, &snapshot(chain_b));
        assert_eq!(system.colliders.len(), 1);
        assert_eq!(system.segments.len(), 2);
        assert_eq!(system.endpoints.len(), 2);

        let endpoint = system.endpoints[&(chain_a, 0)];
        let collider_marker = system.colliders[&(collider_config, collider_target)];
        let mut systems = SystemWorld::default();
        let mut visuals = VisualWorld::default();
        systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);
        assert_eq!(
            world
                .get_component_by_id_as::<TransformComponent>(endpoint)
                .unwrap()
                .transform
                .scale,
            [0.2; 3]
        );
        assert_eq!(
            world
                .get_component_by_id_as::<TransformComponent>(collider_marker)
                .unwrap()
                .transform
                .scale,
            [0.8; 3]
        );
    }
}
