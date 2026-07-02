pub mod action_system;
pub(crate) mod animation_keyframe_evaluator;
pub(crate) mod animation_scheduler;
pub mod animation_system;
pub mod armature_visualization_system;
pub mod asset_system;
pub mod audio_decode;
pub mod audio_decode_thread;
pub mod audio_graph_compiler;
pub mod audio_sample_format_convert;
pub mod audio_system;
pub(crate) mod audio_system_fundsp;
pub mod avatar_body_yaw_system;
pub mod avatar_control_system;
pub mod bone_mapping_system;
pub mod bounds_system;
pub mod bvh_system;
pub mod camera_system;
pub mod clipping_system;
pub mod clock_system;
pub mod collision_system;
pub mod cursor_3d;
pub mod data_renderer_system;
pub mod editor;
pub mod editor_inspector_system;
pub(crate) mod editor_inspector_system_stopgap_mms_adapter;
pub mod editor_paint_system;
pub mod editor_paint_system_state_manager;
pub mod editor_scene_hit;
pub mod editor_system;
pub mod fit_bounds_system;
pub mod gesture_system;
pub mod gizmo_system;
pub mod gltf_system;
pub mod grid_system;
pub mod ik;
pub mod ik_system;
pub mod input_system;
pub mod input_xr_gamepad_system;
pub mod kinetic_response_system;
pub mod layout;
pub mod light_system;
pub mod mirror_system;
pub mod model;
pub mod music_system;
pub mod object_placement_preview;
pub mod openxr_system;
pub mod paint_placement;
pub mod panel_system;
pub mod pipeline_system;
pub mod pointer_system;
pub mod pose_capture_system;
pub mod raycast_system;
pub mod render_to_texture_system;
pub mod renderable_system;
pub mod renderer_stats_system;
pub mod router_system;
pub mod scroll_system;
pub mod selection_system;
pub mod skinned_mesh_system;
pub mod system_world;
pub mod text_input_system;
pub mod text_system;
pub mod texture_system;
pub mod transform_stream_system;
pub mod transform_system;
pub mod transition_system;
pub mod vr_types;

pub use animation_system::AnimationSystem;
pub use armature_visualization_system::ArmatureVisualizationSystem;
pub use asset_system::AssetSystem;
pub use audio_system::AudioSystem;
pub use avatar_body_yaw_system::AvatarBodyYawSystem;
pub use avatar_control_system::AvatarControlSystem;
pub use bvh_system::BvhSystem;
pub use camera_system::{Camera3D, CameraHandle, CameraSystem};
pub use clipping_system::ClippingSystem;
pub use clock_system::{ClockDriver, ClockSystem};
pub use collision_system::CollisionSystem;
pub use cursor_3d::Cursor3dSystem;
pub use data_renderer_system::{
    DataRendererSystem, DetailRendererSpec, ItemRendererSpec, RendererSpec, UiDetailItem, UiItem,
    UiItemKind,
};
pub use editor::EditorContextSystem;
pub use editor_inspector_system::EditorInspectorSystem;
pub use editor_paint_system::EditorPaintSystem;
pub use editor_system::EditorSystem;
pub use fit_bounds_system::FitBoundsSystem;
pub use gesture_system::{GestureState, GestureSystem};
pub use gizmo_system::TransformGizmoSystem;
pub use gltf_system::GLTFSystem;
pub use grid_system::GridSystem;
pub use ik::HeadPoseBodyXzFollowSystem;
pub use ik_system::IKSystem;
pub use input_system::InputSystem;
pub use input_xr_gamepad_system::InputXRGamepadSystem;
pub use kinetic_response_system::KineticResponseSystem;
pub use layout::LayoutSystem;
pub use light_system::LightSystem;
pub use mirror_system::MirrorSystem;
pub use music_system::MusicSystem;
pub use openxr_system::OpenXRSystem as XrSystem;
pub use pipeline_system::PipelineSystem;
pub use pointer_system::{PointerActivations, PointerSystem, PointerTopologyContext};
pub use pose_capture_system::PoseCaptureSystem;
pub use raycast_system::RayCastSystem;
pub use render_to_texture_system::RenderToTextureSystem;
pub use renderable_system::RenderableSystem;
pub use renderer_stats_system::RendererStatsSystem;
pub use router_system::RouterSystem;
pub use scroll_system::ScrollingSystem;
pub use selection_system::SelectionSystem;
pub use skinned_mesh_system::SkinnedMeshSystem;
pub use system_world::SystemWorld;
pub use text_input_system::TextInputSystem;
pub use text_system::TextSystem;
pub use texture_system::TextureSystem;
pub use transform_stream_system::TransformStreamSystem;
pub use transform_system::TransformSystem;
pub use transition_system::TransitionSystem;
pub use vr_types::{XrGamepadState, XrHandGamepadState, XrInputState};

use super::World;
use crate::engine::graphics::VisualWorld;
use crate::engine::user_input::InputState;

/// Individual system trait that processes specific component types.
///
/// This trait lives in `ecs/system/mod.rs` and is used by `SystemWorld` and all systems.
pub trait System: std::fmt::Debug {
    fn tick(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        input: &InputState,
        dt_sec: f32,
    );
}
