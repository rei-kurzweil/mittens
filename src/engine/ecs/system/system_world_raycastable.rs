use super::BvhSystem;
use super::system_world::SystemWorld;
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::{RenderAssets, VisualWorld};

impl SystemWorld {
    /// Register a RenderableComponent instance with the RenderableSystem.
    pub fn register_renderable(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.register_renderable_with_assets(world, visuals, component, None);
    }

    pub(crate) fn register_renderable_with_assets(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
        assets: Option<&RenderAssets>,
    ) {
        let combined_owner = self.combine_mesh.owner_of(world, component);
        if let Some(owner) = combined_owner {
            let keep_sources = world
                .get_component_by_id_as::<crate::engine::ecs::component::CombineMeshComponent>(
                    owner,
                )
                .is_some_and(|combine| combine.keep_transforms);
            if !keep_sources || !BvhSystem::renderable_is_raycastable(world, component) {
                return;
            }
        } else {
            self.renderable
                .register_renderable(world, visuals, component);
        }

        // Cache the renderable's local-space AABB as a child BoundsComponent
        // so layout (and future CPU culling) can read mesh extents without
        // touching the BVH or recomputing from mesh handles each time.
        attach_bounds_for_renderable(world, assets, component);

        // Keep BVH in sync (defer actual build/refit to CommandQueue flush).
        if BvhSystem::renderable_is_raycastable(world, component) {
            self.bvh.queue_renderable_added(component);
        }

        // Keep RayCastSystem's eligibility index in sync for brute-force fallback.
        self.raycast.notify_renderable_added(&*world, component);

        if combined_owner.is_none() {
            self.clipping.register_renderable(world, visuals, component);
        }
    }

    pub(super) fn refresh_raycastable_bindings(&mut self, world: &World, component: ComponentId) {
        use crate::engine::ecs::component::RenderableComponent;
        use std::collections::HashSet;

        let mut renderables = HashSet::new();

        let mut current = world.parent_of(component);
        while let Some(node) = current {
            if world
                .get_component_by_id_as::<RenderableComponent>(node)
                .is_some()
            {
                renderables.insert(node);
                break;
            }
            current = world.parent_of(node);
        }

        // A sidecar governs its owner's renderable descendants, including siblings
        // which may have registered before this marker was generated.
        let mut stack = vec![world.parent_of(component).unwrap_or(component)];
        while let Some(node) = stack.pop() {
            if world
                .get_component_by_id_as::<RenderableComponent>(node)
                .is_some()
            {
                renderables.insert(node);
            }
            for &child in world.children_of(node) {
                stack.push(child);
            }
        }

        for renderable in renderables {
            if BvhSystem::debug_grid_raycast_enabled() {
                eprintln!(
                    "grid_raycast_trace phase=registration_applied marker={component:?} renderable={renderable:?} eligible={}",
                    BvhSystem::renderable_is_raycastable(world, renderable)
                );
            }
            if BvhSystem::renderable_is_raycastable(world, renderable) {
                self.bvh.queue_renderable_added(renderable);
                self.raycast.notify_renderable_added(world, renderable);
            } else {
                self.bvh.queue_renderable_removed(renderable);
                self.raycast.notify_renderable_removed(renderable);
            }
        }
    }
}

fn attach_bounds_for_renderable(
    world: &mut World,
    assets: Option<&RenderAssets>,
    renderable_id: ComponentId,
) {
    use crate::engine::ecs::component::{BoundsComponent, RenderableComponent};
    use crate::engine::graphics::bounds::mesh_local_aabb;

    let already = world
        .children_of(renderable_id)
        .iter()
        .any(|&c| world.get_component_by_id_as::<BoundsComponent>(c).is_some());
    if already {
        return;
    }

    let mesh = match world.get_component_by_id_as::<RenderableComponent>(renderable_id) {
        Some(r) => r.renderable.base_mesh,
        None => return,
    };
    let local = mesh_local_aabb(mesh).or_else(|| {
        let mesh = assets?.cpu_mesh(mesh)?;
        let first = mesh.vertices.first()?.pos;
        let mut bounds = crate::engine::graphics::bounds::Aabb {
            min: first,
            max: first,
        };
        for vertex in &mesh.vertices[1..] {
            for axis in 0..3 {
                bounds.min[axis] = bounds.min[axis].min(vertex.pos[axis]);
                bounds.max[axis] = bounds.max[axis].max(vertex.pos[axis]);
            }
        }
        Some(bounds)
    });
    let Some(local) = local else {
        return;
    };

    let bounds_id = world.add_component(BoundsComponent::new(local));
    let _ = world.add_child(renderable_id, bounds_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::CommandQueue;
    use crate::engine::ecs::component::{
        ClimbableComponent, RaycastableComponent, RenderableComponent, TransformComponent,
    };

    #[test]
    fn late_climbable_sidecar_registers_preexisting_sibling_geometry() {
        let mut world = World::default();
        let mut systems = SystemWorld::default();
        let mut visuals = VisualWorld::default();
        let mut assets = RenderAssets::new();
        let mut queue = CommandQueue::new();
        let owner = world.add_component(TransformComponent::new().with_position(0.0, 0.0, -2.0));
        let r = world.add_component(RenderableComponent::cube());
        world.add_child(owner, r).unwrap();
        world.init_component_tree(owner, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);
        systems.transform_changed(&mut world, &mut visuals, owner);
        assert!(
            systems
                .bvh
                .raycast_renderables_candidates([0.0; 3], [0.0, 0.0, -1.0], 10.0, 0)
                .is_empty()
        );
        let climbable = world.add_component(ClimbableComponent::new());
        world.add_child(owner, climbable).unwrap();
        world.init_component_tree(climbable, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);
        systems.bvh.flush_pending(&world);
        assert!(world.children_of(owner).iter().any(|&id| {
            world
                .get_component_by_id_as::<RaycastableComponent>(id)
                .is_some()
        }));
        assert!(
            systems
                .bvh
                .raycast_renderables_candidates([0.0; 3], [0.0, 0.0, -1.0], 10.0, 0)
                .iter()
                .any(|&(id, _)| id == r)
        );
    }
    #[test]
    fn late_raycastable_init_refreshes_descendant_renderables() {
        let mut world = World::default();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();
        let mut render_assets = RenderAssets::new();

        let raycastable_root = world.add_component(RaycastableComponent::enabled());
        let terrain_root =
            world.add_component_boxed_named("terrain_root", Box::new(TransformComponent::new()));
        let terrain_renderable = world.add_component(RenderableComponent::cube());

        let _ = world.add_child(terrain_root, terrain_renderable);

        world.init_component_tree(terrain_root, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);

        assert!(
            !crate::engine::ecs::system::BvhSystem::renderable_is_raycastable(
                &world,
                terrain_renderable
            ),
            "renderable should not be raycastable before its marker is attached"
        );

        world.add_child(raycastable_root, terrain_root).unwrap();
        world.init_component_tree(raycastable_root, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);

        assert!(
            crate::engine::ecs::system::BvhSystem::renderable_is_raycastable(
                &world,
                terrain_renderable
            ),
            "initializing an ancestor Raycastable should refresh descendant renderable bindings"
        );
    }
}
