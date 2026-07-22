use crate::engine::ecs::component::{
    GrabbableComponent, RaycastableComponent, SelectableComponent, SerializeComponent,
    TransformComponent,
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
                activation_source,
                renderable,
                delta_world,
                screen_pos_px,
                ..
            }) = env.event.as_ref()
            else {
                return;
            };
            let supported_activation = *activation_source == PointerActivationSource::Grip
                || (*activation_source == PointerActivationSource::Trigger
                    && screen_pos_px.is_some());
            if !supported_activation {
                return;
            }
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
        if world
            .get_component_by_id_as::<GrabbableComponent>(grabbable)
            .is_some_and(|component| !component.enabled)
        {
            return;
        }
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

/// Resolve a renderable hit to its grabbable movement target.
///
/// An enabled Selectable sidecar on the same owner wins over Grabbable. Handle-style
/// `Grabbable.parent()` markers resolve to the owner's parent Transform.
pub fn grabbable_owner_for_hit(world: &World, renderable: ComponentId) -> Option<ComponentId> {
    let mut current = Some(renderable);
    while let Some(id) = current {
        if world
            .get_component_by_id_as::<TransformComponent>(id)
            .is_some()
        {
            let selectable_wins = world
                .get_component_by_id_as::<SelectableComponent>(id)
                .is_some_and(|selectable| selectable.enabled)
                || world.children_of(id).iter().any(|child| {
                    world
                        .get_component_by_id_as::<SelectableComponent>(*child)
                        .is_some_and(|selectable| selectable.enabled)
                });
            if selectable_wins {
                return None;
            }
            if let Some(grabbable) = world
                .children_of(id)
                .iter()
                .find_map(|child| world.get_component_by_id_as::<GrabbableComponent>(*child))
            {
                if !grabbable.enabled {
                    return None;
                }
                if !grabbable.move_parent {
                    return Some(id);
                }
                let mut parent = world.parent_of(id);
                while let Some(candidate) = parent {
                    if world
                        .get_component_by_id_as::<TransformComponent>(candidate)
                        .is_some()
                    {
                        return Some(candidate);
                    }
                    parent = world.parent_of(candidate);
                }
                return None;
            }
        }
        current = world.parent_of(id);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::{CommandQueue, Signal};
    use crate::engine::ecs::component::{RenderableComponent, SelectableComponent};

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

    #[test]
    fn selectable_on_wins_over_grabbable_but_selectable_off_does_not() {
        let mut world = World::default();
        let owner = world.add_component(TransformComponent::new());
        let grabbable = world.add_component(GrabbableComponent::new());
        let selectable = world.add_component(SelectableComponent::on());
        let renderable = world.add_component(RenderableComponent::cube());
        world.add_child(owner, grabbable).unwrap();
        world.add_child(owner, selectable).unwrap();
        world.add_child(owner, renderable).unwrap();

        assert_eq!(grabbable_owner_for_hit(&world, renderable), None);
        world
            .get_component_by_id_as_mut::<SelectableComponent>(selectable)
            .unwrap()
            .enabled = false;
        assert_eq!(grabbable_owner_for_hit(&world, renderable), Some(owner));
    }

    #[test]
    fn parent_handle_resolves_to_the_complete_parent_transform() {
        let mut world = World::default();
        let panel = world.add_component(TransformComponent::new());
        let title_bar = world.add_component(TransformComponent::new());
        let grabbable = world.add_component(GrabbableComponent::parent());
        let renderable = world.add_component(RenderableComponent::cube());
        world.add_child(panel, title_bar).unwrap();
        world.add_child(title_bar, grabbable).unwrap();
        world.add_child(title_bar, renderable).unwrap();

        assert_eq!(grabbable_owner_for_hit(&world, renderable), Some(panel));
    }

    #[test]
    fn desktop_trigger_drag_moves_grabbable_but_xr_trigger_does_not() {
        let mut world = World::default();
        let owner = world.add_component(TransformComponent::new());
        let grabbable = world.add_component(GrabbableComponent::new());
        let renderable = world.add_component(RenderableComponent::cube());
        world.add_child(owner, grabbable).unwrap();
        world.add_child(owner, renderable).unwrap();

        let mut rx = RxWorld::default();
        GrabbableSystem::default().install_handlers(&mut rx);
        let drag = |screen_pos_px| {
            Signal::event(
                renderable,
                EventSignal::DragMove {
                    activation_source: PointerActivationSource::Trigger,
                    raycaster: ComponentId::default(),
                    renderable,
                    hit_point: [1.0, 2.0, 3.0],
                    delta_world: [0.25, -0.5, 1.0],
                    screen_pos_px,
                    screen_delta_px: Some((4.0, 8.0)),
                },
            )
        };

        rx.dispatch_event_handlers(&mut world, &drag(Some((20.0, 30.0))));
        let intents = rx.drain_ready_intents();
        assert!(intents.iter().any(|signal| matches!(
            signal.intent.as_ref().map(|intent| &intent.value),
            Some(IntentValue::UpdateTransform { component_ids, translation, .. })
                if component_ids.as_slice() == [owner]
                    && *translation == [0.25, -0.5, 1.0]
        )));

        rx.dispatch_event_handlers(&mut world, &drag(None));
        assert!(rx.drain_ready_intents().is_empty());
    }
}
