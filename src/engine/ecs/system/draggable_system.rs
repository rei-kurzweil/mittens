use crate::engine::ecs::component::{
    Camera3DComponent, CameraXRComponent, DraggableComponent, DraggablePlane, RaycastableComponent,
    SelectableComponent, SerializeComponent, TransformComponent,
};
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::{ComponentId, SignalEmitter, World};
use crate::engine::ecs::{EventSignal, IntentValue, PointerActivationSource, RxWorld, SignalKind};

#[derive(Debug, Default)]
pub struct DraggableSystem {
    handlers_installed: bool,
}

impl DraggableSystem {
    pub fn install_handlers(&mut self, rx: &mut RxWorld) {
        if self.handlers_installed {
            return;
        }
        rx.add_global_handler_closure(SignalKind::DragMove, |world, emit, env| {
            let Some(EventSignal::DragMove {
                activation_source,
                raycaster,
                renderable,
                delta_world,
                screen_pos_px: _,
                ..
            }) = env.event.as_ref()
            else {
                return;
            };
            let supported_activation = *activation_source == PointerActivationSource::Trigger;
            if !supported_activation {
                return;
            }
            let Some(resolved) = resolve_draggable_for_hit(world, *renderable) else {
                return;
            };
            let owner = resolved.target;
            let local_delta = constrained_parent_local_delta(
                world,
                owner,
                resolved.plane,
                *delta_world,
                Some(*raycaster),
            );
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
        draggable: ComponentId,
        emit: &mut dyn SignalEmitter,
    ) {
        if world
            .get_component_by_id_as::<DraggableComponent>(draggable)
            .is_some_and(|component| !component.enabled)
        {
            return;
        }
        let Some(owner) = world.parent_of(draggable).filter(|id| {
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
            "draggable_generated_raycastable",
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

fn constrained_parent_local_delta(
    world: &World,
    target: ComponentId,
    plane: DraggablePlane,
    delta_world: [f32; 3],
    raycaster: Option<ComponentId>,
) -> [f32; 3] {
    match plane {
        DraggablePlane::Object => {
            let mut local = world_delta_to_parent_local(world, target, delta_world);
            local[2] = 0.0;
            local
        }
        DraggablePlane::Camera => {
            let constrained_world = raycaster
                .and_then(|raycaster| camera_plane_world_axes(world, raycaster))
                .map(|axes| project_onto_world_axes(delta_world, axes))
                .unwrap_or(delta_world);
            world_delta_to_parent_local(world, target, constrained_world)
        }
        DraggablePlane::WorldAxes(axes) => {
            let projected_world = project_onto_world_axes(delta_world, axes);
            world_delta_to_parent_local(world, target, projected_world)
        }
    }
}

fn camera_plane_world_axes(world: &World, raycaster: ComponentId) -> Option<[[f32; 3]; 2]> {
    let mut current = Some(raycaster);
    while let Some(id) = current {
        if world
            .get_component_by_id_as::<Camera3DComponent>(id)
            .is_some()
            || world
                .get_component_by_id_as::<CameraXRComponent>(id)
                .is_some()
        {
            return TransformSystem::world_model(world, id).map(world_xy_axes);
        }
        current = world.parent_of(id);
    }

    world
        .all_components()
        .find(|id| {
            world
                .get_component_by_id_as::<CameraXRComponent>(*id)
                .is_some_and(|camera| camera.enabled)
        })
        .and_then(|camera| TransformSystem::world_model(world, camera))
        .map(world_xy_axes)
}

fn world_xy_axes(model: [[f32; 4]; 4]) -> [[f32; 3]; 2] {
    [
        [model[0][0], model[0][1], model[0][2]],
        [model[1][0], model[1][1], model[1][2]],
    ]
}

fn project_onto_world_axes(delta: [f32; 3], axes: [[f32; 3]; 2]) -> [f32; 3] {
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let normalize = |v: [f32; 3]| {
        let length = dot(v, v).sqrt();
        [v[0] / length, v[1] / length, v[2] / length]
    };
    let first = normalize(axes[0]);
    let second_rejected = {
        let along_first = dot(axes[1], first);
        [
            axes[1][0] - first[0] * along_first,
            axes[1][1] - first[1] * along_first,
            axes[1][2] - first[2] * along_first,
        ]
    };
    let second = normalize(second_rejected);
    let first_amount = dot(delta, first);
    let second_amount = dot(delta, second);
    [
        first[0] * first_amount + second[0] * second_amount,
        first[1] * first_amount + second[1] * second_amount,
        first[2] * first_amount + second[2] * second_amount,
    ]
}

#[derive(Debug, Clone, Copy)]
struct ResolvedDraggable {
    target: ComponentId,
    plane: DraggablePlane,
}

/// Resolve a renderable hit to its draggable movement target.
///
/// An enabled Selectable sidecar on the same owner wins over Draggable. Handle-style
/// `Draggable.parent()` markers resolve to the owner's parent Transform.
pub fn draggable_owner_for_hit(world: &World, renderable: ComponentId) -> Option<ComponentId> {
    resolve_draggable_for_hit(world, renderable).map(|resolved| resolved.target)
}

fn resolve_draggable_for_hit(world: &World, renderable: ComponentId) -> Option<ResolvedDraggable> {
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
            if let Some(draggable) = world
                .children_of(id)
                .iter()
                .find_map(|child| world.get_component_by_id_as::<DraggableComponent>(*child))
            {
                if !draggable.enabled {
                    return None;
                }
                if !draggable.move_parent {
                    return Some(ResolvedDraggable {
                        target: id,
                        plane: draggable.plane,
                    });
                }
                let mut parent = world.parent_of(id);
                while let Some(candidate) = parent {
                    if world
                        .get_component_by_id_as::<TransformComponent>(candidate)
                        .is_some()
                    {
                        return Some(ResolvedDraggable {
                            target: candidate,
                            plane: draggable.plane,
                        });
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
    use crate::engine::ecs::component::{RenderableComponent, SelectableComponent};
    use crate::engine::ecs::{CommandQueue, Signal};

    #[test]
    fn registration_generates_runtime_raycastable_and_nested_hits_resolve_to_owner() {
        let mut world = World::default();
        let owner = world.add_component(TransformComponent::new());
        let draggable = world.add_component(DraggableComponent::new());
        let nested = world.add_component(TransformComponent::new());
        let deep = world.add_component(TransformComponent::new());
        let renderable = world.add_component(RenderableComponent::cube());
        world.add_child(owner, draggable).unwrap();
        world.add_child(owner, nested).unwrap();
        world.add_child(nested, deep).unwrap();
        world.add_child(deep, renderable).unwrap();

        let mut queue = CommandQueue::new();
        let mut system = DraggableSystem::default();
        system.register(&mut world, draggable, &mut queue);

        let generated = world
            .children_of(owner)
            .iter()
            .copied()
            .find(|child| world.component_label(*child) == Some("draggable_generated_raycastable"))
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
        assert_eq!(draggable_owner_for_hit(&world, renderable), Some(owner));
    }

    #[test]
    fn explicit_owner_raycastable_is_preserved() {
        let mut world = World::default();
        let owner = world.add_component(TransformComponent::new());
        let draggable = world.add_component(DraggableComponent::new());
        let explicit = world.add_component(RaycastableComponent::disabled());
        world.add_child(owner, draggable).unwrap();
        world.add_child(owner, explicit).unwrap();
        let mut queue = CommandQueue::new();
        DraggableSystem::default().register(&mut world, draggable, &mut queue);
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
    fn selectable_on_wins_over_draggable_but_selectable_off_does_not() {
        let mut world = World::default();
        let owner = world.add_component(TransformComponent::new());
        let draggable = world.add_component(DraggableComponent::new());
        let selectable = world.add_component(SelectableComponent::on());
        let renderable = world.add_component(RenderableComponent::cube());
        world.add_child(owner, draggable).unwrap();
        world.add_child(owner, selectable).unwrap();
        world.add_child(owner, renderable).unwrap();

        assert_eq!(draggable_owner_for_hit(&world, renderable), None);
        world
            .get_component_by_id_as_mut::<SelectableComponent>(selectable)
            .unwrap()
            .enabled = false;
        assert_eq!(draggable_owner_for_hit(&world, renderable), Some(owner));
    }

    #[test]
    fn parent_handle_resolves_to_the_complete_parent_transform() {
        let mut world = World::default();
        let panel = world.add_component(TransformComponent::new());
        let title_bar = world.add_component(TransformComponent::new());
        let draggable = world.add_component(DraggableComponent::parent());
        let renderable = world.add_component(RenderableComponent::cube());
        world.add_child(panel, title_bar).unwrap();
        world.add_child(title_bar, draggable).unwrap();
        world.add_child(title_bar, renderable).unwrap();

        assert_eq!(draggable_owner_for_hit(&world, renderable), Some(panel));
    }

    #[test]
    fn desktop_and_xr_trigger_drag_move_draggable() {
        let mut world = World::default();
        let owner = world.add_component(TransformComponent::new());
        let draggable = world.add_component(DraggableComponent::new());
        let renderable = world.add_component(RenderableComponent::cube());
        world.add_child(owner, draggable).unwrap();
        world.add_child(owner, renderable).unwrap();

        let mut rx = RxWorld::default();
        DraggableSystem::default().install_handlers(&mut rx);
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
                    && *translation == [0.25, -0.5, 0.0]
        )));

        rx.dispatch_event_handlers(&mut world, &drag(None));
        assert!(rx.drain_ready_intents().iter().any(|signal| matches!(
            signal.intent.as_ref().map(|intent| &intent.value),
            Some(IntentValue::UpdateTransform { component_ids, .. }) if component_ids.as_slice() == [owner]
        )));
    }

    #[test]
    fn camera_and_world_axis_planes_constrain_deltas() {
        let mut world = World::default();
        let owner = world.add_component(TransformComponent::new());

        assert_eq!(
            constrained_parent_local_delta(
                &world,
                owner,
                DraggablePlane::Camera,
                [1.0, 2.0, 3.0],
                None,
            ),
            [1.0, 2.0, 3.0]
        );
        let camera_rig = world.add_component(TransformComponent::new());
        let camera = world.add_component(Camera3DComponent::default());
        world.add_child(camera_rig, camera).unwrap();
        assert_eq!(
            constrained_parent_local_delta(
                &world,
                owner,
                DraggablePlane::Camera,
                [1.0, 2.0, 3.0],
                Some(camera),
            ),
            [1.0, 2.0, 0.0]
        );
        let projected = constrained_parent_local_delta(
            &world,
            owner,
            DraggablePlane::WorldAxes([[1.0, 0.0, 0.0], [0.0, 0.0, 2.0]]),
            [1.0, 2.0, 3.0],
            None,
        );
        assert!((projected[0] - 1.0).abs() < 1e-6);
        assert!(projected[1].abs() < 1e-6);
        assert!((projected[2] - 3.0).abs() < 1e-6);
    }
}
