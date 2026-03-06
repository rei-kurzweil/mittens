pub mod action_system;
pub mod animation_system;
pub(crate) mod animation_system_evaluator;
pub mod audio_graph_compiler;
pub mod audio_system;
pub(crate) mod audio_system_fundsp;
pub mod bvh_system;
pub mod camera_system;
pub mod clock_system;
pub mod collision_system;
pub mod editor_system;
pub mod gesture_system;
pub mod gizmo_system;
pub mod gltf_system;
pub mod input_system;
pub mod kinetic_response_system;
pub mod light_system;
pub mod model;
pub mod music_system;
pub mod openxr_system;
pub mod raycast_system;
pub mod renderable_system;
pub mod skinned_mesh_system;
pub mod system_world;
pub mod text_system;
pub mod texture_system;
pub mod transform_system;

pub use animation_system::AnimationSystem;
pub use audio_system::AudioSystem;
pub use bvh_system::BvhSystem;
pub use camera_system::{Camera3D, CameraHandle, CameraSystem};
pub use clock_system::{ClockDriver, ClockSystem};
pub use collision_system::CollisionSystem;
pub use editor_system::EditorSystem;
pub use gesture_system::{GestureState, GestureSystem};
pub use gizmo_system::TransformGizmoSystem;
pub use gltf_system::GLTFSystem;
pub use input_system::InputSystem;
pub use kinetic_response_system::KineticResponseSystem;
pub use light_system::LightSystem;
pub use music_system::MusicSystem;
pub use openxr_system::OpenXRSystem;
pub use raycast_system::RayCastSystem;
pub use renderable_system::RenderableSystem;
pub use skinned_mesh_system::SkinnedMeshSystem;
pub use system_world::SystemWorld;
pub use text_system::TextSystem;
pub use texture_system::TextureSystem;
pub use transform_system::TransformSystem;

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
