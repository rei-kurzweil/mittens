use crate::engine::ecs::component::ToggleComponent;
use crate::engine::ecs::system::selection_system::{
    add_selection_highlight, remove_selection_highlight,
};
use crate::engine::ecs::{ComponentId, EventSignal, RxWorld, SignalEmitter, SignalKind, World};

#[derive(Debug, Default)]
pub struct ToggleSystem {
    handlers_installed: bool,
}

impl ToggleSystem {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn install_handlers(&mut self, rx: &mut RxWorld) {
        if self.handlers_installed {
            return;
        }
        self.handlers_installed = true;
        rx.add_global_handler_closure(SignalKind::Click, |world, emit, signal| {
            let Some(EventSignal::Click { renderable, .. }) = signal.event.as_ref() else {
                return;
            };
            let Some(toggle) = resolve_toggle_click(world, *renderable) else {
                return;
            };
            let value = !world
                .get_component_by_id_as::<ToggleComponent>(toggle)
                .is_some_and(ToggleComponent::value);
            apply_toggle_set(world, emit, toggle, value);
        });
    }
}

fn toggle_on_node(world: &World, node: ComponentId) -> Option<ComponentId> {
    if world
        .get_component_by_id_as::<ToggleComponent>(node)
        .is_some()
    {
        return Some(node);
    }
    world.children_of(node).iter().copied().find(|child| {
        world
            .get_component_by_id_as::<ToggleComponent>(*child)
            .is_some()
    })
}

fn resolve_toggle_click(world: &World, start: ComponentId) -> Option<ComponentId> {
    let mut current = Some(start);
    while let Some(node) = current {
        if let Some(toggle) = toggle_on_node(world, node) {
            return Some(toggle);
        }
        current = world.parent_of(node);
    }
    None
}

pub fn toggle_owner(world: &World, toggle: ComponentId) -> ComponentId {
    if world.children_of(toggle).is_empty() {
        world.parent_of(toggle).unwrap_or(toggle)
    } else {
        toggle
    }
}

pub fn apply_toggle_set(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    toggle: ComponentId,
    value: bool,
) {
    let Some(old) = world
        .get_component_by_id_as::<ToggleComponent>(toggle)
        .map(ToggleComponent::value)
    else {
        return;
    };
    if old != value {
        if let Some(component) = world.get_component_by_id_as_mut::<ToggleComponent>(toggle) {
            component.set_value(value);
        }
    }
    let owner = toggle_owner(world, toggle);
    if value {
        add_selection_highlight(world, emit, owner);
    } else {
        remove_selection_highlight(world, emit, owner);
    }
    if old != value {
        emit.push_event(toggle, EventSignal::ToggleChanged { toggle, value });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::{StyleComponent, TransformComponent};
    use crate::engine::ecs::{CommandQueue, IntentSignal};
    use crate::engine::graphics::{RenderAssets, VisualWorld};

    #[derive(Default)]
    struct RecordingEmitter {
        events: Vec<EventSignal>,
        intents: Vec<IntentSignal>,
    }

    impl SignalEmitter for RecordingEmitter {
        fn push_event(&mut self, _scope: ComponentId, event: EventSignal) {
            self.events.push(event);
        }

        fn push_intent(&mut self, _scope: ComponentId, intent: IntentSignal) {
            self.intents.push(intent);
        }
    }

    #[test]
    fn click_and_programmatic_updates_emit_changes_and_restore_highlight() {
        let mut world = World::default();
        let owner = world.add_component(TransformComponent::new());
        let mut style_component = StyleComponent::new();
        let original = [0.1, 0.2, 0.3, 1.0];
        style_component.background_color = Some(original);
        let style = world.add_component(style_component);
        let toggle = world.add_component(ToggleComponent::off());
        world.add_child(owner, style).unwrap();
        world.add_child(owner, toggle).unwrap();

        let mut systems = crate::engine::ecs::system::SystemWorld::default();
        systems.toggle.install_handlers(&mut systems.rx);
        let mut visuals = VisualWorld::default();
        let mut assets = RenderAssets::new();
        let mut queue = CommandQueue::new();
        let click = || EventSignal::Click {
            raycaster: owner,
            renderable: owner,
            hit_point: [0.0; 3],
            screen_pos_px: None,
        };

        systems.rx.push_event(owner, click());
        systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);
        assert!(world
            .get_component_by_id_as::<ToggleComponent>(toggle)
            .unwrap()
            .value());
        assert_ne!(
            world
                .get_component_by_id_as::<StyleComponent>(style)
                .unwrap()
                .background_color,
            Some(original)
        );

        systems.rx.push_event(owner, click());
        systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);
        assert!(!world
            .get_component_by_id_as::<ToggleComponent>(toggle)
            .unwrap()
            .value());
        assert_eq!(
            world
                .get_component_by_id_as::<StyleComponent>(style)
                .unwrap()
                .background_color,
            Some(original)
        );

        let mut recorded = RecordingEmitter::default();
        apply_toggle_set(&mut world, &mut recorded, toggle, true);
        apply_toggle_set(&mut world, &mut recorded, toggle, true);
        assert_eq!(recorded.events.len(), 1);
        assert!(matches!(
            recorded.events[0],
            EventSignal::ToggleChanged {
                toggle: changed,
                value: true,
            } if changed == toggle
        ));
    }
}
