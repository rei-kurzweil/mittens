//! Component serialization codec for saving/loading component trees to/from JSON.

use crate::engine::ecs::{ComponentId, World};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Intermediate representation of a component and its subtree.
///
/// On decode, all components get fresh IDs from the World.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentDataNode {
    /// Component GUID (globally unique identifier).
    pub guid: uuid::Uuid,

    /// Component name (may have _N suffix if duplicate at same level).
    pub name: String,

    /// Component type name (e.g., "transform", "renderable").
    pub type_name: String,

    /// Component-specific data as key-value pairs.
    pub data: HashMap<String, serde_json::Value>,

    /// Child components (preserves hierarchy).
    pub components: Vec<ComponentDataNode>,
}

/// A scene containing multiple root component trees.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    /// All root component trees in the scene.
    pub components: Vec<ComponentDataNode>,
}

/// Codec for encoding/decoding component trees to/from JSON files.
pub struct ComponentCodec;

impl ComponentCodec {
    /// Encode a component subtree rooted at `root_id` into a `ComponentDataNode`.
    ///
    /// This is the same encoding used for file-based save operations, but returns the
    /// in-memory representation instead of writing to disk.
    pub fn encode_subtree_node(
        world: &World,
        root_id: ComponentId,
    ) -> Result<ComponentDataNode, String> {
        Self::encode_subtree(world, root_id)
    }

    /// Decode a component subtree from an in-memory `ComponentDataNode`, but generate fresh GUIDs.
    ///
    /// This is useful for prefab-style instancing / cloning, where GUID collisions would be
    /// incorrect and `World::add_component_boxed_with_guid_named` would panic.
    ///
    /// Returns the newly-created root `ComponentId`.
    pub fn decode_subtree_node_with_new_guids(
        world: &mut World,
        parent_id: Option<ComponentId>,
        node: &ComponentDataNode,
    ) -> Result<ComponentId, String> {
        fn decode_subtree_fresh_guids(
            world: &mut World,
            parent_id: Option<ComponentId>,
            node: &ComponentDataNode,
        ) -> Result<ComponentId, String> {
            let mut component = ComponentCodec::create_component(&node.type_name)?;
            component.decode(&node.data)?;

            // Add to world with a fresh GUID but preserve the stored name.
            let new_id = world.add_component_boxed_named(node.name.clone(), component);

            if let Some(parent) = parent_id {
                world
                    .set_parent(new_id, Some(parent))
                    .map_err(|e| format!("Failed to set parent: {}", e))?;
            }

            for child_node in &node.components {
                decode_subtree_fresh_guids(world, Some(new_id), child_node)?;
            }

            Ok(new_id)
        }

        decode_subtree_fresh_guids(world, parent_id, node)
    }

    /// Encode multiple component trees (scene roots) to a JSON file.
    ///
    /// Returns an error if any component doesn't exist or file I/O fails.
    pub fn encode_scene(
        world: &World,
        root_ids: &[ComponentId],
        output_file: &str,
    ) -> Result<(), String> {
        let mut components = Vec::new();
        for &root_id in root_ids {
            components.push(Self::encode_subtree(world, root_id)?);
        }

        let scene = Scene { components };
        let json = serde_json::to_string_pretty(&scene)
            .map_err(|e| format!("Failed to serialize scene to JSON: {}", e))?;

        std::fs::write(output_file, json)
            .map_err(|e| format!("Failed to write file '{}': {}", output_file, e))?;

        Ok(())
    }

    /// Decode a scene from a JSON file and attach all roots to the world.
    ///
    /// Returns the ComponentIds of all loaded roots.
    pub fn decode_scene(world: &mut World, input_file: &str) -> Result<Vec<ComponentId>, String> {
        let json = std::fs::read_to_string(input_file)
            .map_err(|e| format!("Failed to read file '{}': {}", input_file, e))?;

        let scene: Scene = serde_json::from_str(&json)
            .map_err(|e| format!("Failed to parse scene JSON: {}", e))?;

        let mut root_ids = Vec::new();
        for root_node in &scene.components {
            let root_id = Self::decode_subtree(world, None, root_node)?;
            root_ids.push(root_id);
        }

        Ok(root_ids)
    }

    /// Encode a component subtree rooted at `root_id` to a JSON file.
    ///
    /// Returns an error if the component doesn't exist or file I/O fails.
    pub fn encode(world: &World, root_id: ComponentId, output_file: &str) -> Result<(), String> {
        let root_node = Self::encode_subtree(world, root_id)?;

        let json = serde_json::to_string_pretty(&root_node)
            .map_err(|e| format!("Failed to serialize to JSON: {}", e))?;

        std::fs::write(output_file, json)
            .map_err(|e| format!("Failed to write file '{}': {}", output_file, e))?;

        Ok(())
    }

    /// Decode a component tree from a JSON file and attach it to the world.
    ///
    /// - `parent_id`: If `Some(id)`, the loaded root becomes a child of that component.
    ///                If `None`, the loaded root becomes a top-level component.
    ///
    /// Returns the new ComponentId of the loaded root.
    pub fn decode(
        world: &mut World,
        parent_id: Option<ComponentId>,
        input_file: &str,
    ) -> Result<ComponentId, String> {
        let json = std::fs::read_to_string(input_file)
            .map_err(|e| format!("Failed to read file '{}': {}", input_file, e))?;

        let root_node: ComponentDataNode =
            serde_json::from_str(&json).map_err(|e| format!("Failed to parse JSON: {}", e))?;

        Self::decode_subtree(world, parent_id, &root_node)
    }

    /// Recursively encode a component and its children into a ComponentDataNode.
    fn encode_subtree(world: &World, cid: ComponentId) -> Result<ComponentDataNode, String> {
        let node = world
            .get_component_node(cid)
            .ok_or_else(|| format!("Component {:?} not found", cid))?;

        let component = &node.component;
        let type_name = component.name().to_string();
        let base_name = node.name.clone();
        let data = component.encode();

        // Encode children, tracking names to handle duplicates.
        let mut name_counts: HashMap<String, usize> = HashMap::new();
        let mut child_nodes = Vec::new();

        for &child_id in &node.children {
            let mut child_node = Self::encode_subtree(world, child_id)?;

            // Track name usage and append _N if duplicate.
            let count = name_counts.entry(child_node.name.clone()).or_insert(0);
            if *count > 0 {
                child_node.name = format!("{}_{}", child_node.name, count);
            }
            *count += 1;

            child_nodes.push(child_node);
        }

        Ok(ComponentDataNode {
            guid: node.guid,
            name: base_name,
            type_name,
            data,
            components: child_nodes,
        })
    }

    /// Recursively decode a ComponentDataNode, creating components in the world.
    ///
    /// Returns the ComponentId of the newly created root component.
    fn decode_subtree(
        world: &mut World,
        parent_id: Option<ComponentId>,
        node: &ComponentDataNode,
    ) -> Result<ComponentId, String> {
        // Create the component instance based on type_name.
        let mut component = Self::create_component(&node.type_name)?;

        // Decode component-specific data.
        component.decode(&node.data)?;

        // Add to world with restored GUID + stored name (assigns a fresh ComponentId).
        // Note: The name might have _N suffix which we preserve.
        let new_id =
            world.add_component_boxed_with_guid_named(node.guid, node.name.clone(), component);

        // Set parent if specified.
        if let Some(parent) = parent_id {
            world
                .set_parent(new_id, Some(parent))
                .map_err(|e| format!("Failed to set parent: {}", e))?;
        }

        // Recursively decode children.
        for child_node in &node.components {
            Self::decode_subtree(world, Some(new_id), child_node)?;
        }

        Ok(new_id)
    }

    /// Factory function: create a component instance by type name.
    ///
    /// This uses a hard-coded registry for now; could be made extensible later.
    fn create_component(
        type_name: &str,
    ) -> Result<Box<dyn crate::engine::ecs::component::Component>, String> {
        use crate::engine::ecs::component::*;

        match type_name {
            "editor" => Ok(Box::new(EditorComponent::new())),
            "transform" => Ok(Box::new(TransformComponent::new())),
            "renderable" => Ok(Box::new(RenderableComponent::new(
                crate::engine::graphics::primitives::Renderable::new(
                    crate::engine::graphics::primitives::CpuMeshHandle(0),
                    crate::engine::graphics::primitives::MaterialHandle::TOON_MESH,
                ),
            ))),
            "overlay" => Ok(Box::new(OverlayComponent::new())),
            "raycast" => Ok(Box::new(RayCastComponent::default())),
            "pointer" => Ok(Box::new(PointerComponent::default())),
            "gesture_coord_type" => Ok(Box::new(GestureCoordTypeComponent::default())),
            "raycastable" => Ok(Box::new(RaycastableComponent::enabled())),
            "background" => Ok(Box::new(BackgroundComponent::new())),
            "background_color" => Ok(Box::new(BackgroundColorComponent::new())),
            "ambient_light" => Ok(Box::new(AmbientLightComponent::new())),
            "color" => Ok(Box::new(ColorComponent::new())),
            "opacity" => Ok(Box::new(OpacityComponent::new())),
            "light_quantization" => Ok(Box::new(LightQuantizationComponent::new())),
            "emissive" => Ok(Box::new(EmissiveComponent::default())),
            "texture" => Ok(Box::new(TextureComponent::new(""))),
            "camera2d" => Ok(Box::new(Camera2DComponent::new())),
            "camera3d" => Ok(Box::new(Camera3DComponent::new())),
            "camera_xr" => Ok(Box::new(CameraXRComponent::off())),
            "controller_xr" => Ok(Box::new(ControllerXRComponent::default())),
            "point_light" => Ok(Box::new(PointLightComponent::new())),
            "directional_light" => Ok(Box::new(DirectionalLightComponent::new())),
            "uv" => Ok(Box::new(UVComponent::new())),
            "input" => Ok(Box::new(InputComponent::new())),
            "input_transform_mode" => Ok(Box::new(InputTransformModeComponent::default())),
            "openxr" => Ok(Box::new(OpenXRComponent::off())),
            "text" => Ok(Box::new(TextComponent::new(""))),
            "text_shadow" => Ok(Box::new(TextShadowComponent::new())),
            "animation" => Ok(Box::new(AnimationComponent::new())),
            "keyframe" => Ok(Box::new(KeyframeComponent::new(0.0))),
            "action" => Ok(Box::new(ActionComponent::default())),
            "audio_output" => Ok(Box::new(AudioOutputComponent::new())),
            "audio_buffer_size" => Ok(Box::new(
                crate::engine::ecs::component::AudioBufferSizeComponent::default(),
            )),
            "clock" => Ok(Box::new(ClockComponent::new())),
            "collision" => Ok(Box::new(CollisionComponent::default())),
            "collision_shape" => Ok(Box::new(CollisionShapeComponent::new(
                crate::engine::ecs::component::CollisionShape::CUBE(),
            ))),
            "gravity" => Ok(Box::new(GravityComponent::default())),
            "kinetic_response" => Ok(Box::new(KineticResponseComponent::default())),
            "joint" => Ok(Box::new(JointComponent::new(0, Vec::new()))),
            "skinned_mesh" => Ok(Box::new(SkinnedMeshComponent::new(0))),

            // Transform gizmo (renamed from "gizmo").
            "transform_gizmo" | "gizmo" => Ok(Box::new(TransformGizmoComponent::new())),
            "transform_gizmo_translate" | "gizmo_translate" => Ok(Box::new(
                TransformGizmoTranslateComponent::new(TransformGizmoAxis::X),
            )),
            "transform_gizmo_rotate" | "gizmo_rotate" => Ok(Box::new(
                TransformGizmoRotateComponent::new(TransformGizmoAxis::X),
            )),
            "transform_gizmo_scale" | "gizmo_scale" => Ok(Box::new(
                TransformGizmoScaleComponent::new(TransformGizmoAxis::X),
            )),

            _ => Err(format!("Unknown component type: '{}'", type_name)),
        }
    }
}
