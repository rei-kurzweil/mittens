use crate::engine::ecs::component::{
    ColorComponent, GLTFComponent, OverlayComponent, RaycastableComponent, RenderableComponent,
    SignalRouteUpwardComponent, TransformComponent,
};
use crate::engine::ecs::system::GLTFSystem;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::graphics::VisualWorld;
use crate::engine::user_input::InputState;
use std::collections::HashMap;

const VIZ_BOX_SCALE: f32 = 0.03;

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

        for &joint_transform in joint_transforms {
            if world
                .get_component_by_id_as::<TransformComponent>(joint_transform)
                .is_none()
            {
                continue;
            }

            let marker_root = spawn_joint_marker(world, emit, joint_transform);
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
    joint_transform: ComponentId,
) -> ComponentId {
    let marker_root = world.add_component_boxed_named(
        "armature_joint_marker",
        Box::new(TransformComponent::new().with_scale(VIZ_BOX_SCALE, VIZ_BOX_SCALE, VIZ_BOX_SCALE)),
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
        Box::new(RenderableComponent::cube()),
    );
    let raycastable = world.add_component_boxed_named(
        "armature_joint_marker_raycastable",
        Box::new(RaycastableComponent::enabled()),
    );
    let color = world.add_component_boxed_named(
        "armature_joint_marker_color",
        Box::new(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0)),
    );

    let _ = world.add_child(joint_transform, marker_root);
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
    use crate::engine::graphics::{RenderAssets, VisualWorld};

    #[test]
    fn armature_visibility_spawns_once_and_removes_idempotently() {
        let mut world = World::default();
        let mut visuals = VisualWorld::default();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();

        let editor_root = world.add_component(TransformComponent::new());
        let gltf_id = world.add_component(GLTFComponent::new("cat.glb"));
        let joint_a = world.add_component(TransformComponent::new());
        let joint_b = world.add_component(TransformComponent::new());
        let _ = world.add_child(editor_root, gltf_id);
        let _ = world.add_child(editor_root, joint_a);
        let _ = world.add_child(editor_root, joint_b);

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
            Some(2)
        );
        assert_eq!(world.children_of(joint_a).len(), 1);
        assert_eq!(world.children_of(joint_b).len(), 1);

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
            Some(2)
        );
        assert_eq!(world.children_of(joint_a).len(), 1);
        assert_eq!(world.children_of(joint_b).len(), 1);

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
        assert!(world.children_of(joint_a).is_empty());
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
}
