use crate::engine::ecs::component::{
    ColorComponent, ControllerXRComponent, EmissiveComponent, InputComponent, InputXRComponent,
    OpacityComponent, PointerComponent, RayCastComponent, RenderableComponent, SerializeComponent,
    TransformComponent,
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

/// Spawn the optional runtime-only controller laser under the transform driving its pointer.
pub fn ensure_xr_hand_laser(world: &mut World, hand: ComponentId, emit: &mut dyn SignalEmitter) {
    let Some(config) = world.get_component_by_id_as::<ControllerXRComponent>(hand) else {
        return;
    };
    if !config.enabled || !config.laser {
        return;
    }
    let mut stack = world.children_of(hand).to_vec();
    let mut pointer = None;
    while let Some(node) = stack.pop() {
        if world
            .get_component_by_id_as::<PointerComponent>(node)
            .is_some()
        {
            pointer = Some(node);
            break;
        }
        stack.extend_from_slice(world.children_of(node));
    }
    let driver = pointer
        .and_then(|p| world.parent_of(p))
        .and_then(|p| nearest_ancestor_transform(world, p))
        .or_else(|| {
            world.children_of(hand).iter().copied().find(|c| {
                world
                    .get_component_by_id_as::<TransformComponent>(*c)
                    .is_some()
            })
        });
    let Some(driver) = driver else {
        return;
    };
    if world
        .children_of(driver)
        .iter()
        .any(|c| world.component_label(*c) == Some("xr_pointer_laser"))
    {
        return;
    }

    // Keep the laser's authored origin exactly on the pointer-driving transform.
    // The centered cube mesh is offset separately so its near face starts at the
    // origin and extends ten metres along the pointer ray's local -Z.
    let laser =
        world.add_component_boxed_named("xr_pointer_laser", Box::new(TransformComponent::new()));
    let serialize = world.add_component(SerializeComponent::off());
    let mesh_transform = world.add_component_boxed_named(
        "xr_pointer_laser_mesh",
        Box::new(
            TransformComponent::new()
                .with_position(0.0, 0.0, -5.0)
                .with_scale(0.002, 0.002, 5.0),
        ),
    );
    let renderable = world.add_component(RenderableComponent::cube());
    let color = world.add_component(ColorComponent::rgba(0.0, 1.0, 1.0, 0.55));
    let opacity = world.add_component(OpacityComponent::new().with_opacity(0.55));
    let emissive = world.add_component(EmissiveComponent::on());
    let _ = world.add_child(laser, serialize);
    let _ = world.add_child(laser, mesh_transform);
    let _ = world.add_child(mesh_transform, renderable);
    let _ = world.add_child(renderable, color);
    let _ = world.add_child(renderable, opacity);
    let _ = world.add_child(renderable, emissive);
    if world.add_child(driver, laser).is_ok() {
        world.init_component_tree(laser, emit);
    }
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

/// Which `PointerComponent` ids have active trigger or grip state this frame.
///
/// Built by `PointerSystem::build_activations` from `InputState` (mouse) and `XrInputState`
/// (XR controllers). `GestureSystem` consumes this without knowing the underlying input source.
#[derive(Default, Debug, Clone)]
pub struct PointerActivations {
    pub pressed: Vec<ComponentId>,
    pub down: Vec<ComponentId>,
    pub released: Vec<ComponentId>,
    pub grip_pressed: Vec<ComponentId>,
    pub grip_down: Vec<ComponentId>,
    pub grip_released: Vec<ComponentId>,
}

impl PointerActivations {
    pub(crate) fn raycast_active(&self, pointer: ComponentId) -> bool {
        self.down.contains(&pointer)
            || self.pressed.contains(&pointer)
            || self.grip_down.contains(&pointer)
            || self.grip_pressed.contains(&pointer)
    }
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
                if xr.grip_pressed[i] {
                    act.grip_pressed.push(pointer_cid);
                }
                if xr.grip_down[i] {
                    act.grip_down.push(pointer_cid);
                }
                if xr.grip_released[i] {
                    act.grip_released.push(pointer_cid);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::CommandQueue;
    use crate::engine::ecs::component::{ControllerHand, ControllerPoseKind};

    #[test]
    fn controller_grip_edges_are_exposed_separately_from_trigger_edges() {
        let mut world = World::default();
        let controller = world.add_component(ControllerXRComponent::new(
            true,
            ControllerHand::Right,
            ControllerPoseKind::Grip,
        ));
        let pointer = world.add_component(PointerComponent::default());
        let raycast = world.add_component(RayCastComponent::event_driven());
        world.add_child(controller, pointer).unwrap();
        let mut system = PointerSystem::default();
        system.pointer_to_raycast.insert(pointer, raycast);
        let xr = XrInputState {
            grip_pressed: [false, true],
            grip_down: [false, true],
            grip_released: [false, true],
            ..Default::default()
        };
        let activations = system.build_activations(&world, &InputState::default(), &xr);
        assert!(activations.pressed.is_empty());
        assert_eq!(activations.grip_pressed, vec![pointer]);
        assert_eq!(activations.grip_down, vec![pointer]);
        assert_eq!(activations.grip_released, vec![pointer]);
        assert!(activations.raycast_active(pointer));
    }

    #[test]
    fn xr_laser_is_single_runtime_noninteractive_visual_aligned_to_negative_z() {
        let mut world = World::default();
        let hand = world.add_component(
            ControllerXRComponent::new(true, ControllerHand::Left, ControllerPoseKind::Aim).laser(),
        );
        let driver = world.add_component(TransformComponent::new());
        let pointer = world.add_component(PointerComponent::new());
        world.add_child(hand, driver).unwrap();
        world.add_child(driver, pointer).unwrap();
        let mut queue = CommandQueue::new();
        ensure_xr_hand_laser(&mut world, hand, &mut queue);
        ensure_xr_hand_laser(&mut world, hand, &mut queue);
        let lasers: Vec<_> = world
            .children_of(driver)
            .iter()
            .copied()
            .filter(|c| world.component_label(*c) == Some("xr_pointer_laser"))
            .collect();
        assert_eq!(lasers.len(), 1);
        let laser = lasers[0];
        let transform = world
            .get_component_by_id_as::<TransformComponent>(laser)
            .unwrap();
        assert_eq!(transform.transform.translation, [0.0, 0.0, 0.0]);
        assert_eq!(transform.transform.scale, [1.0, 1.0, 1.0]);
        assert!(world.children_of(laser).iter().any(|c| {
            world
                .get_component_by_id_as::<SerializeComponent>(*c)
                .is_some_and(|s| !s.enabled)
        }));
        assert!(!world.children_of(laser).iter().any(|c| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::RaycastableComponent>(*c)
                .is_some()
        }));
        let mesh_transform = world
            .children_of(laser)
            .iter()
            .copied()
            .find(|c| world.component_label(*c) == Some("xr_pointer_laser_mesh"))
            .unwrap();
        let mesh = world
            .get_component_by_id_as::<TransformComponent>(mesh_transform)
            .unwrap();
        assert_eq!(mesh.transform.translation, [0.0, 0.0, -5.0]);
        assert_eq!(mesh.transform.scale, [0.002, 0.002, 5.0]);
        let renderable = world
            .children_of(mesh_transform)
            .iter()
            .copied()
            .find(|c| {
                world
                    .get_component_by_id_as::<RenderableComponent>(*c)
                    .is_some()
            })
            .unwrap();
        assert!(world.children_of(renderable).iter().any(|c| {
            world
                .get_component_by_id_as::<EmissiveComponent>(*c)
                .is_some_and(|e| e.intensity > 0.0)
        }));
        assert!(world.children_of(renderable).iter().any(|c| {
            world
                .get_component_by_id_as::<OpacityComponent>(*c)
                .is_some_and(|o| o.opacity < 1.0)
        }));
    }
}
