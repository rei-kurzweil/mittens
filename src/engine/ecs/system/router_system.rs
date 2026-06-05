use crate::engine::ecs::component::{RouterComponent, TransformComponent};
use crate::engine::ecs::rx::RxWorld;
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, SignalEmitter, SignalKind, World};
use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct RouterSystem {
    observed_owners: HashSet<ComponentId>,
}

#[derive(Debug)]
struct ResolvedRouter {
    router_component: ComponentId,
    target_component: ComponentId,
    ignored_components: HashSet<ComponentId>,
}

impl RouterSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_router(
        &mut self,
        rx: &mut RxWorld,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        router_component: ComponentId,
    ) {
        let Some(owner) = world.parent_of(router_component) else {
            return;
        };

        if Self::immediate_router_child(world, owner) != Some(router_component) {
            return;
        }

        if self.observed_owners.insert(owner) {
            rx.add_handler_closure(SignalKind::ParentChanged, owner, move |world, emit, env| {
                let Some(EventSignal::ParentChanged {
                    child, new_parent, ..
                }) = env.event.as_ref()
                else {
                    return;
                };

                if *new_parent != Some(owner) {
                    return;
                }

                Self::route_external_child(world, emit, owner, *child);
            });
        }

        Self::reroute_owner_children(world, emit, owner);
    }

    fn immediate_router_child(world: &World, owner: ComponentId) -> Option<ComponentId> {
        world.children_of(owner).iter().copied().find(|&child| {
            world
                .get_component_by_id_as::<RouterComponent>(child)
                .is_some()
        })
    }

    fn reroute_owner_children(world: &mut World, emit: &mut dyn SignalEmitter, owner: ComponentId) {
        let children: Vec<ComponentId> = world.children_of(owner).to_vec();
        for child in children {
            Self::route_external_child(world, emit, owner, child);
        }
    }

    fn route_external_child(
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        owner: ComponentId,
        child: ComponentId,
    ) {
        let Some(router) = Self::resolve_router(world, owner) else {
            return;
        };

        if !Self::is_external_direct_child(world, owner, child, &router) {
            return;
        }

        if Self::subtree_contains(world, child, router.target_component) {
            println!(
                "[RouterSystem] refusing to route child={:?} into descendant target={:?}",
                child, router.target_component
            );
            return;
        }

        emit.push_intent_now(
            child,
            IntentValue::Attach {
                parents: vec![router.target_component],
                child,
            },
        );
    }

    fn resolve_router(world: &World, owner: ComponentId) -> Option<ResolvedRouter> {
        let router_component = Self::immediate_router_child(world, owner)?;
        let router = world.get_component_by_id_as::<RouterComponent>(router_component)?;
        let target_name = router.target_name.as_deref()?;
        let target_component = Self::find_first_named_in_subtree(world, owner, target_name)?;

        let mut ignored_components = HashSet::new();
        for ignore_name in &router.ignore_names {
            Self::collect_named_in_subtree(world, owner, ignore_name, &mut ignored_components);
        }

        Some(ResolvedRouter {
            router_component,
            target_component,
            ignored_components,
        })
    }

    fn is_external_direct_child(
        world: &World,
        owner: ComponentId,
        child: ComponentId,
        router: &ResolvedRouter,
    ) -> bool {
        if world.parent_of(child) != Some(owner) {
            return false;
        }
        if child == router.router_component || child == router.target_component {
            return false;
        }
        if router.ignored_components.contains(&child) {
            return false;
        }
        if world
            .get_component_by_id_as::<TransformComponent>(child)
            .is_none()
        {
            return false;
        }
        if world
            .component_label(child)
            .map(|label| label.starts_with("__"))
            .unwrap_or(false)
        {
            return false;
        }
        true
    }

    fn find_first_named_in_subtree(
        world: &World,
        root: ComponentId,
        wanted_name: &str,
    ) -> Option<ComponentId> {
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            if world.component_label(node) == Some(wanted_name) {
                return Some(node);
            }
            for &child in world.children_of(node).iter().rev() {
                stack.push(child);
            }
        }
        None
    }

    fn collect_named_in_subtree(
        world: &World,
        root: ComponentId,
        wanted_name: &str,
        out: &mut HashSet<ComponentId>,
    ) {
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            if world.component_label(node) == Some(wanted_name) {
                out.insert(node);
            }
            for &child in world.children_of(node).iter().rev() {
                stack.push(child);
            }
        }
    }

    fn subtree_contains(world: &World, root: ComponentId, wanted: ComponentId) -> bool {
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            if node == wanted {
                return true;
            }
            for &child in world.children_of(node).iter().rev() {
                stack.push(child);
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::ecs::component::{RouterComponent, StyleComponent, TransformComponent};
    use crate::engine::ecs::{CommandQueue, IntentValue, SignalEmitter, SystemWorld, World};
    use crate::engine::graphics::{RenderAssets, VisualWorld};

    #[test]
    fn router_reroutes_initial_direct_children_to_target() {
        let mut world = World::default();
        let mut visuals = VisualWorld::new();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();

        let owner = world.add_component_boxed_named("owner", Box::new(TransformComponent::new()));
        let router = world.add_component_boxed_named(
            "router",
            Box::new(
                RouterComponent::new()
                    .with_target_name("container")
                    .with_ignored_names(["toolbar"]),
            ),
        );
        let toolbar =
            world.add_component_boxed_named("toolbar", Box::new(TransformComponent::new()));
        let container =
            world.add_component_boxed_named("container", Box::new(TransformComponent::new()));
        let authored =
            world.add_component_boxed_named("authored", Box::new(TransformComponent::new()));

        let _ = world.add_child(owner, router);
        let _ = world.add_child(owner, toolbar);
        let _ = world.add_child(owner, container);
        let _ = world.add_child(owner, authored);

        world.init_component_tree(owner, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        assert_eq!(world.parent_of(authored), Some(container));
        assert_eq!(world.parent_of(toolbar), Some(owner));
    }

    #[test]
    fn router_reroutes_late_attached_children_to_target() {
        let mut world = World::default();
        let mut visuals = VisualWorld::new();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();

        let owner = world.add_component_boxed_named("owner", Box::new(TransformComponent::new()));
        let router = world.add_component_boxed_named(
            "router",
            Box::new(RouterComponent::new().with_target_name("container")),
        );
        let container =
            world.add_component_boxed_named("container", Box::new(TransformComponent::new()));

        let _ = world.add_child(owner, router);
        let _ = world.add_child(owner, container);

        world.init_component_tree(owner, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        let late = world.add_component_boxed_named("late", Box::new(TransformComponent::new()));
        queue.push_intent_now(
            late,
            IntentValue::Attach {
                parents: vec![owner],
                child: late,
            },
        );

        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        assert_eq!(world.parent_of(late), Some(container));
    }

    #[test]
    fn router_does_not_reroute_non_transform_children() {
        let mut world = World::default();
        let mut visuals = VisualWorld::new();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();

        let owner = world.add_component_boxed_named("owner", Box::new(TransformComponent::new()));
        let router = world.add_component_boxed_named(
            "router",
            Box::new(RouterComponent::new().with_target_name("container")),
        );
        let container =
            world.add_component_boxed_named("container", Box::new(TransformComponent::new()));
        let style = world.add_component(StyleComponent::new());
        let authored =
            world.add_component_boxed_named("authored", Box::new(TransformComponent::new()));

        let _ = world.add_child(owner, router);
        let _ = world.add_child(owner, container);
        let _ = world.add_child(owner, style);
        let _ = world.add_child(owner, authored);

        world.init_component_tree(owner, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        assert_eq!(world.parent_of(authored), Some(container));
        assert_eq!(world.parent_of(style), Some(owner));
    }
}
