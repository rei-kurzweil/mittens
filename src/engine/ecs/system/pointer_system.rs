use crate::engine::ecs::component::{
    ControllerXRComponent, InputComponent, InputXRComponent, PointerComponent, RayCastComponent,
};
use crate::engine::ecs::system::XrInputState;
use crate::engine::ecs::{ComponentId, SignalEmitter, World};
use crate::engine::user_input::InputState;
use std::collections::HashMap;
use winit::event::MouseButton;

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

    pub fn raycast_to_pointer(&self, raycaster: ComponentId) -> Option<ComponentId> {
        self.pointer_to_raycast
            .iter()
            .find(|(_, rc)| **rc == raycaster)
            .map(|(&ptr, _)| ptr)
    }
}

/// Lineage classification for a pointer / raycaster node.
///
/// Tells callers what kind of ancestor drives pose and what trigger source applies.
#[derive(Debug, Clone, Copy, Default)]
pub struct PointerTopologyContext {
    pub has_desktop_input_driver: bool,
    pub has_xr_input_driver: bool,
    pub has_controller_driver: bool,
    pub has_desktop_camera_anchor: bool,
    pub has_xr_camera_anchor: bool,
}

/// Walk ancestors looking for a component of type `T`.
pub fn has_ancestor_component<T: 'static>(world: &World, start: ComponentId) -> bool {
    let mut cur = start;
    while let Some(parent) = world.parent_of(cur) {
        if world.get_component_by_id_as::<T>(parent).is_some() {
            return true;
        }
        cur = parent;
    }
    false
}

/// Find the nearest ancestor (or self) that is a `TransformComponent`.
pub fn nearest_ancestor_transform(world: &World, start: ComponentId) -> Option<ComponentId> {
    if world
        .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(start)
        .is_some()
    {
        return Some(start);
    }
    let mut cur = start;
    while let Some(parent) = world.parent_of(cur) {
        if world
            .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(parent)
            .is_some()
        {
            return Some(parent);
        }
        cur = parent;
    }
    None
}

/// Classify a pointer node's lineage.
///
/// Camera anchor flags are set when a Camera3D/2D (desktop) or CameraXR is an **ancestor** of
/// the pointer — i.e. `Pointer` is a child/descendant of the camera component.
pub fn pointer_topology_context(world: &World, cid: ComponentId) -> PointerTopologyContext {
    PointerTopologyContext {
        has_desktop_input_driver: has_ancestor_component::<InputComponent>(world, cid),
        has_xr_input_driver: has_ancestor_component::<InputXRComponent>(world, cid),
        has_controller_driver: has_ancestor_component::<ControllerXRComponent>(world, cid),
        has_desktop_camera_anchor: has_ancestor_component::<
            crate::engine::ecs::component::Camera3DComponent,
        >(world, cid)
            || has_ancestor_component::<crate::engine::ecs::component::Camera2DComponent>(
                world, cid,
            ),
        has_xr_camera_anchor: has_ancestor_component::<
            crate::engine::ecs::component::CameraXRComponent,
        >(world, cid),
    }
}

/// Which `PointerComponent` ids have an active trigger state this frame.
///
/// Built by `PointerSystem::build_activations` from `InputState` (mouse) and `XrInputState`
/// (XR controllers). `GestureSystem` consumes this without knowing the underlying input source.
#[derive(Default, Debug, Clone)]
pub struct PointerActivations {
    pub pressed: Vec<ComponentId>,
    pub down: Vec<ComponentId>,
    pub released: Vec<ComponentId>,
}

impl PointerSystem {
    /// Map each registered pointer to its trigger state this frame.
    ///
    /// Desktop pointers use `InputState` mouse left button. Controller-backed pointers use
    /// `XrInputState` indexed by hand. A gaze-style pointer beneath `CameraXR` has no implicit
    /// trigger source and must not consume desktop mouse input merely because it lacks a
    /// `ControllerXR` ancestor.
    pub fn build_activations(
        &self,
        world: &World,
        input: &InputState,
        xr: &XrInputState,
    ) -> PointerActivations {
        let mut act = PointerActivations::default();

        for (&pointer_cid, _) in &self.pointer_to_raycast {
            let topo = pointer_topology_context(world, pointer_cid);

            if topo.has_controller_driver {
                // Resolve which hand owns this pointer's controller ancestor.
                let hand_idx = controller_hand_index(world, pointer_cid);
                let Some(i) = hand_idx else { continue };
                if xr.trigger_pressed[i] {
                    act.pressed.push(pointer_cid);
                }
                if xr.trigger_down[i] {
                    act.down.push(pointer_cid);
                }
                if xr.trigger_released[i] {
                    act.released.push(pointer_cid);
                }
            } else if !topo.has_xr_camera_anchor && !topo.has_xr_input_driver {
                // Desktop or otherwise non-XR pointer: left mouse button.
                if input.mouse_pressed.contains(&MouseButton::Left) {
                    act.pressed.push(pointer_cid);
                }
                if input.mouse_down.contains(&MouseButton::Left) {
                    act.down.push(pointer_cid);
                }
                if input.mouse_released.contains(&MouseButton::Left) {
                    act.released.push(pointer_cid);
                }
            }
        }

        act
    }
}

/// Find the nearest ancestor `ControllerXRComponent` and return its hand index (0=left, 1=right).
fn controller_hand_index(world: &World, start: ComponentId) -> Option<usize> {
    let mut cur = start;
    loop {
        if let Some(c) = world.get_component_by_id_as::<ControllerXRComponent>(cur) {
            return Some(match c.hand {
                crate::engine::ecs::component::ControllerHand::Left => 0,
                crate::engine::ecs::component::ControllerHand::Right => 1,
            });
        }
        cur = world.parent_of(cur)?;
    }
}
