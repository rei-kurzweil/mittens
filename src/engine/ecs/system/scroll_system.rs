use crate::engine::ecs::component::{RouterComponent, ScrollingComponent, TransformComponent};
use crate::engine::ecs::component::{RenderableComponent, StencilClipComponent};
use crate::engine::ecs::rx::RxWorld;
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, SignalEmitter, SignalKind, World};

#[derive(Debug, Default)]
pub struct ScrollingSystem;

impl ScrollingSystem {
    const OWNED_ROUTER_LABEL: &'static str = "__scroll_router";
    const OWNED_TRACK_LABEL: &'static str = "__scroll_track";

    pub fn new() -> Self {
        Self
    }

    fn immediate_owned_track(world: &World, scroll_component: ComponentId) -> Option<ComponentId> {
        world.children_of(scroll_component).iter().copied().find(|&child| {
            world.component_label(child) == Some(Self::OWNED_TRACK_LABEL)
                && world.get_component_by_id_as::<TransformComponent>(child).is_some()
        })
    }

    fn immediate_owned_router(world: &World, scroll_component: ComponentId) -> Option<ComponentId> {
        world.children_of(scroll_component).iter().copied().find(|&child| {
            world.component_label(child) == Some(Self::OWNED_ROUTER_LABEL)
                && world.get_component_by_id_as::<RouterComponent>(child).is_some()
        })
    }

    fn ensure_owned_router_and_track(
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        scroll_component: ComponentId,
    ) -> Option<ComponentId> {
        let track = if let Some(track) = Self::immediate_owned_track(world, scroll_component) {
            track
        } else {
            let track = world.add_component_boxed_named(
                Self::OWNED_TRACK_LABEL,
                Box::new(TransformComponent::new()),
            );
            let _ = world.add_child(scroll_component, track);
            world.init_component_tree(track, emit);
            track
        };

        if Self::immediate_owned_router(world, scroll_component).is_none() {
            let router = world.add_component_boxed_named(
                Self::OWNED_ROUTER_LABEL,
                Box::new(RouterComponent::new().with_target_name(Self::OWNED_TRACK_LABEL)),
            );
            let _ = world.add_child(scroll_component, router);
            world.init_component_tree(router, emit);
        }

        Some(track)
    }

    fn install_drag_forwarding(
        rx: &mut RxWorld,
        drag_scope: ComponentId,
        scroll_component: ComponentId,
    ) {
        rx.add_handler_closure(
            SignalKind::DragMove,
            drag_scope,
            move |world, emit, env| {
                let Some(EventSignal::DragMove { delta_world, .. }) = env.event.as_ref() else {
                    return;
                };

                let scroll_state = {
                    let Some(sc) = world.get_component_by_id_as_mut::<ScrollingComponent>(scroll_component) else {
                        return;
                    };
                    let prev_offset = sc.scroll_offset;
                    let changed = sc.apply_drag(-delta_world[1]);
                    println!(
                        "[Scrolling] DragMove scroll={:?} track={:?} scope={:?} delta_world=({:.3},{:.3},{:.3}) offset={:.3}->{:.3} changed={} max={:.3}",
                        scroll_component,
                        sc.track,
                        sc.drag_scope,
                        delta_world[0],
                        delta_world[1],
                        delta_world[2],
                        prev_offset,
                        sc.scroll_offset,
                        changed,
                        sc.max_scroll(),
                    );
                    if !changed {
                        return;
                    }
                    (
                        sc.scroll_offset,
                        sc.max_scroll(),
                        sc.viewport_height,
                        sc.content_height,
                    )
                };

                Self::sync_component(world, emit, scroll_component);
                emit.push_event(
                    scroll_component,
                    EventSignal::Scrolling {
                        scroll_component,
                        drag_scope,
                        delta_world: *delta_world,
                        scroll_offset: scroll_state.0,
                        max_scroll: scroll_state.1,
                        viewport_height: scroll_state.2,
                        content_height: scroll_state.3,
                    },
                );
            },
        );
    }

    pub fn deferred_register(
        &mut self,
        rx: &mut RxWorld,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        scroll_component: ComponentId,
    ) {
        let existing_track = world
            .get_component_by_id_as::<ScrollingComponent>(scroll_component)
            .and_then(|sc| sc.track);
        let track = existing_track
            .or_else(|| Self::ensure_owned_router_and_track(world, emit, scroll_component))
            .or_else(|| Self::nearest_ancestor_transform(world, scroll_component));
        let drag_scope = Self::nearest_drag_scope(world, scroll_component);

        println!(
            "[Scrolling] register scroll={:?} label={:?} track={:?} drag_scope={:?}",
            scroll_component,
            world.component_label(scroll_component),
            track,
            drag_scope,
        );

        if let Some(track_id) = track {
            let base_pos = world
                .get_component_by_id_as::<TransformComponent>(track_id)
                .map(|tc| tc.transform.translation)
                .unwrap_or([0.0, 0.0, 0.0]);
            if let Some(sc) = world.get_component_by_id_as_mut::<ScrollingComponent>(scroll_component) {
                if sc.track.is_none() {
                    sc.set_track(track_id, base_pos);
                }
            }
            Self::sync_component(world, emit, scroll_component);
        }

        if let Some(scope) = drag_scope {
            let should_install = world
                .get_component_by_id_as::<ScrollingComponent>(scroll_component)
                .map(|sc| sc.drag_scope != Some(scope))
                .unwrap_or(false);

            if should_install {
                if let Some(sc) = world.get_component_by_id_as_mut::<ScrollingComponent>(scroll_component) {
                    sc.set_drag_scope(scope);
                }
                Self::install_drag_forwarding(rx, scope, scroll_component);
            }
        }
    }

    fn nearest_ancestor_transform(world: &World, start: ComponentId) -> Option<ComponentId> {
        let mut cursor = world.parent_of(start);
        while let Some(node) = cursor {
            if world.get_component_by_id_as::<TransformComponent>(node).is_some() {
                return Some(node);
            }
            cursor = world.parent_of(node);
        }
        None
    }

    fn nearest_drag_scope(world: &World, start: ComponentId) -> Option<ComponentId> {
        let mut cursor = world.parent_of(start);
        while let Some(node) = cursor {
            if world.get_component_by_id_as::<StencilClipComponent>(node).is_some() {
                return Some(Self::stencil_drag_scope_root(world, node).unwrap_or(node));
            }
            if world.get_component_by_id_as::<RenderableComponent>(node).is_some() {
                return Some(node);
            }
            cursor = world.parent_of(node);
        }

        Self::nearest_ancestor_transform(world, start)
    }

    fn stencil_drag_scope_root(world: &World, stencil_clip: ComponentId) -> Option<ComponentId> {
        let parent = world.parent_of(stencil_clip)?;
        if world.component_label(parent) == Some("__bg") {
            return world.parent_of(parent);
        }
        Some(parent)
    }

    pub fn set_content_height(
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        scroll_component: ComponentId,
        content_height: f32,
    ) {
        {
            let Some(sc) = world.get_component_by_id_as_mut::<ScrollingComponent>(scroll_component) else {
                return;
            };
            let _ = sc.set_content_height(content_height);
        }

        Self::sync_component(world, emit, scroll_component);
    }

    pub fn sync_component(
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        scroll_component: ComponentId,
    ) {
        let (track_id, translation, rotation, scale) = {
            let Some(sc) = world.get_component_by_id_as::<ScrollingComponent>(scroll_component) else {
                return;
            };
            let Some(track_id) = sc.track else {
                return;
            };
            let translation = sc.track_translation();
            let Some(track_tc) = world.get_component_by_id_as::<TransformComponent>(track_id) else {
                return;
            };
            (
                track_id,
                translation,
                track_tc.transform.rotation,
                track_tc.transform.scale,
            )
        };

        emit.push_intent_now(
            track_id,
            IntentValue::UpdateTransform {
                component_ids: vec![track_id],
                translation,
                rotation_quat_xyzw: rotation,
                scale,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::ScrollingSystem;
    use crate::engine::ecs::CommandQueue;
    use crate::engine::ecs::IntentValue;
    use crate::engine::ecs::SignalEmitter;
    use crate::engine::ecs::SystemWorld;
    use crate::engine::ecs::World;
    use crate::engine::ecs::component::{ScrollingComponent, TransformComponent};
    use crate::engine::graphics::VisualWorld;

    #[test]
    fn scrolling_without_explicit_track_gets_owned_scroll_track() {
        let mut world = World::default();
        let mut visuals = VisualWorld::new();
        let mut queue = CommandQueue::new();
        let mut systems = SystemWorld::default();

        let scrolling = world.add_component(ScrollingComponent::new(1.0, 10.0));
        let item = world.add_component(TransformComponent::new().with_position(0.0, 2.0, 0.0));
        let _ = world.add_child(scrolling, item);

        world.init_component_tree(scrolling, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &mut queue);

        let track = world
            .get_component_by_id_as::<ScrollingComponent>(scrolling)
            .and_then(|sc| sc.track)
            .expect("owned track");

        assert_eq!(world.component_label(track), Some("__scroll_track"));
        assert_eq!(world.parent_of(track), Some(scrolling));
        assert_eq!(world.parent_of(item), Some(track));
    }

    #[test]
    fn explicit_scroll_track_is_preserved() {
        let mut world = World::default();
        let mut visuals = VisualWorld::new();
        let mut queue = CommandQueue::new();
        let mut systems = SystemWorld::default();

        let scrolling = world.add_component(ScrollingComponent::new(1.0, 10.0));
        let explicit_track = world.add_component(TransformComponent::new().with_position(3.0, 4.0, 5.0));
        let child = world.add_component(TransformComponent::new());
        let _ = world.add_child(scrolling, explicit_track);
        let _ = world.add_child(explicit_track, child);

        if let Some(sc) = world.get_component_by_id_as_mut::<ScrollingComponent>(scrolling) {
            sc.set_track(explicit_track, [3.0, 4.0, 5.0]);
        }

        world.init_component_tree(scrolling, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &mut queue);

        let sc = world
            .get_component_by_id_as::<ScrollingComponent>(scrolling)
            .expect("scrolling state");
        assert_eq!(sc.track, Some(explicit_track));
        assert!(ScrollingSystem::immediate_owned_track(&world, scrolling).is_none());
        assert_eq!(world.parent_of(child), Some(explicit_track));
    }

    #[test]
    fn scrolling_late_attached_children_route_into_owned_track() {
        let mut world = World::default();
        let mut visuals = VisualWorld::new();
        let mut queue = CommandQueue::new();
        let mut systems = SystemWorld::default();

        let scrolling = world.add_component(ScrollingComponent::new(1.0, 10.0));
        world.init_component_tree(scrolling, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &mut queue);

        let track = world
            .get_component_by_id_as::<ScrollingComponent>(scrolling)
            .and_then(|sc| sc.track)
            .expect("owned track");

        let late = world.add_component(TransformComponent::new().with_position(0.0, 3.0, 0.0));
        queue.push_intent_now(
            late,
            IntentValue::Attach {
                parents: vec![scrolling],
                child: late,
            },
        );
        systems.process_commands(&mut world, &mut visuals, &mut queue);

        assert_eq!(world.parent_of(late), Some(track));
    }
}