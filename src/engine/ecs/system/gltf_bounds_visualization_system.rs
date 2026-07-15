use crate::engine::ecs::component::{
    BoundsComponent, ColorComponent, ComponentRef, GLTFComponent, MeshComponent, OpacityComponent,
    OverlayComponent, RaycastableComponent, RenderableComponent, SelectableComponent,
    SerializeComponent, TransformComponent, TransformParentComponent,
};
use crate::engine::ecs::system::GLTFSystem;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::graphics::RenderAssets;
use crate::engine::graphics::VisualWorld;
use std::collections::{HashMap, HashSet};

const BOUNDS_EDGE_THICKNESS: f32 = 0.02;

#[derive(Debug, Clone, Copy)]
struct BoundsMarker {
    target: ComponentId,
    root: ComponentId,
}

/// Draws imported GLTF mesh bounds without inserting debug nodes into the GLTF hierarchy.
#[derive(Debug, Default)]
pub struct GltfBoundsVisualizationSystem {
    markers: HashMap<ComponentId, Vec<BoundsMarker>>,
}

impl GltfBoundsVisualizationSystem {
    pub fn tick_with_queue(
        &mut self,
        world: &mut World,
        gltf_system: &GLTFSystem,
        _visuals: &mut VisualWorld,
        render_assets: &mut RenderAssets,
        emit: &mut dyn SignalEmitter,
    ) {
        self.cleanup(world);

        for gltf_id in gltf_system.tracked_components() {
            let Some(gltf) = world.get_component_by_id_as::<GLTFComponent>(gltf_id) else {
                continue;
            };
            if !gltf.spawned {
                continue;
            }
            if gltf.bounds_visible {
                self.ensure_markers(world, render_assets, emit, gltf_id);
            } else {
                self.remove_markers(emit, gltf_id);
            }
        }
    }

    fn cleanup(&mut self, world: &World) {
        self.markers.retain(|gltf_id, markers| {
            if world.get_component_record(*gltf_id).is_none() {
                return false;
            }
            markers.retain(|marker| {
                world.get_component_record(marker.root).is_some()
                    && world.get_component_record(marker.target).is_some()
            });
            true
        });
    }

    fn ensure_markers(
        &mut self,
        world: &mut World,
        render_assets: &mut RenderAssets,
        emit: &mut dyn SignalEmitter,
        gltf_id: ComponentId,
    ) {
        let existing_targets: HashSet<ComponentId> = self
            .markers
            .get(&gltf_id)
            .into_iter()
            .flatten()
            .map(|marker| marker.target)
            .collect();
        let mut stack = world
            .get_component_by_id_as::<GLTFComponent>(gltf_id)
            .map(|gltf| gltf.spawned_node_transforms.clone())
            .unwrap_or_default();
        let mut additions = Vec::new();

        while let Some(node_transform) = stack.pop() {
            let children = world.children_of(node_transform).to_vec();
            for child in children {
                if world
                    .get_component_by_id_as::<TransformComponent>(child)
                    .is_some()
                {
                    stack.push(child);
                }
                if existing_targets.contains(&child)
                    || world
                        .get_component_by_id_as::<RenderableComponent>(child)
                        .and_then(RenderableComponent::get_handle)
                        .is_none()
                    || !world.children_of(child).iter().any(|&sidecar| {
                        world
                            .get_component_by_id_as::<MeshComponent>(sidecar)
                            .is_some()
                    })
                {
                    continue;
                }
                let Some(bounds) = world.children_of(child).iter().find_map(|&sidecar| {
                    world
                        .get_component_by_id_as::<BoundsComponent>(sidecar)
                        .map(|bounds| bounds.local)
                }) else {
                    continue;
                };
                additions.push(spawn_marker(world, render_assets, emit, child, bounds));
            }
        }

        self.markers.entry(gltf_id).or_default().extend(additions);
    }

    fn remove_markers(&mut self, emit: &mut dyn SignalEmitter, gltf_id: ComponentId) {
        let Some(markers) = self.markers.get_mut(&gltf_id) else {
            return;
        };
        for marker in markers.drain(..) {
            emit.push_intent_now(
                marker.root,
                IntentValue::RemoveSubtree {
                    component_ids: vec![marker.root],
                },
            );
        }
    }
}

fn spawn_marker(
    world: &mut World,
    render_assets: &mut RenderAssets,
    emit: &mut dyn SignalEmitter,
    target: ComponentId,
    bounds: crate::engine::graphics::bounds::Aabb,
) -> BoundsMarker {
    let center = bounds.center();
    let local = TransformComponent::new()
        .with_position(center[0], center[1], center[2])
        .with_scale(bounds.width(), bounds.height(), bounds.depth());
    let target_guid = world
        .get_component_record(target)
        .expect("bounds target must exist")
        .guid;
    let root = world.add_component_boxed_named(
        "gltf_bounds_marker",
        Box::new(
            TransformParentComponent::new().with_target_source(ComponentRef::Guid(target_guid)),
        ),
    );
    let local = world.add_component(local);
    let selectable = world.add_component(SelectableComponent::off());
    let serialize = world.add_component(SerializeComponent::off());
    let overlay = world.add_component(OverlayComponent::new());
    let renderable = world.add_component(RenderableComponent::wireframe_box(
        render_assets,
        BOUNDS_EDGE_THICKNESS,
    ));
    let raycastable = world.add_component(RaycastableComponent::disabled());
    let color = world.add_component(ColorComponent::rgba(1.0, 0.42, 0.05, 1.0));
    let opacity = world.add_component(OpacityComponent::new().with_opacity(0.9));
    let _ = world.add_child(root, local);
    let _ = world.add_child(root, selectable);
    let _ = world.add_child(root, serialize);
    let _ = world.add_child(local, overlay);
    let _ = world.add_child(overlay, renderable);
    let _ = world.add_child(renderable, raycastable);
    let _ = world.add_child(renderable, color);
    let _ = world.add_child(renderable, opacity);
    world.init_component_tree(root, emit);

    BoundsMarker { target, root }
}
