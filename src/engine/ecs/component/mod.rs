pub mod camera2d;
pub mod camera3d;
pub mod color;
pub mod gltf;
pub mod input;
pub mod input_transform_mode;
pub mod mesh;
pub mod point_light;
pub mod renderable;
pub mod texture;
pub mod transform;
pub mod uv;

pub use camera2d::Camera2DComponent;
pub use camera3d::Camera3DComponent;
pub use color::ColorComponent;
pub use self::gltf::GLTFComponent;
pub use input::InputComponent;
pub use input_transform_mode::{ForwardAxis, InputTransformModeComponent, RollAxis};
pub use self::mesh::MeshComponent;
pub use point_light::PointLightComponent;
pub use renderable::RenderableComponent;
pub use texture::{CatEngineTextureFormat, TextureComponent};
pub use transform::TransformComponent;
pub use uv::UVComponent;

/// For now, our "LightComponent" is a point light.
pub type LightComponent = point_light::PointLightComponent;

/// World-owned record for a component payload plus its topology.
///
/// This is the building block of the component-centric ECS: a single flat store of records
/// in `World`, each record carrying its own parent/children handles.

pub struct ComponentNode {
    pub guid: uuid::Uuid,
    pub name: String,
    pub component: Box<dyn Component>,
    pub parent: Option<crate::engine::ecs::ComponentId>,
    pub children: Vec<crate::engine::ecs::ComponentId>,
}

impl ComponentNode {
    pub fn new(component: Box<dyn Component>) -> Self {
        let name = component.name().to_string();
        Self {
            guid: uuid::Uuid::new_v4(),
            name,
            component,
            parent: None,
            children: Vec::new(),
        }
    }

    pub fn new_named(name: impl Into<String>, component: Box<dyn Component>) -> Self {
        Self {
            guid: uuid::Uuid::new_v4(),
            name: name.into(),
            component,
            parent: None,
            children: Vec::new(),
        }
    }

    pub fn new_with_guid_named(
        guid: uuid::Uuid,
        name: impl Into<String>,
        component: Box<dyn Component>,
    ) -> Self {
        Self {
            guid,
            name: name.into(),
            component,
            parent: None,
            children: Vec::new(),
        }
    }
}

/// Component interface.
/// `init` runs when the component is registered
pub trait Component: std::any::Any {
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    /// Short debug/type name for this component kind (e.g. "transform", "camera").
    fn name(&self) -> &'static str;

    fn set_id(&mut self, _component: crate::engine::ecs::ComponentId) {}

    /// Called when component is added to the World
    fn init(
        &mut self,
        _queue: &mut crate::engine::ecs::CommandQueue,
        _component: crate::engine::ecs::ComponentId,
    ) {
    }

    /// Called when component is removed from the World.
    fn cleanup(
        &mut self,
        _queue: &mut crate::engine::ecs::CommandQueue,
        _component: crate::engine::ecs::ComponentId,
    ) {
    }

    /// Encode component data to a HashMap for serialization.
    ///
    /// Components should serialize their data fields (not runtime handles).
    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        std::collections::HashMap::new()
    }

    /// Decode component data from a HashMap after deserialization.
    ///
    /// Components should restore their data fields from the map.
    fn decode(
        &mut self,
        _data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        Ok(())
    }
}
