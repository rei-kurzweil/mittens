pub mod action;
pub mod ambient_light;
pub mod animation;
pub mod audio_band_pass_filter;
pub mod audio_buffer_size;
pub mod audio_gain;
pub mod audio_high_pass_filter;
pub mod audio_limiter;
pub mod audio_low_pass_filter;
pub mod audio_mix;
pub mod audio_oscillator;
pub mod audio_output;
pub mod background;
pub mod background_color;
pub mod camera_2d;
pub mod camera_3d;
pub mod camera_xr;
pub mod clock;
pub mod collision;
pub mod collision_shape;
pub mod color;
pub mod directional_light;
pub mod emissive;
pub mod gltf;
pub mod input;
pub mod input_transform_mode;
pub mod joint;
pub mod keyframe;
pub mod light_quantization;
pub mod mesh;
pub mod music_note;
pub mod opacity;
pub mod skinned_mesh;
pub mod transparent_cutout;

pub mod openxr;
pub mod point_light;
pub mod raycast;
pub mod renderable;
pub mod text;
pub mod texture;
pub mod texture_filtering;
pub mod transform;
pub mod uv;

pub use self::gltf::GLTFComponent;
pub use self::mesh::MeshComponent;
pub use crate::engine::ecs::system::model::collision_types::{CollisionMode, CollisionShape};
pub use action::ActionComponent;
pub use action::{Action, ActionMethod};
pub use ambient_light::AmbientLightComponent;
pub use animation::AnimationComponent;
pub use animation::AnimationState;
pub use audio_band_pass_filter::*;
pub use audio_buffer_size::AudioBufferSizeComponent;
pub use audio_gain::*;
pub use audio_high_pass_filter::*;
pub use audio_limiter::*;
pub use audio_low_pass_filter::*;
pub use audio_mix::AudioMixComponent;
pub use audio_oscillator::{AudioOscillator, AudioOscillatorComponent, OscillatorType};
pub use audio_output::AudioOutputComponent;
pub use background::BackgroundComponent;
pub use background_color::BackgroundColorComponent;
pub use camera_2d::Camera2DComponent;
pub use camera_3d::Camera3DComponent;
pub use camera_xr::CameraXRComponent;
pub use clock::ClockComponent;
pub use collision::CollisionComponent;
pub use collision_shape::CollisionShapeComponent;
pub use color::ColorComponent;
pub use directional_light::DirectionalLightComponent;
pub use emissive::EmissiveComponent;
pub use input::InputComponent;
pub use input_transform_mode::{ForwardAxis, InputTransformModeComponent, RollAxis};
pub use joint::JointComponent;
pub use keyframe::KeyframeComponent;
pub use light_quantization::LightQuantizationComponent;
pub(crate) use music_note::NotePitch;
pub use music_note::{MusicNote, MusicNoteComponent};
pub use opacity::OpacityComponent;
pub use openxr::OpenXRComponent;
pub use point_light::PointLightComponent;
pub use raycast::{RayCastComponent, RayCastMode};
pub use renderable::RenderableComponent;
pub use skinned_mesh::SkinnedMeshComponent;
pub use text::TextComponent;
pub use texture::{CatEngineTextureFormat, TextureComponent};
pub use texture_filtering::TextureFilteringComponent;
pub use transform::TransformComponent;
pub use transparent_cutout::TransparentCutoutComponent;
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
    pub initialized: bool,
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
            initialized: false,
        }
    }

    pub fn new_named(name: impl Into<String>, component: Box<dyn Component>) -> Self {
        Self {
            guid: uuid::Uuid::new_v4(),
            name: name.into(),
            component,
            parent: None,
            children: Vec::new(),
            initialized: false,
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
            initialized: false,
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
