//! Owned services and retained system state.
use super::*;

/// System world that holds and runs all registered systems.
#[derive(Debug, Default)]
pub struct SystemWorld {
    pub rx: RxWorld,
    pub benchmarks: crate::engine::benchmark::Benchmarks,

    /// REPL command queue (executed by Universe on the main thread).
    pub(super) repl_command_queue: Vec<String>,
    pub(super) phase_profile: SystemPhaseProfile,
    pub(super) vr_perf_enabled: bool,
    pub(super) vr_perf_pre_xr: VrPerfPreXrCpu,

    pub clock: ClockSystem,
    pub audio: AudioSystem,
    pub audio_input: AudioInputSystem,
    pub amplitude: AmplitudeSystem,
    pub music: MusicSystem,
    pub animation: AnimationSystem,
    pub transition: TransitionSystem,

    pub transform_operator: TransformOperatorSystem,
    pub transform: TransformSystem,
    pub bvh: BvhSystem,
    pub zone_visualization: ZoneVisualizationSystem,
    pub spring_bone_visualization: crate::engine::ecs::system::SpringBoneVisualizationSystem,
    pub camera_visualization: crate::engine::ecs::system::CameraVisualizationSystem,
    pub static_contact: StaticContactSystem,
    pub zone_observation:
        crate::engine::ecs::system::zone_observation_system::ZoneObservationSystem,
    pub skinned_mesh: SkinnedMeshSystem,
    pub combine_mesh: CombineMeshSystem,
    pub implicit_surface: ImplicitSurfaceSystem,
    pub destroyable: crate::engine::ecs::system::destroyable_system::DestroyableSystem,
    pub mesh_bounds: MeshBoundsSystem,
    pub renderable: RenderableSystem,
    pub material: crate::engine::ecs::system::material_system::MaterialSystem,
    pub clipping: ClippingSystem,
    pub renderer_stats: RendererStatsSystem,
    pub router: RouterSystem,
    pub http_server: HttpServerSystem,
    pub http_client: HttpClientSystem,
    pub xr_eye_tracking: XREyeTrackingSystem,
    pub scrolling: ScrollingSystem,

    pub pointer: PointerSystem,
    pub attachment: crate::engine::ecs::system::AttachmentSystem,
    pub grabbable: crate::engine::ecs::system::GrabbableSystem,
    pub draggable: crate::engine::ecs::system::DraggableSystem,
    pub raycast: RayCastSystem,

    pub editor: EditorSystem,
    pub cursor_3d: Cursor3dSystem,
    pub editor_context: EditorContextSystem,
    pub editor_inspector: EditorInspectorSystem,
    pub selection: SelectionSystem,
    pub toggle: ToggleSystem,
    pub slider: crate::engine::ecs::system::SliderSystem,
    pub asset_system: AssetSystem,
    pub fit_bounds: FitBoundsSystem,
    pub grid: GridSystem,
    pub editor_paint: EditorPaintSystem,
    pub avatar_body_yaw: AvatarBodyYawSystem,
    pub avatar_control: AvatarControlSystem,
    pub head_neck_body_constraint: HeadNeckBodyConstraintSystem,
    pub lower_body_ik: crate::engine::ecs::system::LowerBodyIkSystem,
    pub ik: IKSystem,
    pub secondary_motion: SecondaryMotionSystem,
    pub secondary_motion_constraint: SecondaryMotionConstraintSystem,
    pub joint_basis_retargeting: crate::engine::ecs::system::JointBasisRetargetingSystem,
    pub hand_tracking: crate::engine::ecs::system::HandTrackingSystem,
    pub humanoid_bone_map: crate::engine::ecs::system::HumanoidBoneMapSystem,

    pub gesture: GestureSystem,
    pub transform_gizmo: TransformGizmoSystem,

    pub bounds: BoundsSystem,
    pub bounds_visualization: BoundsVisualizationSystem,
    pub layout: LayoutSystem,

    pub gltf: GLTFSystem,
    pub armature_visualization: ArmatureVisualizationSystem,
    pub gltf_bounds_visualization: GltfBoundsVisualizationSystem,

    pub xr: XrSystem,
    pub input_xr_gamepad: InputXRGamepadSystem,
    pub velocity: VelocitySystem,

    pub pose_capture: PoseCaptureSystem,

    pub pipeline: PipelineSystem,

    pub camera: CameraSystem,
    pub input: InputSystem,
    pub keyboard_input: KeyboardInputSystem,
    pub light: LightSystem,
    pub mirror: MirrorSystem,
    pub graphics_settings: crate::engine::ecs::system::GraphicsSettingsSystem,
    pub xr_settings: crate::engine::ecs::system::XrSettingsSystem,

    pub text: TextSystem,
    pub text_input: TextInputSystem,
    pub render_to_texture: RenderToTextureSystem,
    pub texture: TextureSystem,
}
