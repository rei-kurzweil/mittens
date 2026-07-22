use crate::engine::ecs::component::{
    GrabbableComponent, RaycastableComponent, SerializeComponent, TransformComponent,
};
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::{ComponentId, SignalEmitter, World};
use crate::engine::ecs::{EventSignal, IntentValue, PointerActivationSource, RxWorld, SignalKind};

#[derive(Debug, Default)]
pub struct GrabbableSystem {
    handlers_installed: bool,
}

impl GrabbableSystem {
    pub fn install_handlers(&mut self, rx: &mut RxWorld) {
        if self.handlers_installed {
            return;
        }
        rx.add_global_handler_closure(SignalKind::DragMove, |world, emit, env| {
            let Some(EventSignal::DragMove {
                activation_source: PointerActivationSource::Grip,
                renderable,
                delta_world,
                ..
            }) = env.event.as_ref()
            else {
                return;
            };
            let Some(owner) = grabbable_owner_for_hit(world, *renderable) else {
                return;
            };
            let local_delta = world_delta_to_parent_local(world, owner, *delta_world);
            let Some(transform) = world.get_component_by_id_as::<TransformComponent>(owner) else {
                return;
            };
            let mut translation = transform.transform.translation;
            for i in 0..3 {
                translation[i] += local_delta[i];
            }
            emit.push_intent_now(
                owner,
                IntentValue::UpdateTransform {
                    component_ids: vec![owner],
                    translation,
                    rotation_quat_xyzw: transform.transform.rotation,
                    scale: transform.transform.scale,
                },
            );
        });
        self.handlers_installed = true;
    }

    pub fn register(
        &mut self,
        world: &mut World,
        grabbable: ComponentId,
        emit: &mut dyn SignalEmitter,
    ) {
        let Some(owner) = world.parent_of(grabbable).filter(|id| {
            world
                .get_component_by_id_as::<TransformComponent>(*id)
                .is_some()
        }) else {
            return;
        };
        let has_immediate_raycastable = world.children_of(owner).iter().any(|child| {
            world
                .get_component_by_id_as::<RaycastableComponent>(*child)
                .is_some()
        });
        if has_immediate_raycastable {
            return;
        }
        let raycastable = world.add_component_boxed_named(
            "grabbable_generated_raycastable",
            Box::new(RaycastableComponent::enabled()),
        );
        let serialize = world.add_component(SerializeComponent::off());
        let _ = world.add_child(raycastable, serialize);
        if world.add_child(owner, raycastable).is_ok() {
            world.init_component_tree(raycastable, emit);
        }
    }
}

fn world_delta_to_parent_local(
    world: &World,
    owner: ComponentId,
    delta_world: [f32; 3],
) -> [f32; 3] {
    let Some(parent) = world.parent_of(owner) else {
        return delta_world;
    };
    let Some(parent_world) = TransformSystem::world_model(world, parent) else {
        return delta_world;
    };
    let Some(inv) = crate::utils::math::mat4_inverse(parent_world) else {
        return delta_world;
    };
    [
        inv[0][0] * delta_world[0] + inv[1][0] * delta_world[1] + inv[2][0] * delta_world[2],
        inv[0][1] * delta_world[0] + inv[1][1] * delta_world[1] + inv[2][1] * delta_world[2],
        inv[0][2] * delta_world[0] + inv[1][2] * delta_world[1] + inv[2][2] * delta_world[2],
    ]
}

/// Resolve a renderable hit to the nearest Transform carrying an immediate Grabbable sidecar.
pub fn grabbable_owner_for_hit(world: &World, renderable: ComponentId) -> Option<ComponentId> {
    let mut current = Some(renderable);
    while let Some(id) = current {
        if world
            .get_component_by_id_as::<TransformComponent>(id)
            .is_some()
            && world.children_of(id).iter().any(|child| {
                world
                    .get_component_by_id_as::<GrabbableComponent>(*child)
                    .is_some()
            })
        {
            return Some(id);
        }
        current = world.parent_of(id);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::CommandQueue;
    use crate::engine::ecs::component::RenderableComponent;

    #[test]
    fn registration_generates_runtime_raycastable_and_nested_hits_resolve_to_owner() {
        let mut world = World::default();
        let owner = world.add_component(TransformComponent::new());
        let grabbable = world.add_component(GrabbableComponent::new());
        let nested = world.add_component(TransformComponent::new());
        let deep = world.add_component(TransformComponent::new());
        let renderable = world.add_component(RenderableComponent::cube());
        world.add_child(owner, grabbable).unwrap();
        world.add_child(owner, nested).unwrap();
        world.add_child(nested, deep).unwrap();
        world.add_child(deep, renderable).unwrap();

        let mut queue = CommandQueue::new();
        let mut system = GrabbableSystem::default();
        system.register(&mut world, grabbable, &mut queue);

        let generated = world
            .children_of(owner)
            .iter()
            .copied()
            .find(|child| world.component_label(*child) == Some("grabbable_generated_raycastable"))
            .expect("generated raycastable");
        assert!(
            world
                .get_component_by_id_as::<RaycastableComponent>(generated)
                .is_some()
        );
        assert!(world.children_of(generated).iter().any(|child| {
            world
                .get_component_by_id_as::<SerializeComponent>(*child)
                .is_some_and(|serialize| !serialize.enabled)
        }));
        assert_eq!(grabbable_owner_for_hit(&world, renderable), Some(owner));
    }

    #[test]
    fn explicit_owner_raycastable_is_preserved() {
        let mut world = World::default();
        let owner = world.add_component(TransformComponent::new());
        let grabbable = world.add_component(GrabbableComponent::new());
        let explicit = world.add_component(RaycastableComponent::disabled());
        world.add_child(owner, grabbable).unwrap();
        world.add_child(owner, explicit).unwrap();
        let mut queue = CommandQueue::new();
        GrabbableSystem::default().register(&mut world, grabbable, &mut queue);
        assert_eq!(
            world
                .children_of(owner)
                .iter()
                .filter(|child| {
                    world
                        .get_component_by_id_as::<RaycastableComponent>(**child)
                        .is_some()
                })
                .count(),
            1
        );
        assert!(
            !world
                .get_component_by_id_as::<RaycastableComponent>(explicit)
                .unwrap()
                .enable
        );
    }
}
