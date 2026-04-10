pub mod action;
pub mod avatar_body_yaw;
pub mod avatar_control;
pub mod ambient_light;
pub mod animation;
pub mod bloom;
pub mod blur_pass;
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
pub mod controller_xr;
pub mod directional_light;
pub mod editor;
pub mod emissive_pass;
pub mod emissive;
pub mod inspector_panel;
pub mod gesture_coord_type;
pub mod gizmo;
pub mod normal_visualisation;
pub mod gltf;
pub mod gravity;
pub mod html_element;
pub mod layout;
pub mod ik_chain;
pub mod input;
pub mod input_xr;
pub mod input_transform_mode;
pub mod keyframe;
pub mod kinetic_response;
pub mod light_quantization;
pub mod mesh;
pub mod music_note;
pub mod opacity;
pub mod overlay;
pub mod render_graph;
pub mod scrolling;
pub mod selectable;
pub mod signal_route_upward;
pub mod skinned_mesh;
pub mod style;
pub mod transparent_cutout;

pub mod openxr;
pub mod point_light;
pub mod pointer;
pub mod raycast;
pub mod raycastable;
pub mod raycastable_shape;
pub mod renderable;
pub mod renderer_stats;
pub mod renderer_settings;
pub mod text;
pub mod text_shadow;
pub mod texture;
pub mod texture_filtering;
pub mod transition;
pub mod transform;
pub mod transform_pipeline;
pub mod world_panel;
pub mod transform_pipeline_map;
pub mod transform_temporal_filter;
pub mod uv;

pub use self::gltf::GLTFComponent;
pub use self::mesh::MeshComponent;
pub use crate::engine::ecs::system::model::collision_types::{CollisionMode, CollisionShape};
pub use action::ActionComponent;
pub use avatar_body_yaw::AvatarBodyYawComponent;
pub use avatar_control::AvatarControlComponent;
pub use ambient_light::AmbientLightComponent;
pub use animation::AnimationComponent;
pub use animation::AnimationState;
pub use bloom::BloomComponent;
pub use blur_pass::BlurPassComponent;
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
pub use controller_xr::{ControllerHand, ControllerPoseKind, ControllerXRComponent};
pub use directional_light::DirectionalLightComponent;
pub use editor::EditorComponent;
pub use editor::TransformGizmoCoordSpace;
pub use emissive_pass::EmissivePassComponent;
pub use inspector_panel::InspectorPanelComponent;
pub use emissive::EmissiveComponent;
pub use normal_visualisation::NormalVisualisationComponent;
pub use gesture_coord_type::{GestureCoordType, GestureCoordTypeComponent};
pub use gizmo::{
    TransformGizmoAxis, TransformGizmoComponent, TransformGizmoRotateComponent,
    TransformGizmoScaleComponent, TransformGizmoTranslateComponent,
};
pub use gravity::GravityComponent;
pub use html_element::{ElementType, HtmlElementComponent};
pub use layout::LayoutComponent;
pub use ik_chain::{IKChainComponent, IKSolver};
pub use input::InputComponent;
pub use input_xr::InputXRComponent;
pub use input_transform_mode::{ForwardAxis, InputTransformModeComponent, RollAxis};
pub use keyframe::KeyframeComponent;
pub use kinetic_response::{KineticResponseComponent, KineticResponseMode};
pub use light_quantization::LightQuantizationComponent;
pub(crate) use music_note::NotePitch;
pub use music_note::{MusicNote, MusicNoteComponent};
pub use opacity::OpacityComponent;
pub use openxr::OpenXRComponent;
pub use overlay::OverlayComponent;
pub use selectable::SelectableComponent;
pub use point_light::PointLightComponent;
pub use pointer::PointerComponent;
pub use raycast::{RayCastComponent, RayCastMode};
pub use raycastable::{PointerEvents, RaycastableComponent};
pub use raycastable_shape::{RaycastableShapeComponent, RaycastableShapeType};
pub use renderable::RenderableComponent;
pub use render_graph::RenderGraphComponent;
pub use renderer_stats::RendererStatsComponent;
pub use renderer_settings::RendererSettingsComponent;
pub use signal_route_upward::SignalRouteUpwardComponent;
pub use skinned_mesh::SkinnedMeshComponent;
pub use style::{
    AlignItems, Display, EdgeInsets, FlexDirection, FlexWrap, JustifyContent,
    Overflow, Position, SizeDimension, StyleComponent, StylePatch,
};
pub use text::TextComponent;
pub use text_shadow::TextShadowComponent;
pub use texture::{CatEngineTextureFormat, TextureComponent};
pub use texture_filtering::TextureFilteringComponent;
pub use transition::{TransitionComponent, TransitionEasing, TransitionReplacePolicy};
pub use transform::TransformComponent;
pub use world_panel::WorldPanelComponent;
pub use transform_pipeline::{
    TransformDropComponent, TransformForkTRSComponent, TransformMergeTRSComponent,
    TransformPipelineComponent, TransformPipelineOutputComponent,
    TransformSampleAncestorComponent,
};
pub use transform_pipeline_map::{
    TransformMapRotationComponent, TransformMapScaleComponent, TransformMapTranslationComponent,
};
pub use transform_temporal_filter::{
    QuatExtractYawComponent, QuatTemporalFilterComponent, QuatYawFollowComponent,
    Vector3TemporalFilterComponent,
};
pub use scrolling::ScrollingComponent;
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
    /// Engine-side type identifier from `Component::name()` (e.g. `"transform"`, `"text"`).
    /// Set at construction, never changes.
    pub component_type: String,
    /// User-assigned label for this node (e.g. `name = "catgirl"` in MMS).
    /// Defaults to empty. Used for `#label` query selectors.
    pub name: String,
    /// CSS-style class membership (e.g. `class = "avatar"` in MMS).
    /// Used for `.class` query selectors.
    pub classes: Vec<String>,
    pub component: Box<dyn Component>,
    pub parent: Option<crate::engine::ecs::ComponentId>,
    pub children: Vec<crate::engine::ecs::ComponentId>,
    pub initialized: bool,
}

impl ComponentNode {
    pub fn new(component: Box<dyn Component>) -> Self {
        let component_type = component.name().to_string();
        Self {
            guid: uuid::Uuid::new_v4(),
            component_type,
            name: String::new(),
            classes: Vec::new(),
            component,
            parent: None,
            children: Vec::new(),
            initialized: false,
        }
    }

    /// Create a node with a user-assigned label (`name`).
    /// `component_type` is still derived from `component.name()`.
    pub fn new_named(name: impl Into<String>, component: Box<dyn Component>) -> Self {
        let component_type = component.name().to_string();
        Self {
            guid: uuid::Uuid::new_v4(),
            component_type,
            name: name.into(),
            classes: Vec::new(),
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
        let component_type = component.name().to_string();
        Self {
            guid,
            component_type,
            name: name.into(),
            classes: Vec::new(),
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
        _emit: &mut dyn crate::engine::ecs::SignalEmitter,
        _component: crate::engine::ecs::ComponentId,
    ) {
    }

    /// Called when component is removed from the World.
    fn cleanup(
        &mut self,
        _emit: &mut dyn crate::engine::ecs::SignalEmitter,
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
