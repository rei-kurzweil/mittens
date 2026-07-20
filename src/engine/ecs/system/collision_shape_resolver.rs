use crate::engine::ecs::component::{CollisionShapeComponent, RenderableComponent};
use crate::engine::ecs::system::model::collision_types::CollisionShape;
use crate::engine::ecs::{ComponentId, World};

/// Resolve the exact shape used by collision simulation and diagnostic visualization.
pub fn resolve_collision_shape(world: &World, collision: ComponentId) -> Option<CollisionShape> {
    for child in world.children_of(collision) {
        if let Some(shape) = world.get_component_by_id_as::<CollisionShapeComponent>(*child) {
            return Some(shape.shape.normalized());
        }
    }
    let parent = world.parent_of(collision)?;
    for sibling in world.children_of(parent) {
        if *sibling == collision {
            continue;
        }
        let Some(renderable) = world.get_component_by_id_as::<RenderableComponent>(*sibling) else {
            continue;
        };
        if renderable.renderable.base_mesh
            == crate::engine::graphics::primitives::CpuMeshHandle::CUBE
        {
            return Some(CollisionShape::CUBE());
        }
        if renderable.renderable.base_mesh
            == crate::engine::graphics::primitives::CpuMeshHandle::SPHERE
        {
            return Some(CollisionShape::SPHERE());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::{
        CollisionComponent, CollisionShapeComponent, TransformComponent,
    };

    #[test]
    fn explicit_shape_wins_and_builtin_shapes_are_inferred() {
        let mut world = World::default();
        let transform = world.add_component(TransformComponent::new());
        let cube = world.add_component(RenderableComponent::cube());
        let collision = world.add_component(CollisionComponent::KINEMATIC());
        world.add_child(transform, cube).unwrap();
        world.add_child(transform, collision).unwrap();
        assert_eq!(
            resolve_collision_shape(&world, collision),
            Some(CollisionShape::CUBE())
        );

        let explicit = world.add_component(CollisionShapeComponent::new(CollisionShape::Sphere {
            radius: 0.7,
        }));
        world.add_child(collision, explicit).unwrap();
        assert_eq!(
            resolve_collision_shape(&world, collision),
            Some(CollisionShape::Sphere { radius: 0.7 })
        );

        let sphere_transform = world.add_component(TransformComponent::new());
        let sphere = world.add_component(RenderableComponent::sphere());
        let sphere_collision = world.add_component(CollisionComponent::KINEMATIC());
        world.add_child(sphere_transform, sphere).unwrap();
        world.add_child(sphere_transform, sphere_collision).unwrap();
        assert_eq!(
            resolve_collision_shape(&world, sphere_collision),
            Some(CollisionShape::SPHERE())
        );
    }
}
