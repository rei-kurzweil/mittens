use crate::engine::ecs::component::{
    ColorComponent, GLTFComponent, OverlayComponent, RaycastableComponent, RenderableComponent,
    SignalRouteUpwardComponent, TransformComponent,
};
use crate::engine::ecs::system::GLTFSystem;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::graphics::VisualWorld;
use crate::engine::user_input::InputState;
use crate::utils::math::{shortest_arc_quat, vec3_len, vec3_normalize, vec3_scale};
use std::collections::{HashMap, HashSet};

const ARMATURE_MARKER_RADIUS: f32 = 0.03;
const MIN_ARMATURE_EDGE_LENGTH: f32 = 1.0e-6;

#[derive(Debug, Default)]
pub struct ArmatureVisualizationSystem {
    visualization_roots: HashMap<ComponentId, Vec<ComponentId>>,
}

impl ArmatureVisualizationSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn registry_entry(&self, component_id: ComponentId) -> Option<&Vec<ComponentId>> {
        self.visualization_roots.get(&component_id)
    }

    pub fn tick_with_queue(
        &mut self,
        world: &mut World,
        gltf_system: &GLTFSystem,
        _visuals: &mut VisualWorld,
        emit: &mut dyn SignalEmitter,
        _dt_sec: f32,
    ) {
        self.cleanup_dead_entries(world);

        for component_id in gltf_system.tracked_components() {
            let Some(gltf) = world.get_component_by_id_as::<GLTFComponent>(component_id) else {
                continue;
            };
            if !gltf.spawned {
                continue;
            }

            let wants_visible = gltf.armature_visible;
            let has_visualizations = self
                .visualization_roots
                .get(&component_id)
                .is_some_and(|roots| !roots.is_empty());

            if wants_visible {
                if has_visualizations {
                    continue;
                }
                let joint_transforms = gltf.armature_joint_transforms.clone();
                self.ensure_visualizations(world, emit, component_id, &joint_transforms);
            } else {
                if !has_visualizations {
                    continue;
                }
                self.remove_visualizations(emit, component_id);
            }
        }
    }

    fn cleanup_dead_entries(&mut self, world: &World) {
        self.visualization_roots
            .retain(|component_id, _| world.get_component_record(*component_id).is_some());
    }

    fn ensure_visualizations(
        &mut self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        component_id: ComponentId,
        joint_transforms: &[ComponentId],
    ) {
        let existing = self.visualization_roots.entry(component_id).or_default();
        if !existing.is_empty() {
            existing.retain(|marker_root| world.get_component_record(*marker_root).is_some());
        }
        if !existing.is_empty() || joint_transforms.is_empty() {
            return;
        }

        let joint_set: HashSet<_> = joint_transforms.iter().copied().collect();
        let edges: Vec<_> = joint_transforms
            .iter()
            .copied()
            .filter_map(|child_joint| {
                let parent_joint = world.parent_of(child_joint)?;
                if !joint_set.contains(&parent_joint) {
                    return None;
                }
                let edge = world
                    .get_component_by_id_as::<TransformComponent>(child_joint)?
                    .transform
                    .translation;
                (vec3_len(edge) > MIN_ARMATURE_EDGE_LENGTH).then_some((parent_joint, edge))
            })
            .collect();

        // Iterate in the GLTF's recorded joint order. This makes branching-joint marker creation
        // deterministic while still producing one cone for every direct armature edge.
        for (parent_joint, edge) in edges {
            let marker_root = spawn_joint_marker(world, emit, parent_joint, edge);
            existing.push(marker_root);
        }
    }

    fn remove_visualizations(&mut self, emit: &mut dyn SignalEmitter, component_id: ComponentId) {
        let Some(existing) = self.visualization_roots.get_mut(&component_id) else {
            return;
        };

        for &marker_root in existing.iter() {
            emit.push_intent_now(
                marker_root,
                IntentValue::RemoveSubtree {
                    component_ids: vec![marker_root],
                },
            );
        }
        existing.clear();
    }
}

fn spawn_joint_marker(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    parent_joint: ComponentId,
    edge: [f32; 3],
) -> ComponentId {
    let edge_length = vec3_len(edge);
    let rotation = shortest_arc_quat([0.0, 0.0, 1.0], vec3_normalize(edge));
    let midpoint = vec3_scale(edge, 0.5);

    // The segment is authored in the parent joint's local space, so inherited non-uniform scale
    // may distort its radius, but both endpoints still coincide with the imported joints. Never
    // compensate by mutating or normalizing the imported armature transforms.
    let marker_root = world.add_component_boxed_named(
        "armature_joint_marker",
        Box::new(
            TransformComponent::new()
                .with_position(midpoint[0], midpoint[1], midpoint[2])
                .with_rotation_quat(rotation)
                .with_scale(ARMATURE_MARKER_RADIUS, ARMATURE_MARKER_RADIUS, edge_length),
        ),
    );
    let route_up = world.add_component_boxed_named(
        "armature_joint_marker_route",
        Box::new(SignalRouteUpwardComponent::new(
            "update_transform",
            "transform",
        )),
    );
    let overlay = world.add_component_boxed_named(
        "armature_joint_marker_overlay",
        Box::new(OverlayComponent::new()),
    );
    let renderable = world.add_component_boxed_named(
        "armature_joint_marker_renderable",
        Box::new(RenderableComponent::cone()),
    );
    let raycastable = world.add_component_boxed_named(
        "armature_joint_marker_raycastable",
        Box::new(RaycastableComponent::enabled()),
    );
    let color = world.add_component_boxed_named(
        "armature_joint_marker_color",
        Box::new(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0)),
    );

    let _ = world.add_child(parent_joint, marker_root);
    let _ = world.add_child(marker_root, route_up);
    let _ = world.add_child(marker_root, overlay);
    let _ = world.add_child(overlay, renderable);
    let _ = world.add_child(renderable, raycastable);
    let _ = world.add_child(renderable, color);
    world.init_component_tree(marker_root, emit);
    marker_root
}

impl crate::engine::ecs::system::System for ArmatureVisualizationSystem {
    fn tick(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::CommandQueue;
    use crate::engine::ecs::component::GLTFComponent;
    use crate::engine::ecs::system::SystemWorld;
    use crate::engine::graphics::primitives::CpuMeshHandle;
    use crate::engine::graphics::{RenderAssets, VisualWorld};
    use crate::utils::math::quat_rotate_vec3;

    #[test]
    fn armature_visibility_spawns_once_and_removes_idempotently() {
        let mut world = World::default();
        let mut visuals = VisualWorld::default();
        let mut render_assets = RenderAssets::new();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();

        let editor_root = world.add_component(TransformComponent::new());
        let gltf_id = world.add_component(GLTFComponent::new("cat.glb"));
        let joint_a = world.add_component(TransformComponent::new());
        let joint_b = world.add_component(TransformComponent::new().with_position(0.0, 0.2, 0.0));
        let _ = world.add_child(editor_root, gltf_id);
        let _ = world.add_child(editor_root, joint_a);
        let _ = world.add_child(joint_a, joint_b);

        let gltf = world
            .get_component_by_id_as_mut::<GLTFComponent>(gltf_id)
            .expect("gltf");
        gltf.spawned = true;
        gltf.armature_visible = true;
        gltf.armature_joint_transforms = vec![joint_a, joint_b];
        systems.gltf.register_component(gltf_id);

        systems.armature_visualization.tick_with_queue(
            &mut world,
            &systems.gltf,
            &mut visuals,
            &mut queue,
            0.016,
        );
        assert_eq!(
            systems
                .armature_visualization
                .registry_entry(gltf_id)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(world.children_of(joint_a).len(), 2);
        assert!(world.children_of(joint_b).is_empty());

        systems.armature_visualization.tick_with_queue(
            &mut world,
            &systems.gltf,
            &mut visuals,
            &mut queue,
            0.016,
        );
        assert_eq!(
            systems
                .armature_visualization
                .registry_entry(gltf_id)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(world.children_of(joint_a).len(), 2);
        assert!(world.children_of(joint_b).is_empty());

        world
            .get_component_by_id_as_mut::<GLTFComponent>(gltf_id)
            .expect("gltf")
            .armature_visible = false;
        systems.armature_visualization.tick_with_queue(
            &mut world,
            &systems.gltf,
            &mut visuals,
            &mut queue,
            0.016,
        );
        systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);
        assert_eq!(
            systems
                .armature_visualization
                .registry_entry(gltf_id)
                .map(Vec::len),
            Some(0)
        );
        assert_eq!(world.children_of(joint_a), &[joint_b]);
        assert!(world.children_of(joint_b).is_empty());

        systems.armature_visualization.tick_with_queue(
            &mut world,
            &systems.gltf,
            &mut visuals,
            &mut queue,
            0.016,
        );
        systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);
        assert_eq!(
            systems
                .armature_visualization
                .registry_entry(gltf_id)
                .map(Vec::len),
            Some(0)
        );
    }

    #[test]
    fn armature_edge_spawns_directional_cone_in_parent_local_space() {
        let mut world = World::default();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();

        let gltf_id = world.add_component(GLTFComponent::new("branch.glb"));
        let parent = world.add_component(TransformComponent::new().with_scale(2.0, 3.0, 4.0));
        let child = world.add_component(TransformComponent::new().with_position(0.0, 2.0, 0.0));
        let _ = world.add_child(parent, child);

        let gltf = world
            .get_component_by_id_as_mut::<GLTFComponent>(gltf_id)
            .expect("gltf");
        gltf.spawned = true;
        gltf.armature_visible = true;
        gltf.armature_joint_transforms = vec![parent, child];
        systems.gltf.register_component(gltf_id);

        systems.armature_visualization.tick_with_queue(
            &mut world,
            &systems.gltf,
            &mut visuals,
            &mut queue,
            0.016,
        );

        let marker = systems
            .armature_visualization
            .registry_entry(gltf_id)
            .and_then(|markers| markers.first())
            .copied()
            .expect("edge marker");
        assert_eq!(world.parent_of(marker), Some(parent));

        let transform = world
            .get_component_by_id_as::<TransformComponent>(marker)
            .expect("marker transform");
        assert_eq!(transform.transform.translation, [0.0, 1.0, 0.0]);
        assert_eq!(
            transform.transform.scale,
            [ARMATURE_MARKER_RADIUS, ARMATURE_MARKER_RADIUS, 2.0]
        );
        let direction = quat_rotate_vec3(transform.transform.rotation, [0.0, 0.0, 1.0]);
        assert!((direction[0] - 0.0).abs() < 1.0e-5);
        assert!((direction[1] - 1.0).abs() < 1.0e-5);
        assert!((direction[2] - 0.0).abs() < 1.0e-5);

        let renderable = world
            .find_component(marker, "Renderable")
            .and_then(|id| world.get_component_by_id_as::<RenderableComponent>(id))
            .expect("marker renderable");
        assert_eq!(renderable.renderable.base_mesh, CpuMeshHandle::CONE);

        // The marker inherits the parent's non-uniform scale without changing either imported
        // transform; its tip still lands at the child's local origin.
        assert_eq!(
            world
                .get_component_by_id_as::<TransformComponent>(parent)
                .expect("parent")
                .transform
                .scale,
            [2.0, 3.0, 4.0]
        );
        assert_eq!(
            world
                .get_component_by_id_as::<TransformComponent>(child)
                .expect("child")
                .transform
                .translation,
            [0.0, 2.0, 0.0]
        );
    }

    #[test]
    fn branching_edges_follow_recorded_joint_order_and_skip_zero_length_edges() {
        let mut world = World::default();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();

        let gltf_id = world.add_component(GLTFComponent::new("branch.glb"));
        let parent = world.add_component(TransformComponent::new());
        let child_b = world.add_component(TransformComponent::new().with_position(0.0, 0.0, 2.0));
        let zero_length_child = world.add_component(TransformComponent::new());
        let child_a = world.add_component(TransformComponent::new().with_position(1.0, 0.0, 0.0));
        let _ = world.add_child(parent, child_a);
        let _ = world.add_child(parent, child_b);
        let _ = world.add_child(parent, zero_length_child);

        let gltf = world
            .get_component_by_id_as_mut::<GLTFComponent>(gltf_id)
            .expect("gltf");
        gltf.spawned = true;
        gltf.armature_visible = true;
        gltf.armature_joint_transforms = vec![parent, child_b, zero_length_child, child_a];
        systems.gltf.register_component(gltf_id);

        systems.armature_visualization.tick_with_queue(
            &mut world,
            &systems.gltf,
            &mut visuals,
            &mut queue,
            0.016,
        );

        let markers = systems
            .armature_visualization
            .registry_entry(gltf_id)
            .expect("marker registry");
        assert_eq!(markers.len(), 2);
        let translations: Vec<_> = markers
            .iter()
            .map(|marker| {
                world
                    .get_component_by_id_as::<TransformComponent>(*marker)
                    .expect("marker")
                    .transform
                    .translation
            })
            .collect();
        assert_eq!(translations, vec![[0.0, 0.0, 1.0], [0.5, 0.0, 0.0]]);
    }
}
