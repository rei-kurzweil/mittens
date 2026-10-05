pub mod amplitude_system;
pub(crate) mod animation_keyframe_evaluator;
pub(crate) mod animation_scheduler;
pub mod animation_system;
pub mod armature_visualization_system;
pub mod asset_system;
pub mod attachment_system;
pub mod audio_decode;
pub mod audio_decode_thread;
pub mod audio_graph_compiler;
pub mod audio_input_system;
pub mod audio_sample_format_convert;
pub mod audio_system;
pub(crate) mod audio_system_fundsp;
pub mod avatar_body_yaw_system;
pub mod avatar_control_system;
pub mod bounds_system;
pub mod bounds_visualization_system;
pub mod bvh_system;
pub mod camera_system;
pub mod camera_visualization_system;
pub mod clipping_system;
pub mod clock_system;
pub(crate) mod collision_geometry;
pub mod collision_shape_inference;
pub mod collision_shape_resolver;
pub mod collision_system;
pub mod collision_visualization_system;
pub mod combine_mesh_system;
pub mod cursor_3d;
pub mod data_renderer_system;
pub mod draggable_system;
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
pub mod gltf_bounds_visualization_system;
pub mod gltf_system;
pub mod grabbable_system;
pub mod grid_gesture;
pub mod grid_system;
pub mod http_client_system;
pub mod http_server_system;
pub mod humanoid_bone_map_system;
pub mod ik;
pub mod ik_system;
pub mod implicit_surface_system;
pub mod input_system;
pub mod input_xr_gamepad_system;
pub mod joint_basis_retargeting_system;
pub mod keyboard_input_system;
pub mod layout;
pub mod light_system;
pub mod mesh_bounds_system;
pub mod mirror_system;
pub mod model;
pub mod morph_target_system;
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
pub mod rest_attachment;
pub mod router_system;
pub mod scroll_system;
pub mod secondary_motion_constraint_system;
pub mod secondary_motion_system;
pub mod selection_system;
pub mod skinned_mesh_system;
pub mod slider_system;
pub mod spring_bone_visualization_system;
pub mod static_contact_system;
pub mod system_world;
pub mod text_input_system;
pub mod text_system;
pub mod texture_system;
pub mod toggle_system;
pub mod transform_stream_system;
pub mod transform_system;
pub mod transition_system;
pub mod velocity_system;
pub mod vr_types;
pub mod xr_eye_tracking_system;
pub mod zone_query;
pub mod zone_visualization_system;

pub use amplitude_system::{AmplitudeSnapshot, AmplitudeSystem};
pub use animation_system::AnimationSystem;
pub use armature_visualization_system::ArmatureVisualizationSystem;
pub use asset_system::AssetSystem;
pub use attachment_system::AttachmentSystem;
pub use audio_input_system::AudioInputSystem;
pub use audio_system::AudioSystem;
pub use avatar_body_yaw_system::AvatarBodyYawSystem;
pub use avatar_control_system::AvatarControlSystem;
pub use bounds_visualization_system::BoundsVisualizationSystem;
pub use bvh_system::BvhSystem;
pub use camera_system::{Camera3D, CameraHandle, CameraSystem};
pub use camera_visualization_system::{CameraVisualizationRequest, CameraVisualizationSystem};
pub use clipping_system::ClippingSystem;
pub use clock_system::{ClockDriver, ClockSystem};
pub use collision_system::CollisionSystem;
pub use collision_visualization_system::{
    CollisionVisualizationMode, CollisionVisualizationRequest, CollisionVisualizationSystem,
};
pub use combine_mesh_system::CombineMeshSystem;
pub use cursor_3d::Cursor3dSystem;
pub use data_renderer_system::{
    DataRendererSystem, DetailRendererSpec, ItemRendererSpec, RendererSpec, UiDetailItem, UiItem,
    UiItemKind,
};
pub use draggable_system::DraggableSystem;
pub use editor::EditorContextSystem;
pub use editor_inspector_system::EditorInspectorSystem;
pub use editor_paint_system::EditorPaintSystem;
pub use editor_system::EditorSystem;
pub use fit_bounds_system::FitBoundsSystem;
pub use gesture_system::{GestureState, GestureSystem};
pub use gizmo_system::TransformGizmoSystem;
pub use gltf_bounds_visualization_system::GltfBoundsVisualizationSystem;
pub use gltf_system::GLTFSystem;
pub use grabbable_system::GrabbableSystem;
pub use grid_system::GridSystem;
pub use http_client_system::HttpClientSystem;
pub use http_server_system::HttpServerSystem;
pub use humanoid_bone_map_system::{
    HumanoidBoneMapReport, HumanoidBoneMapSystem, HumanoidSlotProvenance, HumanoidSlotReport,
    HumanoidSlotStatus, ResolvedHumanoidTarget, ResolvedHumanoidTargetKind,
};
pub use ik::HeadPoseBodyXzFollowSystem;
pub use ik_system::IKSystem;
pub use implicit_surface_system::ImplicitSurfaceSystem;
pub use input_system::InputSystem;
pub use input_xr_gamepad_system::InputXRGamepadSystem;
pub use joint_basis_retargeting_system::{
    JointBasisRetargetingSystem, LandmarkDirection, ResolvedRetargetBasis, RetargetBasisDefinition,
    RetargetBasisDiagnosticSnapshot, RetargetBasisProvenance, RetargetBasisStatus,
};
pub use keyboard_input_system::KeyboardInputSystem;
pub use layout::LayoutSystem;
pub use light_system::LightSystem;
pub use mesh_bounds_system::{MeshBoundsSystem, MeshOutputBounds, MeshOutputKind};
pub use mirror_system::MirrorSystem;
pub use music_system::MusicSystem;
pub use openxr_system::OpenXRSystem as XrSystem;
pub use pipeline_system::PipelineSystem;
pub use pointer_system::{PointerActivations, PointerSystem, PointerTopologyContext};
pub use pose_capture_system::PoseCaptureSystem;
pub use raycast_system::{PointerRaySnapshot, RayCastSystem};
pub use render_to_texture_system::RenderToTextureSystem;
pub use renderable_system::RenderableSystem;
pub use renderer_stats_system::RendererStatsSystem;
pub use rest_attachment::ResolvedRestAttachment;
pub use router_system::RouterSystem;
pub use scroll_system::ScrollingSystem;
pub use secondary_motion_constraint_system::SecondaryMotionConstraintSystem;
pub use secondary_motion_system::SecondaryMotionSystem;
pub use secondary_motion_system::{
    SecondaryMotionChainSnapshot, SecondaryMotionColliderSnapshot, SecondaryMotionSegmentSnapshot,
};
pub use selection_system::SelectionSystem;
pub use skinned_mesh_system::SkinnedMeshSystem;
pub use slider_system::SliderSystem;
pub use spring_bone_visualization_system::{
    SpringBoneVisualizationRequest, SpringBoneVisualizationSystem,
};
pub use system_world::SystemWorld;
pub use text_input_system::TextInputSystem;
pub use text_system::TextSystem;
pub use texture_system::TextureSystem;
pub use toggle_system::ToggleSystem;
pub use transform_stream_system::TransformStreamSystem;
pub use transform_system::{TransformAccessError, TransformSystem};
pub use transition_system::TransitionSystem;
pub use velocity_system::VelocitySystem;
pub use vr_types::{XrGamepadState, XrHandGamepadState, XrInputState};
pub use xr_eye_tracking_system::XREyeTrackingSystem;
pub use zone_query::{
    ZoneContact, ZoneOverlap, ZoneQueryError, ZoneRelation, ZoneSeparation, ZoneSweepHit,
    classify_point as classify_zone_point, contact_zones, overlap_zones, resolve_zone_frame,
    sweep_capsule_floor, zones_in_subtree,
};
pub use zone_visualization_system::{ZoneVisualizationRequest, ZoneVisualizationSystem};

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

pub mod zone_observation_system;
