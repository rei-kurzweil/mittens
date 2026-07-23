use crate::engine::ecs::component::{
    AvatarControlComponent, BoneRestPoseComponent, ColorComponent, ComponentRef, ControllerHand,
    ControllerXRComponent, EmissiveComponent, GLTFComponent, InputComponent, InputXRComponent,
    OpacityComponent, PointerComponent, RayCastComponent, RenderableComponent, SerializeComponent,
    TransformComponent,
};
use crate::engine::ecs::system::XrInputState;
use crate::engine::ecs::{ComponentId, SignalEmitter, World};
use crate::engine::graphics::primitives::Transform;
use crate::engine::user_input::InputState;
use crate::utils::math::{
    mat4_identity, mat4_mul, shortest_arc_quat, vec3_add, vec3_len, vec3_normalize, vec3_sub,
};
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
    let avatar_finger = config.avatar_finger.clone();
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
    let legacy_driver = pointer
        .and_then(|p| world.parent_of(p))
        .and_then(|p| nearest_ancestor_transform(world, p))
        .or_else(|| {
            world.children_of(hand).iter().copied().find(|c| {
                world
                    .get_component_by_id_as::<TransformComponent>(*c)
                    .is_some()
            })
        });
    let Some(mut driver) = legacy_driver else {
        return;
    };
    if let Some(finger) = avatar_finger {
        match avatar_finger_mount(world, hand, pointer, &finger) {
            Ok(Some(mount)) => driver = mount,
            Ok(None) => return, // AVC/GLTF initialization is still in flight.
            Err(message) => {
                let warn = world
                    .get_component_by_id_as::<ControllerXRComponent>(hand)
                    .is_some_and(|component| !component.avatar_laser_warned);
                if warn {
                    eprintln!(
                        "[XRHand] avatar-finger laser for {hand:?} could not bind: {message}; falling back to controller-space laser"
                    );
                    if let Some(component) =
                        world.get_component_by_id_as_mut::<ControllerXRComponent>(hand)
                    {
                        component.avatar_laser_warned = true;
                    }
                }
            }
        }
    }
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

fn avatar_finger_mount(
    world: &mut World,
    hand: ComponentId,
    pointer: Option<ComponentId>,
    finger: &[ComponentRef; 3],
) -> Result<Option<ComponentId>, String> {
    let mut current = world.parent_of(hand);
    let avc_id = loop {
        let Some(id) = current else {
            return Err("XRHand is not owned by an AVC".into());
        };
        if world
            .get_component_by_id_as::<AvatarControlComponent>(id)
            .is_some()
        {
            break id;
        }
        current = world.parent_of(id);
    };
    let (model_root, target, hand_bone) = {
        let avc = world
            .get_component_by_id_as::<AvatarControlComponent>(avc_id)
            .ok_or("AVC disappeared")?;
        let side = world
            .get_component_by_id_as::<ControllerXRComponent>(hand)
            .ok_or("XRHand disappeared")?
            .hand;
        let target = match side {
            ControllerHand::Left => avc.left_hand_visual_target_id,
            ControllerHand::Right => avc.right_hand_visual_target_id,
        };
        let target = match target {
            Some(target) => target,
            None => return Ok(None),
        };
        let hand_bone = match side {
            ControllerHand::Left => avc.left_hand_bone_id,
            ControllerHand::Right => avc.right_hand_bone_id,
        }
        .ok_or("AVC hand bone is not initialized")?;
        (
            avc.model_root_id
                .ok_or("AVC model root is not initialized")?,
            target,
            hand_bone,
        )
    };
    if let Some(existing) = world
        .children_of(target)
        .iter()
        .copied()
        .find(|id| world.component_label(*id) == Some("xr_avatar_finger_laser_mount"))
    {
        return Ok(Some(existing));
    }
    let gltf_id = find_descendant_gltf(world, model_root).ok_or("avatar GLTF was not found")?;
    let gltf = world
        .get_component_by_id_as::<GLTFComponent>(gltf_id)
        .ok_or("avatar GLTF disappeared")?;
    if gltf.spawned_node_transforms.is_empty() {
        return Ok(None);
    }
    let resolve = |reference: &ComponentRef| -> Result<ComponentId, String> {
        let matches: Vec<_> = gltf
            .spawned_node_transforms
            .iter()
            .copied()
            .filter(|id| match reference {
                ComponentRef::Guid(guid) => world.component_id_by_guid(*guid) == Some(*id),
                ComponentRef::Query(query) => world.component_matches_selector(*id, query),
            })
            .collect();
        if matches.len() != 1 {
            return Err(format!(
                "finger selector {} matched {} avatar nodes (expected exactly one)",
                component_ref_surface(reference),
                matches.len()
            ));
        }
        Ok(matches[0])
    };
    let root = resolve(&finger[0])?;
    let middle = resolve(&finger[1])?;
    let tip = resolve(&finger[2])?;
    if !is_descendant(world, middle, root) || !is_descendant(world, tip, middle) {
        return Err("configured finger joints are not an ancestral root/middle/tip chain".into());
    }
    let middle_model = rest_model_relative(world, hand_bone, middle)
        .ok_or("middle finger joint is not beneath the AVC hand bone")?;
    let tip_model = rest_model_relative(world, hand_bone, tip)
        .ok_or("tip finger joint is not beneath the AVC hand bone")?;
    // Resolving the root is intentional even though the final segment alone
    // determines the ray: it validates the complete configured chain.
    rest_model_relative(world, hand_bone, root)
        .ok_or("root finger joint is not beneath the AVC hand bone")?;
    let middle_position = [middle_model[3][0], middle_model[3][1], middle_model[3][2]];
    let tip_position = [tip_model[3][0], tip_model[3][1], tip_model[3][2]];
    let final_segment = vec3_sub(tip_position, middle_position);
    if vec3_len(final_segment) <= 1e-6 {
        return Err("finger's final rest-space segment has zero length".into());
    }
    let direction = vec3_normalize(final_segment);
    let origin = vec3_add(tip_position, final_segment);
    let rotation = shortest_arc_quat([0.0, 0.0, -1.0], direction);
    let mount = world.add_component_boxed_named(
        "xr_avatar_finger_laser_mount",
        Box::new(
            TransformComponent::new()
                .with_position(origin[0], origin[1], origin[2])
                .with_rotation_quat(rotation),
        ),
    );
    let serialize = world.add_component(SerializeComponent::off());
    let _ = world.add_child(mount, serialize);
    world
        .add_child(target, mount)
        .map_err(|_| "could not mount laser beneath the AVC hand target")?;
    if let Some(pointer) = pointer {
        world
            .set_parent(pointer, Some(mount))
            .map_err(|_| "could not move pointer ray source to the fingertip mount")?;
    }
    Ok(Some(mount))
}

fn component_ref_surface(reference: &ComponentRef) -> String {
    match reference {
        ComponentRef::Guid(guid) => format!("@uuid:{guid}"),
        ComponentRef::Query(query) => query.clone(),
    }
}

fn find_descendant_gltf(world: &World, root: ComponentId) -> Option<ComponentId> {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if world.get_component_by_id_as::<GLTFComponent>(id).is_some() {
            return Some(id);
        }
        stack.extend_from_slice(world.children_of(id));
    }
    None
}

fn is_descendant(world: &World, mut id: ComponentId, ancestor: ComponentId) -> bool {
    for _ in 0..64 {
        if id == ancestor {
            return true;
        }
        let Some(parent) = world.parent_of(id) else {
            return false;
        };
        id = parent;
    }
    false
}

fn rest_model_relative(
    world: &World,
    ancestor: ComponentId,
    descendant: ComponentId,
) -> Option<[[f32; 4]; 4]> {
    let mut ids = Vec::new();
    let mut current = Some(descendant);
    while let Some(id) = current {
        if id == ancestor {
            ids.reverse();
            return Some(ids.into_iter().fold(mat4_identity(), |model, id| {
                let local = world
                    .children_of(id)
                    .iter()
                    .find_map(|child| world.get_component_by_id_as::<BoneRestPoseComponent>(*child))
                    .map(|rest| {
                        let mut transform = Transform::default();
                        transform.translation = rest.translation;
                        transform.rotation = rest.rotation;
                        transform.scale = rest.scale;
                        transform.recompute_model();
                        transform.model
                    })
                    .or_else(|| {
                        world
                            .get_component_by_id_as::<TransformComponent>(id)
                            .map(|transform| transform.transform.model)
                    })
                    .unwrap_or_else(mat4_identity);
                mat4_mul(model, local)
            }));
        }
        if world
            .get_component_by_id_as::<TransformComponent>(id)
            .is_some()
        {
            ids.push(id);
        }
        current = world.parent_of(id);
    }
    None
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

    fn assert_avatar_finger_mount(hand_side: ControllerHand) {
        let mut world = World::default();
        let avc = world.add_component(AvatarControlComponent::new());
        let model_root = world.add_component(TransformComponent::new());
        let gltf = world.add_component(GLTFComponent::new("finger-test.glb"));
        let hand_bone =
            world.add_component_boxed_named("hand_bone", Box::new(TransformComponent::new()));
        let root = world.add_component_boxed_named(
            "middle_root",
            Box::new(TransformComponent::new().with_position(0.1, 0.0, 0.0)),
        );
        let middle = world.add_component_boxed_named(
            "middle_joint",
            Box::new(TransformComponent::new().with_position(0.1, 0.0, 0.0)),
        );
        let tip = world.add_component_boxed_named(
            "middle_tip",
            Box::new(TransformComponent::new().with_position(0.1, 0.0, 0.0)),
        );
        world.add_child(avc, model_root).unwrap();
        world.add_child(model_root, gltf).unwrap();
        world.add_child(gltf, hand_bone).unwrap();
        world.add_child(hand_bone, root).unwrap();
        world.add_child(root, middle).unwrap();
        world.add_child(middle, tip).unwrap();
        for (bone, translation) in [
            (hand_bone, [0.0, 0.0, 0.0]),
            (root, [0.1, 0.0, 0.0]),
            (middle, [0.1, 0.0, 0.0]),
            (tip, [0.1, 0.0, 0.0]),
        ] {
            let rest = world.add_component(BoneRestPoseComponent::new(
                translation,
                [0.0, 0.0, 0.0, 1.0],
                [1.0; 3],
            ));
            world.add_child(bone, rest).unwrap();
        }
        {
            let component = world
                .get_component_by_id_as_mut::<GLTFComponent>(gltf)
                .unwrap();
            component.spawned_node_transforms = vec![hand_bone, root, middle, tip];
            component.armature_joint_transforms = vec![hand_bone, root, middle, tip];
        }

        let hand = world.add_component(
            ControllerXRComponent::new(true, hand_side, ControllerPoseKind::Grip)
                .laser_from_avatar_finger(
                    ComponentRef::Query("#middle_root".into()),
                    ComponentRef::Query("#middle_joint".into()),
                    ComponentRef::Query("#middle_tip".into()),
                ),
        );
        let driver = world.add_component(TransformComponent::new());
        let correction = world.add_component(
            TransformComponent::new().with_rotation_quat([0.0, 0.0, 0.70710677, 0.70710677]),
        );
        let pointer = world.add_component(PointerComponent::new());
        world.add_child(avc, hand).unwrap();
        world.add_child(hand, driver).unwrap();
        world.add_child(driver, correction).unwrap();
        world.add_child(driver, pointer).unwrap();
        {
            let component = world
                .get_component_by_id_as_mut::<AvatarControlComponent>(avc)
                .unwrap();
            component.model_root_id = Some(model_root);
            match hand_side {
                ControllerHand::Left => {
                    component.left_hand_bone_id = Some(hand_bone);
                    component.left_hand_visual_target_id = Some(correction);
                }
                ControllerHand::Right => {
                    component.right_hand_bone_id = Some(hand_bone);
                    component.right_hand_visual_target_id = Some(correction);
                }
            }
        }

        let mut queue = CommandQueue::new();
        ensure_xr_hand_laser(&mut world, hand, &mut queue);
        let mount = world
            .children_of(correction)
            .iter()
            .copied()
            .find(|id| world.component_label(*id) == Some("xr_avatar_finger_laser_mount"))
            .expect("avatar fingertip mount");
        assert_eq!(world.parent_of(pointer), Some(mount));
        let mount_transform = world
            .get_component_by_id_as::<TransformComponent>(mount)
            .unwrap();
        assert!((mount_transform.transform.translation[0] - 0.4).abs() < 1e-5);
        assert_eq!(
            world
                .get_component_by_id_as::<TransformComponent>(correction)
                .unwrap()
                .transform
                .rotation,
            [0.0, 0.0, 0.70710677, 0.70710677]
        );
        let laser = world
            .children_of(mount)
            .iter()
            .copied()
            .find(|id| world.component_label(*id) == Some("xr_pointer_laser"))
            .expect("laser visual beneath shared fingertip mount");
        assert_eq!(
            world
                .get_component_by_id_as::<TransformComponent>(laser)
                .unwrap()
                .transform
                .translation,
            [0.0; 3]
        );
    }

    #[test]
    fn avatar_finger_lasers_mount_both_hands_beneath_corrected_targets() {
        assert_avatar_finger_mount(ControllerHand::Left);
        assert_avatar_finger_mount(ControllerHand::Right);
    }
}
