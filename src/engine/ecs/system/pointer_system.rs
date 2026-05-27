use crate::engine::ecs::component::{PointerComponent, RayCastComponent};
use crate::engine::ecs::{ComponentId, SignalEmitter, World};
use std::collections::HashMap;

/// Runtime owner for pointer-specific state and lifecycle.
///
/// Initial responsibilities:
/// - ensure each `PointerComponent` owns a child `RayCastComponent`
/// - cache pointer ↔ raycaster relationships
///
/// Higher-level pointer behavior (topology classification, trigger policy, etc.) will migrate
/// here incrementally from other systems.
#[derive(Debug, Default)]
pub struct PointerSystem {
    pointer_to_raycast: HashMap<ComponentId, ComponentId>,
}

impl PointerSystem {
    pub fn register_pointer(
        &mut self,
        world: &mut World,
        component: ComponentId,
        emit: &mut dyn SignalEmitter,
    ) {
        let Some(pointer) = world.get_component_by_id_as::<PointerComponent>(component) else {
            return;
        };

        if !pointer.enabled {
            self.pointer_to_raycast.remove(&component);
            return;
        }

        let existing_raycast = world.children_of(component).iter().copied().find(|&child| {
            world
                .get_component_by_id_as::<RayCastComponent>(child)
                .is_some()
        });

        let raycast = match existing_raycast {
            Some(raycast) => raycast,
            None => {
                let raycast = world.add_component(RayCastComponent::event_driven());
                if world.add_child(component, raycast).is_err() {
                    return;
                }
                world.init_component_tree(raycast, emit);
                raycast
            }
        };

        self.pointer_to_raycast.insert(component, raycast);
    }

    pub fn remove_pointer(&mut self, component: ComponentId) {
        self.pointer_to_raycast.remove(&component);
    }

    pub fn raycast_for_pointer(&self, component: ComponentId) -> Option<ComponentId> {
        self.pointer_to_raycast.get(&component).copied()
    }
}