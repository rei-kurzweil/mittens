use super::World;
use crate::engine::ecs::ComponentId;
use crate::engine::ecs::RxWorld;
use crate::engine::ecs::SignalKind;
use crate::engine::ecs::system::ArmatureVisualizationSystem;
use crate::engine::ecs::system::BvhSystem;
use crate::engine::ecs::system::CameraSystem;
use crate::engine::ecs::system::ClippingSystem;
use crate::engine::ecs::system::ClockSystem;
use crate::engine::ecs::system::CollisionSystem;
use crate::engine::ecs::system::GLTFSystem;
use crate::engine::ecs::system::InputSystem;
use crate::engine::ecs::system::InputXRGamepadSystem;
use crate::engine::ecs::system::KineticResponseSystem;
use crate::engine::ecs::system::LightSystem;
use crate::engine::ecs::system::MirrorSystem;
use crate::engine::ecs::system::MusicSystem;
use crate::engine::ecs::system::PipelineSystem;
use crate::engine::ecs::system::PointerSystem;
use crate::engine::ecs::system::PoseCaptureSystem;
use crate::engine::ecs::system::RayCastSystem;
use crate::engine::ecs::system::RenderToTextureSystem;
use crate::engine::ecs::system::RenderableSystem;
use crate::engine::ecs::system::RendererStatsSystem;
use crate::engine::ecs::system::RouterSystem;
use crate::engine::ecs::system::ScrollingSystem;
use crate::engine::ecs::system::SkinnedMeshSystem;
use crate::engine::ecs::system::System;
use crate::engine::ecs::system::TextInputSystem;
use crate::engine::ecs::system::TextSystem;
use crate::engine::ecs::system::TextureSystem;
use crate::engine::ecs::system::TransformStreamSystem;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::system::TransitionSystem;
use crate::engine::ecs::system::XrSystem;
use crate::engine::ecs::system::bounds_system::BoundsSystem;
use crate::engine::ecs::system::{AnimationSystem, AudioSystem};
use crate::engine::ecs::system::{
    AssetSystem, AvatarBodyYawSystem, AvatarControlSystem, Cursor3dSystem, EditorContextSystem,
    EditorInspectorSystem, EditorPaintSystem, EditorSystem, FitBoundsSystem, GestureSystem,
    GridSystem, HeadPoseBodyXzFollowSystem, IKSystem, LayoutSystem, SelectionSystem,
    TransformGizmoSystem,
};
use crate::engine::graphics::{RenderAssets, RenderUploader, VisualWorld};
use crate::engine::user_input::InputState;
use std::path::Path;

/// System world that holds and runs all registered systems.
#[derive(Debug, Default)]
pub struct SystemWorld {
    pub rx: RxWorld,

    /// REPL command queue (executed by Universe on the main thread).
    repl_command_queue: Vec<String>,

    pub clock: ClockSystem,
    pub audio: AudioSystem,
    pub music: MusicSystem,
    pub animation: AnimationSystem,
    pub transition: TransitionSystem,

    pub transform_stream: TransformStreamSystem,
    pub transform: TransformSystem,
    pub bvh: BvhSystem,
    pub collision: CollisionSystem,
    pub kinetic_response: KineticResponseSystem,
    pub skinned_mesh: SkinnedMeshSystem,
    pub renderable: RenderableSystem,
    pub clipping: ClippingSystem,
    pub renderer_stats: RendererStatsSystem,
    pub router: RouterSystem,
    pub scrolling: ScrollingSystem,

    pub pointer: PointerSystem,
    pub raycast: RayCastSystem,

    pub editor: EditorSystem,
    pub cursor_3d: Cursor3dSystem,
    pub editor_context: EditorContextSystem,
    pub editor_inspector: EditorInspectorSystem,
    pub selection: SelectionSystem,
    pub asset_system: AssetSystem,
    pub fit_bounds: FitBoundsSystem,
    pub grid: GridSystem,
    pub editor_paint: EditorPaintSystem,
    pub avatar_body_yaw: AvatarBodyYawSystem,
    pub avatar_control: AvatarControlSystem,
    pub head_pose_body_xz_follow: HeadPoseBodyXzFollowSystem,
    pub ik: IKSystem,

    pub gesture: GestureSystem,
    pub transform_gizmo: TransformGizmoSystem,

    pub bounds: BoundsSystem,
    pub layout: LayoutSystem,

    pub gltf: GLTFSystem,
    pub armature_visualization: ArmatureVisualizationSystem,

    pub xr: XrSystem,
    pub input_xr_gamepad: InputXRGamepadSystem,

    pub pose_capture: PoseCaptureSystem,

    pub pipeline: PipelineSystem,

    pub camera: CameraSystem,
    pub input: InputSystem,
    pub light: LightSystem,
    pub mirror: MirrorSystem,

    pub text: TextSystem,
    pub text_input: TextInputSystem,
    pub render_to_texture: RenderToTextureSystem,
    pub texture: TextureSystem,
}

#[cfg(test)]
mod tests {
    use super::SystemWorld;
    use crate::engine::ecs::CommandQueue;
    use crate::engine::ecs::World;
    use crate::engine::ecs::component::{
        BoundsComponent, ColorComponent, MirrorComponent, RenderableComponent,
        StencilClipComponent, TextureComponent, TransformComponent,
    };
    use crate::engine::ecs::system::System;
    use crate::engine::graphics::primitives::{MaterialHandle, MeshHandle, TextureHandle};
    use crate::engine::graphics::{
        CpuMesh, GpuRenderable, MeshUploader, RenderAssets, TextureUploader, VisualWorld,
    };

    #[derive(Default)]
    struct TestUploader {
        next_mesh: u32,
        next_texture: u32,
    }

    impl MeshUploader for TestUploader {
        fn upload_mesh(
            &mut self,
            _mesh: &CpuMesh,
        ) -> Result<MeshHandle, Box<dyn std::error::Error>> {
            let handle = MeshHandle(self.next_mesh);
            self.next_mesh += 1;
            Ok(handle)
        }
    }

    impl TextureUploader for TestUploader {
        fn upload_texture_rgba8(
            &mut self,
            _rgba: &[u8],
            _width: u32,
            _height: u32,
        ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
            let handle = TextureHandle(self.next_texture);
            self.next_texture += 1;
            Ok(handle)
        }

        fn upload_texture_bc7(
            &mut self,
            _bc7_blocks: &[u8],
            _width: u32,
            _height: u32,
            _srgb: bool,
        ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
            let handle = TextureHandle(self.next_texture);
            self.next_texture += 1;
            Ok(handle)
        }
    }

    fn register_test_instance(
        visuals: &mut VisualWorld,
        component: crate::engine::ecs::ComponentId,
    ) -> crate::engine::graphics::primitives::InstanceHandle {
        visuals.register(
            component,
            GpuRenderable::new(MeshHandle(1), MaterialHandle::TOON_MESH),
            Default::default(),
            [1.0, 1.0, 1.0, 1.0],
            1.0,
            false,
            false,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        )
    }

    fn configure_test_window_camera(visuals: &mut VisualWorld) {
        let proj = crate::engine::ecs::system::camera_system::Camera3D::perspective_rh_zo(
            60.0f32.to_radians(),
            1.0,
            0.1,
            100.0,
        );
        let view = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, -1.0, 1.0],
        ];
        let mut transform = crate::engine::graphics::primitives::Transform::default();
        transform.translation = [0.0, 0.0, 1.0];
        transform.recompute_model();
        transform.matrix_world = transform.model;
        visuals.set_camera_mono_for_target_with_transform(
            crate::engine::graphics::CameraTarget::Window,
            view,
            proj,
            transform,
        );
    }

    #[test]
    fn prepare_render_resyncs_stencil_state_after_renderable_flush() {
        let mut world = World::default();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();
        let mut render_assets = RenderAssets::new();
        let mut uploader = TestUploader::default();

        let root = world.add_component(TransformComponent::new());

        let clip_scope =
            world.add_component_boxed_named("clip_scope", Box::new(TransformComponent::new()));
        let clip_bg = world.add_component_boxed_named("__bg", Box::new(TransformComponent::new()));
        let clip_bg_color = world.add_component(ColorComponent::rgba(0.0, 0.0, 0.0, 0.0));
        let clip_bg_renderable = world.add_component(RenderableComponent::square());
        let clip = world.add_component_boxed_named(
            crate::engine::ecs::system::clipping_system::OWNED_LAYOUT_STENCIL_CLIP_LABEL,
            Box::new(StencilClipComponent::new()),
        );

        let content_t =
            world.add_component_boxed_named("content", Box::new(TransformComponent::new()));
        let content_color = world.add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));
        let content_renderable = world.add_component(RenderableComponent::square());

        let _ = world.add_child(root, clip_scope);
        let _ = world.add_child(clip_scope, clip_bg);
        let _ = world.add_child(clip_bg, clip_bg_color);
        let _ = world.add_child(clip_bg_color, clip_bg_renderable);
        let _ = world.add_child(clip_scope, clip);

        let _ = world.add_child(clip_scope, content_t);
        let _ = world.add_child(content_t, content_color);
        let _ = world.add_child(content_color, content_renderable);

        world.init_component_tree(root, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);

        assert!(
            world
                .get_component_by_id_as::<RenderableComponent>(clip_bg_renderable)
                .and_then(|r| r.get_handle())
                .is_none()
        );
        assert!(
            world
                .get_component_by_id_as::<RenderableComponent>(content_renderable)
                .and_then(|r| r.get_handle())
                .is_none()
        );

        systems.prepare_render(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut uploader,
            &mut queue,
        );

        let clip_handle = world
            .get_component_by_id_as::<RenderableComponent>(clip_bg_renderable)
            .and_then(|r| r.get_handle())
            .expect("clip renderable handle");
        let content_handle = world
            .get_component_by_id_as::<RenderableComponent>(content_renderable)
            .and_then(|r| r.get_handle())
            .expect("content renderable handle");

        let clip_instance = visuals.instance(clip_handle).expect("clip visual instance");
        let content_instance = visuals
            .instance(content_handle)
            .expect("content visual instance");

        assert!(clip_instance.is_stencil_clip);
        assert_eq!(clip_instance.stencil_ref, 0);
        assert!(!content_instance.is_stencil_clip);
        assert_eq!(content_instance.stencil_ref, 1);
    }

    #[test]
    fn mirror_tick_updates_existing_visual_material_same_frame() {
        let mut world = World::default();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();

        let root = world.add_component(TransformComponent::new());
        let renderable = world.add_component(RenderableComponent::square());
        let mirror = world.add_component(MirrorComponent::default());
        let _ = world.add_child(root, renderable);
        let _ = world.add_child(renderable, mirror);

        let handle = register_test_instance(&mut visuals, renderable);
        world
            .get_component_by_id_as_mut::<RenderableComponent>(renderable)
            .expect("renderable")
            .handle = Some(handle);

        systems
            .mirror
            .tick(&mut world, &mut visuals, &Default::default(), 0.0);

        let renderable_component = world
            .get_component_by_id_as::<RenderableComponent>(renderable)
            .expect("renderable component");
        assert_eq!(
            renderable_component.renderable.material,
            MaterialHandle::MIRROR
        );
        assert_eq!(
            visuals
                .instance(handle)
                .expect("visual instance")
                .renderable
                .material,
            MaterialHandle::MIRROR
        );
    }

    #[test]
    fn mirror_texture_registration_attaches_runtime_texture_for_new_child() {
        let mut world = World::default();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let mut uploader = TestUploader::default();
        configure_test_window_camera(&mut visuals);

        let root = world.add_component(TransformComponent::new());
        let renderable = world.add_component(RenderableComponent::square());
        let mirror = world.add_component(MirrorComponent::default());
        let _ = world.add_child(root, renderable);
        let _ = world.add_child(renderable, mirror);

        let handle = register_test_instance(&mut visuals, renderable);
        world
            .get_component_by_id_as_mut::<RenderableComponent>(renderable)
            .expect("renderable")
            .handle = Some(handle);

        systems
            .mirror
            .tick(&mut world, &mut visuals, &Default::default(), 0.0);
        let registrations = systems.mirror.take_pending_texture_registrations();
        assert_eq!(registrations.len(), 1);
        for component in registrations {
            systems.register_texture(&mut world, &mut visuals, component);
        }

        systems
            .render_to_texture
            .flush_pending(&mut visuals, &mut uploader);
        systems
            .texture
            .flush_pending(&mut world, &mut visuals, &mut uploader);

        let mirror_key = visuals
            .mirrors()
            .first()
            .expect("mirror registration")
            .captures
            .iter()
            .find(|capture| {
                capture.family
                    == crate::engine::graphics::visual_world::MirrorViewerFamily::Monoscopic
                    && capture.view_index == 0
            })
            .expect("monoscopic mirror capture")
            .target_key
            .clone();
        let runtime_handle = visuals
            .runtime_texture_handle(&mirror_key)
            .expect("runtime texture handle");
        let visual_instance = visuals.instance(handle).expect("visual instance");
        assert_eq!(visual_instance.texture, Some(runtime_handle));
        assert!(
            systems
                .render_to_texture
                .producer_requests()
                .any(|request| request.selector == mirror_key)
        );
    }

    #[test]
    fn mirror_texture_reregistration_overwrites_existing_render_image_attachment() {
        let mut world = World::default();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let mut uploader = TestUploader::default();
        configure_test_window_camera(&mut visuals);

        let root = world.add_component(TransformComponent::new());
        let renderable = world.add_component(RenderableComponent::square());
        let mirror = world.add_component(MirrorComponent::default());
        let texture = world.add_component(TextureComponent::render_image(
            "capture.mirror.stale.mono.0.color",
        ));
        let _ = world.add_child(root, renderable);
        let _ = world.add_child(renderable, mirror);
        let _ = world.add_child(renderable, texture);

        let handle = register_test_instance(&mut visuals, renderable);
        world
            .get_component_by_id_as_mut::<RenderableComponent>(renderable)
            .expect("renderable")
            .handle = Some(handle);

        systems.register_texture(&mut world, &mut visuals, texture);
        systems
            .render_to_texture
            .flush_pending(&mut visuals, &mut uploader);
        systems
            .texture
            .flush_pending(&mut world, &mut visuals, &mut uploader);
        let stale_handle = visuals.instance(handle).expect("instance").texture;

        systems
            .mirror
            .tick(&mut world, &mut visuals, &Default::default(), 0.0);
        for component in systems.mirror.take_pending_texture_registrations() {
            systems.register_texture(&mut world, &mut visuals, component);
        }
        systems
            .render_to_texture
            .flush_pending(&mut visuals, &mut uploader);
        systems
            .texture
            .flush_pending(&mut world, &mut visuals, &mut uploader);

        let mirror_key = visuals
            .mirrors()
            .first()
            .expect("mirror registration")
            .captures
            .iter()
            .find(|capture| {
                capture.family
                    == crate::engine::graphics::visual_world::MirrorViewerFamily::Monoscopic
                    && capture.view_index == 0
            })
            .expect("monoscopic mirror capture")
            .target_key
            .clone();
        let runtime_handle = visuals
            .runtime_texture_handle(&mirror_key)
            .expect("runtime texture handle");
        let visual_instance = visuals.instance(handle).expect("visual instance");
        assert_eq!(visual_instance.texture, Some(runtime_handle));
        assert_ne!(visual_instance.texture, stale_handle);
        assert_eq!(
            world
                .get_component_by_id_as::<TextureComponent>(texture)
                .expect("texture component")
                .render_image
                .as_deref(),
            Some(mirror_key.as_str())
        );
    }

    #[test]
    fn mirror_registers_capture_family_for_window_and_xr_cameras() {
        let mut world = World::default();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();

        let proj = crate::engine::ecs::system::camera_system::Camera3D::perspective_rh_zo(
            60.0f32.to_radians(),
            1.0,
            0.1,
            100.0,
        );
        configure_test_window_camera(&mut visuals);
        let xr_view = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, -1.0, 1.0],
        ];
        let mut xr_transform = crate::engine::graphics::primitives::Transform::default();
        xr_transform.translation = [0.0, 0.0, 1.0];
        xr_transform.recompute_model();
        xr_transform.matrix_world = xr_transform.model;
        visuals.set_xr_camera(vec![
            crate::engine::graphics::CameraData {
                view: xr_view,
                proj,
                transform: xr_transform,
            },
            crate::engine::graphics::CameraData {
                view: xr_view,
                proj,
                transform: xr_transform,
            },
        ]);

        let root = world.add_component(TransformComponent::new());
        let renderable = world.add_component(RenderableComponent::square());
        let mirror = world.add_component(MirrorComponent::default());
        let _ = world.add_child(root, renderable);
        let _ = world.add_child(renderable, mirror);

        let handle = register_test_instance(&mut visuals, renderable);
        world
            .get_component_by_id_as_mut::<RenderableComponent>(renderable)
            .expect("renderable")
            .handle = Some(handle);

        systems
            .mirror
            .tick(&mut world, &mut visuals, &Default::default(), 0.0);

        let mirror = visuals.mirrors().first().expect("mirror registration");
        assert_eq!(mirror.captures.len(), 3);
        assert!(
            mirror.captures.iter().any(|capture| {
                capture.family
                    == crate::engine::graphics::visual_world::MirrorViewerFamily::Monoscopic
                    && capture.view_index == 0
            }),
            "expected monoscopic capture"
        );
        assert!(
            mirror.captures.iter().any(|capture| {
                capture.family
                    == crate::engine::graphics::visual_world::MirrorViewerFamily::Stereoscopic
                    && capture.view_index == 0
            }),
            "expected left stereo capture"
        );
        assert!(
            mirror.captures.iter().any(|capture| {
                capture.family
                    == crate::engine::graphics::visual_world::MirrorViewerFamily::Stereoscopic
                    && capture.view_index == 1
            }),
            "expected right stereo capture"
        );
    }

    #[test]
    fn mirror_allocates_runtime_handles_for_stereo_capture_selectors() {
        let mut world = World::default();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let mut uploader = TestUploader::default();

        let proj = crate::engine::ecs::system::camera_system::Camera3D::perspective_rh_zo(
            60.0f32.to_radians(),
            1.0,
            0.1,
            100.0,
        );
        configure_test_window_camera(&mut visuals);
        let xr_view = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, -1.0, 1.0],
        ];
        let mut xr_transform = crate::engine::graphics::primitives::Transform::default();
        xr_transform.translation = [0.0, 0.0, 1.0];
        xr_transform.recompute_model();
        xr_transform.matrix_world = xr_transform.model;
        visuals.set_xr_camera(vec![
            crate::engine::graphics::CameraData {
                view: xr_view,
                proj,
                transform: xr_transform,
            },
            crate::engine::graphics::CameraData {
                view: xr_view,
                proj,
                transform: xr_transform,
            },
        ]);

        let root = world.add_component(TransformComponent::new());
        let renderable = world.add_component(RenderableComponent::square());
        let mirror = world.add_component(MirrorComponent::default());
        let _ = world.add_child(root, renderable);
        let _ = world.add_child(renderable, mirror);

        let handle = register_test_instance(&mut visuals, renderable);
        world
            .get_component_by_id_as_mut::<RenderableComponent>(renderable)
            .expect("renderable")
            .handle = Some(handle);

        systems
            .mirror
            .tick(&mut world, &mut visuals, &Default::default(), 0.0);
        for component in systems.mirror.take_pending_texture_registrations() {
            systems.register_texture(&mut world, &mut visuals, component);
        }
        systems
            .render_to_texture
            .flush_pending(&mut visuals, &mut uploader);

        let mirror = visuals.mirrors().first().expect("mirror registration");
        for capture in &mirror.captures {
            assert!(
                visuals
                    .runtime_texture_handle(&capture.target_key)
                    .is_some(),
                "expected runtime texture handle for {}",
                capture.target_key
            );
        }
    }
}

impl SystemWorld {
    pub(crate) fn remove_subtree_immediate(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        root: ComponentId,
    ) {
        use crate::engine::ecs::component::{
            CollisionComponent, ControllerXRComponent, InputXRComponent, KineticResponseComponent,
            PointerComponent, RayCastComponent, RenderableComponent, SignalRouteUpwardComponent,
            StencilClipComponent, TransformComponent,
        };

        // Best-effort: remove system state for known component types before deleting.
        let mut stack = vec![root];
        let mut nodes = Vec::new();
        while let Some(n) = stack.pop() {
            nodes.push(n);
            for &ch in world.children_of(n) {
                stack.push(ch);
            }
        }

        for n in nodes.iter().copied().rev() {
            if world
                .get_component_by_id_as::<SignalRouteUpwardComponent>(n)
                .is_some()
            {
                self.rx.remove_pipelines_from_operator(n);
            }
            if world
                .get_component_by_id_as::<StencilClipComponent>(n)
                .is_some()
            {
                self.clipping
                    .unregister_stencil_clip_for_subtree_node(world, visuals, n);
            }
            if world
                .get_component_by_id_as::<RenderableComponent>(n)
                .is_some()
            {
                self.remove_renderable(world, visuals, n);
            }
            if world
                .get_component_by_id_as::<CollisionComponent>(n)
                .is_some()
            {
                self.remove_collision(world, visuals, n);
            }
            if world
                .get_component_by_id_as::<KineticResponseComponent>(n)
                .is_some()
            {
                self.remove_kinetic_response(world, visuals, n);
            }
            if world
                .get_component_by_id_as::<PointerComponent>(n)
                .is_some()
            {
                self.pointer.remove_pointer(n);
            }
            if world
                .get_component_by_id_as::<RayCastComponent>(n)
                .is_some()
            {
                self.remove_raycast(world, visuals, n);
            }
            if world
                .get_component_by_id_as::<InputXRComponent>(n)
                .is_some()
            {
                self.remove_input_xr(world, visuals, n);
            }
            if world
                .get_component_by_id_as::<ControllerXRComponent>(n)
                .is_some()
            {
                self.remove_controller_xr(world, visuals, n);
            }
            if world
                .get_component_by_id_as::<TransformComponent>(n)
                .is_some()
            {
                self.transition.cancel_transform_transitions(n);
                self.remove_transform(world, visuals, n);
            }
        }

        let _ = world.remove_component_subtree(root);

        // Component lifecycle: remove any scoped handlers rooted in the deleted subtree.
        // Global handlers are unaffected.
        let _ = self.rx.remove_all_scoped_handlers_for_scopes(nodes.clone());
        self.text_input.clear_focus_if_removed(&nodes);
    }

    pub fn new() -> Self {
        let mut systems = Self::default();
        systems.grid.install_handlers(&mut systems.rx);
        let asset_dir = Path::new("assets/components/");
        if let Err(error) = systems.asset_system.scan_assets_dir(asset_dir) {
            eprintln!("[SystemWorld] failed to scan assets dir: {error}");
        }
        systems
    }

    pub fn queue_repl_command(&mut self, command: String) {
        // Avoid unbounded growth if something spams this.
        if self.repl_command_queue.len() >= 128 {
            self.repl_command_queue.drain(0..64);
        }
        self.repl_command_queue.push(command);
    }

    pub fn take_repl_commands(&mut self) -> Vec<String> {
        std::mem::take(&mut self.repl_command_queue)
    }

    /// Execute pending signals up to `max_signals`.
    ///
    /// Semantics:
    /// - Events are dispatched to handlers first.
    /// - Intents are then executed.
    /// - Intents emitted by event handlers run later in the same tick.
    /// - Events emitted by event handlers are deferred to the next tick.
    pub fn process_signals(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        render_assets: &mut crate::engine::graphics::RenderAssets,
        queue: &mut crate::engine::ecs::CommandQueue,
        max_signals: usize,
    ) -> usize {
        let mut processed = 0usize;

        let mut intent_executor = crate::engine::ecs::rx::RxIntentExecutor::default();
        let mut mutation_executor = crate::engine::ecs::rx::RxMutationExecutor::default();
        let mut pipeline_processor = crate::engine::ecs::rx::SignalPipelineProcessor::default();

        // Drain locally-queued signals into `RxWorld` before we start.
        let _ = queue.drain_into_rx(&mut self.rx);

        // Timed holding-pen: promote any pending intents that are now due.
        let now_beat = self.clock.beat_now();
        let _ = self.rx.promote_due_intents(now_beat);

        loop {
            if processed >= max_signals {
                break;
            }

            // 1) Dispatch all ready events.
            let events = self.rx.drain_ready_events();
            if !events.is_empty() {
                let mut leftover = Vec::new();
                for env in events {
                    if processed >= max_signals {
                        leftover.push(env);
                        continue;
                    }
                    processed += 1;
                    self.rx.dispatch_event_handlers(world, &env);
                }
                if !leftover.is_empty() {
                    self.rx.requeue_ready_events(leftover);
                    return processed;
                }
            }

            // 2) Promote any newly-due timed intents.
            let _ = self.rx.promote_due_intents(now_beat);

            // 3) Execute all ready intents.
            let intents = self.rx.drain_ready_intents();
            if !intents.is_empty() {
                let mut leftover = Vec::new();
                for env in intents {
                    if processed >= max_signals {
                        leftover.push(env);
                        continue;
                    }
                    processed += 1;

                    let env = pipeline_processor.process_intent(world, &self.rx, env);

                    let Some(intent) = env.intent.as_ref() else {
                        continue;
                    };

                    if crate::engine::ecs::rx::RxIntentExecutor::handles_value(&intent.value) {
                        // Emit follow-up intent work directly into the per-frame queue to avoid
                        // borrowing `self.rx` while also mutably borrowing `self`.
                        intent_executor.execute(world, render_assets, queue, &env);
                    } else {
                        mutation_executor.execute(self, world, visuals, render_assets, queue, &env);
                    }
                }
                if !leftover.is_empty() {
                    self.rx.requeue_ready_intents(leftover);
                    return processed;
                }
            }

            // If the executor queued more signals during processing, drain them and continue.
            if queue.drain_into_rx(&mut self.rx) > 0 {
                continue;
            }

            // If timed intents became due, keep going.
            if self.rx.promote_due_intents(now_beat) > 0 {
                continue;
            }

            // If new ready work was produced (unlikely without queue drain), keep draining.
            if self.rx.has_ready_events() || self.rx.has_ready_intents() {
                continue;
            }

            break;
        }

        processed
    }

    #[cfg(any())]
    fn execute_intent_signal(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        env: &crate::engine::ecs::Signal,
    ) {
        use crate::engine::ecs::component::{
            RenderableComponent, TextComponent, TransformComponent,
        };
        use crate::engine::ecs::system::audio_system::AudioOp;
        use crate::engine::ecs::{EventSignal, IntentValue};
        use crate::engine::graphics::primitives::Transform;

        fn collect_text_targets(world: &World, target: ComponentId, out: &mut Vec<ComponentId>) {
            if world
                .get_component_by_id_as::<TextComponent>(target)
                .is_some()
            {
                out.push(target);
                return;
            }

            let mut stack = vec![target];
            while let Some(node) = stack.pop() {
                for &ch in world.children_of(node) {
                    stack.push(ch);
                }

                if world
                    .get_component_by_id_as::<TextComponent>(node)
                    .is_some()
                {
                    out.push(node);
                }
            }
        }

        fn apply_set_text_to_component(
            this: &mut SystemWorld,
            world: &mut World,
            visuals: &mut VisualWorld,
            emit: &mut dyn crate::engine::ecs::SignalEmitter,
            component: ComponentId,
            text: &String,
        ) {
            // Update text payload and force a rebuild.
            if let Some(tc) = world.get_component_by_id_as_mut::<TextComponent>(component) {
                tc.text = text.clone();
                tc.mark_unbuilt();

                // Best-effort: delete glyph transform children (keep style components).
                let children: Vec<ComponentId> = world.children_of(component).to_vec();
                for ch in children {
                    if world
                        .get_component_by_id_as::<TransformComponent>(ch)
                        .is_none()
                    {
                        continue;
                    }

                    let has_renderable_child = world.children_of(ch).iter().any(|&gch| {
                        world
                            .get_component_by_id_as::<RenderableComponent>(gch)
                            .is_some()
                    });
                    if has_renderable_child {
                        // This is very likely a glyph root.
                        this.remove_subtree_immediate(world, visuals, ch);
                    }
                }
            }

            this.register_text(world, visuals, component, emit);
        }

        let Some(intent) = env.intent.as_ref() else {
            return;
        };

        match &intent.value {
            IntentValue::RegisterRenderable { component } => {
                self.register_renderable(world, visuals, *component);
            }
            IntentValue::RemoveRenderable { component } => {
                self.remove_renderable(world, visuals, *component);
            }

            IntentValue::RegisterTransform { component } => {
                self.transform_changed(world, visuals, *component);
            }
            IntentValue::UpdateTransform {
                component,
                translation,
                rotation_quat_xyzw,
                scale,
            } => {
                let mut t = Transform::default();
                t.translation = *translation;
                t.rotation = *rotation_quat_xyzw;
                t.scale = *scale;
                t.recompute_model();
                self.update_transform(world, visuals, *component, t);
            }
            IntentValue::RemoveTransform { component } => {
                self.remove_transform(world, visuals, *component);
            }

            IntentValue::RegisterCamera3d { component } => {
                self.register_camera(world, visuals, *component);
            }
            IntentValue::RegisterCamera2d { component } => {
                self.register_camera2d(world, visuals, *component);
            }
            IntentValue::MakeActiveCamera { component } => {
                self.make_active_camera(world, visuals, *component);
            }

            IntentValue::RegisterInput { component } => {
                self.register_input(*component);
            }
            IntentValue::RegisterUv { component } => {
                self.register_uv(world, visuals, *component);
            }

            IntentValue::RegisterLight { component } => {
                self.register_light(world, visuals, *component);
            }
            IntentValue::RegisterColor { component } => {
                self.register_color(world, visuals, *component);
            }
            IntentValue::RegisterOpacity { component } => {
                self.register_opacity(world, visuals, *component);
            }
            IntentValue::RegisterTransparentCutout { component } => {
                self.register_transparent_cutout(world, visuals, *component);
            }
            IntentValue::RegisterBackgroundColor { component } => {
                self.register_background_color(world, visuals, *component);
            }
            IntentValue::RegisterAmbientLight { component } => {
                self.register_ambient_light(world, visuals, *component);
            }
            IntentValue::RegisterEmissive { component } => {
                self.register_emissive(world, visuals, *component);
            }
            IntentValue::RegisterLightQuantization { component } => {
                self.register_light_quantization(world, visuals, *component);
            }

            IntentValue::RegisterTexture { component } => {
                self.register_texture(world, visuals, *component);
            }
            IntentValue::RegisterTextureFiltering { component } => {
                self.register_texture_filtering(world, visuals, *component);
            }

            IntentValue::RegisterText { component } => {
                self.register_text(world, visuals, *component, emit);
            }
            IntentValue::SetText { target, text } => {
                let mut text_cids = Vec::new();
                for &t in target.iter() {
                    collect_text_targets(world, t, &mut text_cids);
                }
                text_cids.sort();
                text_cids.dedup();

                for text_cid in text_cids {
                    apply_set_text_to_component(self, world, visuals, emit, text_cid, text);
                }
            }

            IntentValue::RegisterCollision { component } => {
                self.register_collision(world, visuals, *component);
            }
            IntentValue::RemoveCollision { component } => {
                self.remove_collision(world, visuals, *component);
            }
            IntentValue::RegisterKineticResponse { component } => {
                self.register_kinetic_response(world, visuals, *component);
            }
            IntentValue::RemoveKineticResponse { component } => {
                self.remove_kinetic_response(world, visuals, *component);
            }

            IntentValue::RemoveSubtree { target } => {
                let mut roots: Vec<ComponentId> = target.iter().copied().collect();
                roots.sort();
                roots.dedup();
                for root in roots {
                    // Best-effort: if the root is still attached, detach it first and publish
                    // a topology fact before deletion.
                    if let Some(old_parent) = world.parent_of(root) {
                        world.detach_from_parent(root);
                        emit.push_event(
                            root,
                            EventSignal::ParentChanged {
                                child: root,
                                old_parent: Some(old_parent),
                                new_parent: None,
                            },
                        );
                    }
                    self.remove_subtree_immediate(world, visuals, root);
                }
            }

            IntentValue::RegisterXr { component } => {
                self.register_xr(world, visuals, *component);
            }
            IntentValue::RegisterInputXr { component } => {
                self.register_input_xr(world, visuals, *component);
            }
            IntentValue::RegisterControllerXr { component } => {
                self.register_controller_xr(world, visuals, *component);
            }
            IntentValue::RegisterInputXrGamepad { component } => {
                self.register_input_xr_gamepad(world, visuals, *component);
            }
            IntentValue::RemoveInputXr { component } => {
                self.remove_input_xr(world, visuals, *component);
            }
            IntentValue::RemoveControllerXr { component } => {
                self.remove_controller_xr(world, visuals, *component);
            }
            IntentValue::RemoveInputXrGamepad { component } => {
                self.remove_input_xr_gamepad(world, visuals, *component);
            }

            IntentValue::RegisterRaycast { component } => {
                self.register_raycast(world, visuals, *component);
            }
            IntentValue::RemoveRaycast { component } => {
                self.remove_raycast(world, visuals, *component);
            }

            IntentValue::RegisterAnimation { component } => {
                self.register_animation(world, visuals, *component);
            }
            IntentValue::RegisterKeyframe { component } => {
                self.register_keyframe(world, visuals, *component);
            }

            IntentValue::RegisterAudioOutput { component } => {
                self.register_audio_output(world, visuals, *component);
            }
            IntentValue::AudioGraphDirtyImmediate { component } => {
                self.audio_graph_dirty(world, visuals, *component);
            }
            IntentValue::RegisterAudioOscillator { component } => {
                self.register_audio_oscillator(world, visuals, *component);
            }
            IntentValue::RegisterAudioBufferSize { component } => {
                self.register_audio_buffer_size(world, visuals, *component);
            }

            IntentValue::RegisterClock { component } => {
                self.register_clock(world, visuals, *component);
            }

            IntentValue::RegisterTransformGizmo { component } => {
                self.register_transform_gizmo(world, visuals, *component, emit);
            }

            IntentValue::ScheduleAudioOp {
                component,
                beat,
                op,
            } => {
                self.audio.schedule_audio_op(*component, *beat, *op);
            }
            IntentValue::ScheduleAudioGraphSwap { component, beat } => {
                self.audio.schedule_graph_swap(&*world, *component, *beat);
            }
            IntentValue::ScheduleAudioPitchSetHz {
                component,
                beat,
                frequency_hz,
            } => {
                self.audio
                    .schedule_audio_op(*component, *beat, AudioOp::SetHz(*frequency_hz));
            }
            IntentValue::ScheduleAudioOscillatorEnabled {
                component,
                beat,
                enabled,
            } => {
                self.audio
                    .schedule_audio_op(*component, *beat, AudioOp::SetEnabled(*enabled));
            }
            IntentValue::ScheduleAudioGainSet {
                component,
                beat,
                gain,
            } => {
                self.audio
                    .schedule_audio_op(*component, *beat, AudioOp::SetGain(*gain));
            }

            // Not executed by the default executor.
            _ => {}
        }
    }

    /// Register a TransformGizmoComponent by spawning its visual subtree.
    ///
    /// Contract: TransformGizmoComponent is expected to be attached under a TransformComponent.
    pub fn register_transform_gizmo(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
    ) {
        self.transform_gizmo
            .install_scoped_handlers_for_gizmo(&mut self.rx, component);
        self.transform_gizmo
            .register_transform_gizmo(world, component, emit);
    }

    pub fn register_normal_vis(&mut self, world: &World, component: ComponentId) {
        self.renderable.register_normal_vis(world, component);
    }

    pub fn register_text_input(
        &mut self,
        world: &mut World,
        component: ComponentId,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
    ) {
        self.text_input.register_text_input(world, emit, component);
    }

    pub fn register_editor(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        render_assets: &mut crate::engine::graphics::RenderAssets,
        component: ComponentId,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
    ) {
        let panel_cfg = world
            .get_component_by_id_as::<crate::engine::ecs::component::EditorComponent>(component)
            .map(|ed| (ed.spawn_panels, ed.world_panel_pos, ed.inspector_panel_pos));

        let Some((spawn_panels, world_panel_pos, inspector_panel_pos)) = panel_cfg else {
            return;
        };

        let editor_context_state = self.editor_context.shared_state();
        self.transform_gizmo
            .set_editor_context_state(editor_context_state.clone());

        self.editor
            .materialize_editor_raycastables(world, emit, component);

        if spawn_panels {
            self.editor_inspector.setup_panels_for_editor(
                &mut self.rx,
                world,
                render_assets,
                emit,
                component,
                world_panel_pos,
                inspector_panel_pos,
                editor_context_state.clone(),
                &self.asset_system,
            );
            let Some(panel_query_root) = world.all_components().find(|&component_id| {
                world.parent_of(component_id).is_none()
                    && world.component_label(component_id) == Some("editor_runtime_ui_root")
            }) else {
                return;
            };
            self.editor.install_scoped_handlers_for_editor(
                &mut self.rx,
                component,
                panel_query_root,
                editor_context_state.clone(),
            );
            self.cursor_3d.install_scoped_handlers_for_editor(
                &mut self.rx,
                component,
                panel_query_root,
                editor_context_state.clone(),
            );
            self.editor_context.install_scoped_handlers_for_editor(
                &mut self.rx,
                world,
                component,
                panel_query_root,
            );
            self.editor_paint.install_scoped_handlers_for_editor(
                &mut self.rx,
                world,
                self.grid.clone(),
                component,
                panel_query_root,
                editor_context_state,
                self.asset_system.paint_templates(),
            );
        } else {
            self.editor.install_scoped_handlers_for_editor(
                &mut self.rx,
                component,
                component,
                editor_context_state,
            );
            self.cursor_3d.install_scoped_handlers_for_editor(
                &mut self.rx,
                component,
                component,
                self.editor_context.shared_state(),
            );
        }
    }

    pub fn register_scrolling(
        &mut self,
        world: &mut World,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        component: ComponentId,
    ) {
        self.scrolling
            .deferred_register(&mut self.rx, world, emit, component);
    }

    pub fn register_router(
        &mut self,
        world: &mut World,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        component: ComponentId,
    ) {
        self.router
            .register_router(&mut self.rx, world, emit, component);
    }

    /// Register a RenderableComponent instance with the RenderableSystem.
    pub fn register_renderable(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.renderable
            .register_renderable(world, visuals, component);

        // Cache the renderable's local-space AABB as a child BoundsComponent
        // so layout (and future CPU culling) can read mesh extents without
        // touching the BVH or recomputing from mesh handles each time.
        attach_bounds_for_renderable(world, component);

        // Keep BVH in sync (defer actual build/refit to CommandQueue flush).
        if BvhSystem::renderable_is_raycastable(world, component) {
            self.bvh.queue_renderable_added(component);
        }

        // Keep RayCastSystem's eligibility index in sync for brute-force fallback.
        self.raycast.notify_renderable_added(&*world, component);

        self.clipping.register_renderable(world, visuals, component);
    }

    /// Register a StencilClipComponent: find the sibling RenderableComponent in the same
    /// TC subtree, count ancestor clip depth, and mark the VisualInstance as a clip source.
    pub fn register_stencil_clip(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.clipping
            .register_stencil_clip(world, visuals, component);
    }

    /// Unregister a StencilClipComponent: clear `is_stencil_clip` on the associated VisualInstance.
    pub fn unregister_stencil_clip(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.clipping
            .unregister_stencil_clip(world, visuals, component);
    }

    /// Remove a RenderableComponent instance from the RenderableSystem (and BVH).
    pub fn remove_renderable(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.renderable.remove_renderable(world, visuals, component);
        self.bvh.queue_renderable_removed(component);
        self.raycast.notify_renderable_removed(component);
    }

    /// Register a UVComponent and apply it to its ancestor RenderableComponent.
    pub fn register_uv(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.renderable.register_uv(world, visuals, component);
    }

    /// Register a ColorComponent and apply it to its ancestor RenderableComponent.
    pub fn register_color(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.renderable.register_color(world, visuals, component);
    }

    /// Register an OpacityComponent and apply it to its ancestor RenderableComponent.
    pub fn register_opacity(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.renderable.register_opacity(world, visuals, component);
    }

    /// Register a TransparentCutoutComponent and apply it to its ancestor RenderableComponent.
    pub fn register_transparent_cutout(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.renderable
            .register_transparent_cutout(world, visuals, component);
    }

    /// Register a BackgroundColorComponent and apply it to VisualWorld.
    pub fn register_background_color(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.renderable
            .register_background_color(world, visuals, component);
    }

    /// Register a RendererSettingsComponent and apply it to VisualWorld.
    pub fn register_renderer_settings(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.renderable
            .register_renderer_settings(world, visuals, component);
    }

    /// Register a RenderGraphComponent and apply it to VisualWorld.
    pub fn register_render_graph(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        let Some(render_graph) = world
            .get_component_by_id_as::<crate::engine::ecs::component::RenderGraphComponent>(
                component,
            )
        else {
            return;
        };

        let mut config = crate::engine::graphics::PostProcessingConfig {
            enabled: render_graph.enabled,
            ..Default::default()
        };

        for &child in world.children_of(component) {
            if world
                .get_component_by_id_as::<crate::engine::ecs::component::EmissivePassComponent>(
                    child,
                )
                .is_some()
            {
                let mut emissive_pass = crate::engine::graphics::EmissivePassConfig::default();

                for &grandchild in world.children_of(child) {
                    if let Some(texture) = world
                        .get_component_by_id_as::<crate::engine::ecs::component::TextureComponent>(
                            grandchild,
                        )
                    {
                        if emissive_pass.output_texture.is_none() {
                            emissive_pass.output_texture =
                                Some(texture.render_image.clone().unwrap_or_else(|| {
                                    "render_graph.emissive_pass.output".to_string()
                                }));
                        }
                    }

                    if let Some(blur_pass) = world
                        .get_component_by_id_as::<crate::engine::ecs::component::BlurPassComponent>(
                            grandchild,
                        )
                    {
                        if blur_pass.enabled {
                            config.blur_pass = Some(crate::engine::graphics::BlurPassConfig {
                                enabled: true,
                                radius_ndc: blur_pass.radius_ndc,
                                half_res: blur_pass.half_res,
                            });
                        }
                    }
                }

                config.emissive_pass = Some(emissive_pass);
                continue;
            }

            let Some(bloom) = world
                .get_component_by_id_as::<crate::engine::ecs::component::BloomComponent>(child)
            else {
                continue;
            };

            if bloom.enabled {
                config.bloom = Some(crate::engine::graphics::BloomConfig {
                    intensity: bloom.intensity,
                    radius_ndc: bloom.radius_ndc,
                    emissive_scale: bloom.emissive_scale,
                    half_res: bloom.half_res,
                    output_texture: bloom.output_texture.clone(),
                    ..Default::default()
                });
            }

            for &grandchild in world.children_of(child) {
                let Some(texture) = world
                    .get_component_by_id_as::<crate::engine::ecs::component::TextureComponent>(
                        grandchild,
                    )
                else {
                    continue;
                };

                if let Some(bloom_cfg) = config.bloom.as_mut() {
                    if bloom_cfg.output_texture.is_none() {
                        bloom_cfg.output_texture = Some(
                            texture
                                .render_image
                                .clone()
                                .unwrap_or_else(|| "render_graph.bloom.blur".to_string()),
                        );
                    }
                }
            }
        }

        visuals.set_post_processing(config);
    }

    /// Register an AmbientLightComponent and apply it to VisualWorld.
    pub fn register_ambient_light(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.light.register_ambient_light(world, visuals, component);
    }

    /// Register a TextureComponent and apply it to its ancestor RenderableComponent.
    pub fn register_texture(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.render_to_texture.register_texture(world, component);
        self.texture.register_texture(world, visuals, component);
    }

    /// Register a TextureFilteringComponent and apply it to its ancestor RenderableComponent.
    pub fn register_texture_filtering(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.texture
            .register_texture_filtering(world, visuals, component);
    }

    /// Register a TextComponent and expand it into per-glyph components.
    pub fn register_text(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
    ) {
        // Allow text to react to late-attached style nodes (e.g. ColorComponent).
        self.rx.add_handler(
            SignalKind::ParentChanged,
            component,
            TextSystem::on_parent_changed,
        );

        let _spawned = self.text.register_text(world, visuals, component);

        // Initialize any newly spawned glyph/background subtrees.
        // This is idempotent: nodes that were already initialized are skipped.
        world.init_component_tree(component, emit);
    }

    /// Register an EmissiveComponent and apply it to its ancestor RenderableComponent.
    pub fn register_emissive(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.renderable.register_emissive(world, visuals, component);
    }

    /// Register a LightQuantizationComponent and apply it to its ancestor RenderableComponent.
    pub fn register_light_quantization(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.renderable
            .register_light_quantization(world, visuals, component);
    }

    /// Register a CollisionComponent instance with the CollisionSystem.
    pub fn register_collision(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.collision.register_collision(world, visuals, component);
    }

    /// Register a KineticResponseComponent instance with the KineticResponseSystem.
    pub fn register_kinetic_response(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.kinetic_response
            .register_kinetic_response(world, component);
    }

    /// Remove a KineticResponseComponent instance from the KineticResponseSystem.
    pub fn remove_kinetic_response(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.kinetic_response.remove_kinetic_response(component);
    }

    /// Remove a CollisionComponent instance from the CollisionSystem.
    pub fn remove_collision(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.collision.remove_collision(world, visuals, component);
    }

    /// Register an XrComponent (initializes the XR runtime if enabled).
    pub fn register_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.xr.register_xr(world, visuals, component);
    }

    /// Register an InputXRComponent (tracks the headset/root XR pose and drives a transform).
    pub fn register_input_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.xr.register_input_xr(world, visuals, component);
    }

    /// Register a ControllerXRComponent (tracks an XR controller pose and drives a transform).
    pub fn register_controller_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.xr.register_controller_xr(world, visuals, component);
    }

    pub fn register_input_xr_gamepad(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.input_xr_gamepad.register_input_xr_gamepad(component);
    }

    /// Remove a ControllerXRComponent from XR runtime tracking.
    pub fn remove_controller_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.xr.remove_controller_xr(world, visuals, component);
    }

    /// Remove an InputXRComponent from XR runtime tracking.
    pub fn remove_input_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.xr.remove_input_xr(world, visuals, component);
    }

    pub fn remove_input_xr_gamepad(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.input_xr_gamepad.remove_input_xr_gamepad(component);
    }

    /// Register a PointLightComponent instance with the LightSystem.
    pub fn register_light(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.light.register_light(world, visuals, component);
    }

    /// Register a RayCastComponent instance with the RayCastSystem.
    pub fn register_raycast(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.raycast.register_raycast(world, visuals, component);
    }

    /// Register a PointerComponent by ensuring it owns a child RayCastComponent.
    pub fn register_pointer(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
    ) {
        self.pointer.register_pointer(world, component, emit);
    }

    /// Remove a RayCastComponent instance from the RayCastSystem.
    pub fn remove_raycast(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.raycast.remove_raycast(world, visuals, component);
    }

    pub fn register_animation(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.animation.register_animation(world, component);
    }

    pub fn set_animation_state(
        &mut self,
        component: ComponentId,
        state: crate::engine::ecs::component::AnimationState,
    ) {
        self.animation.set_animation_state(component, state);
    }

    pub fn register_keyframe(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.animation.register_keyframe(world, component);
    }

    pub fn register_audio_output(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.audio.register_audio_output(world, component);
    }

    pub fn register_audio_oscillator(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.audio.register_audio_oscillator(world, component);
    }

    pub fn register_audio_clip(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.audio.register_audio_clip(world, component);
    }

    pub fn register_audio_buffer_size(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.audio.register_audio_buffer_size(world, component);
    }

    pub fn audio_graph_dirty(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.audio.mark_audio_graph_dirty(world, component);
    }

    pub fn register_clock(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        if world
            .get_component_by_id_as::<crate::engine::ecs::component::ClockComponent>(component)
            .is_some()
        {
            self.clock.register_clock_component(component);
        }
    }

    /// Prepare render state before issuing a frame.
    ///
    /// This flushes any pending renderables by uploading meshes and inserting GPU-ready
    /// instances into `VisualWorld`.
    pub fn prepare_render(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        render_assets: &mut RenderAssets,
        uploader: &mut dyn RenderUploader,
        queue: &mut crate::engine::ecs::CommandQueue,
    ) {
        // Ensure any imported assets are registered before renderables try to resolve meshes/textures.
        self.gltf
            .flush_imports(render_assets, &mut self.texture, uploader);

        let flushed_renderables =
            self.renderable
                .flush_pending(world, visuals, render_assets, uploader, queue);
        if flushed_renderables {
            self.clipping.resync_after_renderable_flush(world, visuals);
        }

        self.render_to_texture.flush_pending(visuals, uploader);

        // Must run after renderables are flushed so instance handles exist.
        self.texture.flush_pending(world, visuals, uploader);
    }

    /// Called when a TransformComponent changes.
    pub fn transform_changed(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.transform.transform_changed(
            world,
            visuals,
            component,
            &mut self.transform_stream,
            &mut self.camera,
            &mut self.light,
            &mut self.collision,
        );

        // Any transform changes can affect joint world matrices for skinning.
        // Let SkinnedMeshSystem lazily recompute only affected rigs.
        self.skinned_mesh
            .transform_subtree_changed(&*world, component);

        // Transform propagation may move many renderables in the subtree.
        // Queue a BVH refit for that subtree (applied after command flush).
        self.bvh.queue_transform_subtree(world, component);
    }

    fn apply_transform_immediate(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
        transform: crate::engine::graphics::primitives::Transform,
    ) {
        if let Some(transform_comp) = world
            .get_component_by_id_as_mut::<crate::engine::ecs::component::TransformComponent>(
                component,
            )
        {
            transform_comp.transform = transform;
        }
        self.transform_changed(world, visuals, component);
    }

    fn tick_transition_runtime(&mut self, world: &mut World, visuals: &mut VisualWorld) {
        let updates = self
            .transition
            .sample_transform_updates(self.clock.beat_now());
        for update in updates {
            self.apply_transform_immediate(world, visuals, update.component, update.transform);
        }
    }

    /// Update a transform component's transform value and notify systems.
    pub fn update_transform(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
        transform: crate::engine::graphics::primitives::Transform,
    ) {
        let transition = world.children_of(component).iter().find_map(|&child| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::TransitionComponent>(child)
                .copied()
        });

        if let (Some(policy), Some(current)) = (
            transition,
            world
                .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(
                    component,
                )
                .map(|transform_comp| transform_comp.transform),
        ) {
            if self.transition.start_transform_transition(
                component,
                current,
                transform,
                policy,
                self.clock.beat_now(),
            ) {
                return;
            }
        }

        self.apply_transform_immediate(world, visuals, component, transform);
    }

    /// Remove/reset a transform component's transform value and notify systems.
    pub fn remove_transform(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.transition.cancel_transform_transitions(component);
        if let Some(transform_comp) = world
            .get_component_by_id_as_mut::<crate::engine::ecs::component::TransformComponent>(
                component,
            )
        {
            transform_comp.transform = crate::engine::graphics::primitives::Transform::default();
        }
        self.transform_changed(world, visuals, component);
    }

    /// Register a camera component.
    pub fn register_camera(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        let handle = self.camera.register_camera(world, visuals, component);
        // Store the handle in the component
        if let Some(camera_comp) = world
            .get_component_by_id_as_mut::<crate::engine::ecs::component::Camera3DComponent>(
                component,
            )
        {
            camera_comp.handle = Some(handle);
        }
    }

    /// Register a Camera2D component.
    pub fn register_camera2d(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        let handle = self.camera.register_camera2d(world, visuals, component);
        // Store the handle in the component
        if let Some(camera2d_comp) = world
            .get_component_by_id_as_mut::<crate::engine::ecs::component::Camera2DComponent>(
                component,
            )
        {
            camera2d_comp.handle = Some(handle);
        }

        // Apply 2D camera view transform from the parent Transform immediately so the camera is correct
        // on the first frame after registration.
        if let Some(parent) = world.parent_of(component) {
            if world
                .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(parent)
                .is_some()
            {
                self.camera
                    .update_camera_2d_from_parent_transform(&*world, visuals, component, parent);
            }
        }
    }

    /// Register an InputComponent.
    pub fn register_input(&mut self, component: ComponentId) {
        self.input.register_input(component);
    }

    /// Make a camera active by its component ID.
    pub fn make_active_camera(
        &mut self,
        _world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        // XR rig cameras.
        if let Some(camera_xr) = _world
            .get_component_by_id_as::<crate::engine::ecs::component::CameraXRComponent>(component)
        {
            match camera_xr.target {
                crate::engine::graphics::CameraTarget::Xr => {
                    self.camera.set_active_xr_camera(_world, visuals, component);
                }
                crate::engine::graphics::CameraTarget::Window => {
                    // If someone explicitly targets Window with a CameraXRComponent, ignore for now.
                    // (Window camera matrices are driven by Camera2D/Camera3D.)
                }
            }
            return;
        }

        // Try Camera3DComponent first
        if let Some(camera_comp) = _world
            .get_component_by_id_as::<crate::engine::ecs::component::Camera3DComponent>(component)
        {
            if let Some(handle) = camera_comp.handle {
                match camera_comp.target {
                    crate::engine::graphics::CameraTarget::Window => {
                        self.camera.set_active_window_camera(visuals, handle);
                    }
                    crate::engine::graphics::CameraTarget::Xr => {
                        // XR camera matrices are driven by OpenXR each frame.
                        // Keep this as a no-op for now.
                    }
                }
                return;
            }
        }
        // Try Camera2DComponent
        if let Some(camera2d_comp) = _world
            .get_component_by_id_as::<crate::engine::ecs::component::Camera2DComponent>(component)
        {
            if let Some(handle) = camera2d_comp.handle {
                match camera2d_comp.target {
                    crate::engine::graphics::CameraTarget::Window => {
                        self.camera.set_active_window_camera(visuals, handle);
                    }
                    crate::engine::graphics::CameraTarget::Xr => {
                        // XR camera matrices are driven by OpenXR each frame.
                        // Keep this as a no-op for now.
                    }
                }
            }
        }
    }

    // first, tick is called on all systems,
    // process_commands is called after, systems.tick(), to process the commands in the queue

    pub fn tick(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        render_assets: &mut crate::engine::graphics::RenderAssets,
        input: &InputState,
        queue: &mut crate::engine::ecs::CommandQueue,
        dt_sec: f32,
    ) {
        visuals.set_window_frame_dt_sec(dt_sec);

        // Drain-point signal graph setup.
        // Per-frame caches are reset here; global handlers are installed idempotently.
        // (Per-gizmo scoped handlers are installed when the gizmo is registered.)
        self.rx.begin_frame();
        self.gesture.install_handlers(&mut self.rx);
        self.gesture.begin_frame();
        self.text_input.install_handlers(&mut self.rx);
        self.selection.install_handlers(&mut self.rx);

        // Process input first - it may queue commands
        self.input.process_input(world, input, queue, dt_sec);

        // Spawn any GLTF component trees. This may queue component registrations.
        self.gltf
            .tick_with_queue(world, visuals, &mut self.skinned_mesh, queue, dt_sec);

        // Flush queued registrations/transform updates *before* systems that need current
        // world matrices / acceleration structures (e.g. raycasting).
        queue.flush(world, self, visuals, render_assets);

        self.armature_visualization
            .tick_with_queue(world, &self.gltf, visuals, queue, dt_sec);
        queue.flush(world, self, visuals, render_assets);

        // Audio clock takeover: once audio output is active, use it as the ClockDriver.
        if let Some(driver) = self.audio.driver() {
            if self.clock.driver_name() != driver.name() {
                self.clock.set_driver(driver);
            }
        }

        self.clock.tick(world, visuals, input, dt_sec);

        // Provide tempo + transport to the audio thread scheduler.
        // ClockSystem may be using AudioClockDriver, so this keeps both timelines aligned.
        self.audio
            .update_transport_from_clock(self.clock.beat_now(), self.clock.bpm());

        self.animation
            .tick_with_beat(world, self.clock.beat_now(), self.clock.bpm(), &mut self.rx);

        // Execute/dispatch any signals emitted by AnimationSystem before downstream systems run.
        let _ = self.process_signals(world, visuals, render_assets, queue, 100_000);
        queue.flush(world, self, visuals, render_assets);
        self.tick_transition_runtime(world, visuals);

        self.transform_stream.tick(world, visuals, input, dt_sec);

        // Ensure transforms are propagated before any camera systems consume world matrices.
        self.transform.tick(world, visuals, input, dt_sec);

        // Compute joint palettes from cached world transforms.
        self.skinned_mesh.tick(world, visuals, input, dt_sec);

        // Spatial acceleration structure built from latest world transforms.
        self.bvh.tick(world, visuals, input, dt_sec);

        // Collision runs before camera/OpenXR for now; it reads cached world transforms.
        self.collision
            .tick_with_rx(world, visuals, input, dt_sec, &mut self.rx);

        // Default kinematic-vs-static collision response (opt-in via KineticResponseComponent).
        // This may enqueue transform updates; flush them immediately so camera/OpenXR
        // consume resolved transforms this frame.
        self.kinetic_response.tick_with_queue(
            world,
            visuals,
            input,
            dt_sec,
            queue,
            &self.collision,
        );
        queue.flush(world, self, visuals, render_assets);
        self.tick_transition_runtime(world, visuals);

        // Physics may have moved renderables; refit BVH so raycasts see the resolved state.
        self.bvh.tick(world, visuals, input, dt_sec);

        // Update window camera + select active XR camera rig before OpenXR consumes it.
        self.camera.tick(world, visuals, input, dt_sec);
        // OpenXR consumes the latest rig transform + publishes per-eye cameras.
        self.xr
            .tick_with_queue(world, visuals, input, queue, dt_sec);
        // Controller pose updates should be visible to raycasting/gestures this frame.
        queue.flush(world, self, visuals, render_assets);
        self.tick_transition_runtime(world, visuals);
        self.input_xr_gamepad
            .tick_with_queue(world, visuals, &self.xr, queue, dt_sec);
        queue.flush(world, self, visuals, render_assets);
        self.tick_transition_runtime(world, visuals);

        let activations = self
            .pointer
            .build_activations(world, input, self.xr.xr_input_state());

        self.raycast.tick_with_queue(
            world,
            visuals,
            input,
            &mut self.rx,
            &self.bvh,
            &activations,
            &self.pointer,
            dt_sec,
        );

        // Execute/dispatch any signals produced by raycast immediately (e.g. RayIntersected).
        let _ = self.process_signals(world, visuals, render_assets, queue, 100_000);

        // Gestures interpret ray hits + input into drag events.
        self.gesture
            .tick_with_rx(visuals, input, &activations, &self.pointer, &mut self.rx);

        // Execute/dispatch gesture-produced signals immediately (e.g. DragStart/DragMove/DragEnd).
        let _ = self.process_signals(world, visuals, render_assets, queue, 100_000);

        // Gizmos consume drag events and apply transform changes.
        self.transform_gizmo
            .tick_with_queue(world, input, queue, &mut self.rx);

        // Execute/dispatch gizmo-produced signals immediately (if any).
        let _ = self.process_signals(world, visuals, render_assets, queue, 100_000);

        // Bridge buffered platform text input after gesture-driven focus changes have landed.
        self.text_input.tick_with_queue(world, input, queue);
        let _ = self.process_signals(world, visuals, render_assets, queue, 100_000);

        // Apply gizmo transform updates immediately so visuals reflect the drag this frame.
        queue.flush(world, self, visuals, render_assets);
        self.tick_transition_runtime(world, visuals);

        // Avatar body yaw: smoothly rotate body to follow head when yaw diverges.
        // Runs after OpenXR + raycasts + gestures so avatar_driven_t.matrix_world is current.
        self.avatar_body_yaw.tick(world, queue, dt_sec);
        self.avatar_control.tick(world, input, queue, dt_sec);
        // head_pose_body_xz_follow owns model_root XZ translation + neck
        // rest-pin. Runs after AVC init so model_root_id / body_pipeline_id
        // are populated. Currently Step 0 pass-through (see
        // docs/task/avatar-control-simple-humanoid-body-follow.md).
        self.head_pose_body_xz_follow.tick(world, queue, dt_sec);
        self.ik.tick(world, queue, dt_sec);
        queue.flush(world, self, visuals, render_assets);
        self.tick_transition_runtime(world, visuals);

        // Flex-column position pass: emit UpdateTransform for dirty LayoutComponent subtrees.
        // Runs after transforms are propagated so initial world matrices are valid.
        self.layout.tick(world, queue);
        queue.flush(world, self, visuals, render_assets);

        self.fit_bounds.tick(world, render_assets, queue);

        // Remeasure any pending preview shells whose styled content now has
        // layout-generated background quads (RenderableComponents) available.
        self.asset_system
            .remeasure_pending_previews(world, render_assets, queue);
        // Flush immediately so the UpdateTransform intent (centering + scale)
        // takes effect on this frame, not the next.
        queue.flush(world, self, visuals, render_assets);

        self.renderable.tick(world, visuals, input, dt_sec);

        self.renderer_stats
            .tick_with_queue(world, visuals, queue, dt_sec);

        self.text.tick(world, visuals, input, dt_sec);

        self.light.tick(world, visuals, input, dt_sec);
        self.mirror.tick(world, visuals, input, dt_sec);
        for texture_component in self.mirror.take_pending_texture_registrations() {
            self.register_texture(world, visuals, texture_component);
        }
    }

    /// Process commands from the command queue.
    pub fn process_commands(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        render_assets: &mut crate::engine::graphics::RenderAssets,
        commands: &mut crate::engine::ecs::CommandQueue,
    ) {
        commands.flush(world, self, visuals, render_assets);

        // Drain-point: ensure any remaining undispatched signals get handled.
        // This covers signals emitted after the last explicit dispatch point.
        let _ = self.process_signals(world, visuals, render_assets, commands, 100_000);

        // Signal handlers may have queued commands (e.g. register_color). Apply them now so
        // the effects are visible this frame.
        commands.flush(world, self, visuals, render_assets);

        // Batch audio graph rebuild work once after all mutations for this frame.
        self.audio.rebuild_dirty_audio_graphs(world);

        // Drain decode-worker completion messages so newly-loaded clips
        // ship to the RT thread and `AudioClipComponent.load_state`
        // updates land in the world.
        self.audio.drain_decode_completions_public(world);
    }
}

/// Attach a `BoundsComponent` child to a renderable, caching its mesh's local
/// AABB. Skips when the mesh has no tabulated AABB (e.g. GLTF-loaded) or when
/// a `BoundsComponent` child already exists.
fn attach_bounds_for_renderable(world: &mut World, renderable_id: ComponentId) {
    use crate::engine::ecs::component::{BoundsComponent, RenderableComponent};
    use crate::engine::graphics::bounds::mesh_local_aabb;

    let mesh = match world.get_component_by_id_as::<RenderableComponent>(renderable_id) {
        Some(r) => r.renderable.base_mesh,
        None => return,
    };
    let Some(local) = mesh_local_aabb(mesh) else {
        return;
    };

    let already = world
        .children_of(renderable_id)
        .iter()
        .any(|&c| world.get_component_by_id_as::<BoundsComponent>(c).is_some());
    if already {
        return;
    }

    let bounds_id = world.add_component(BoundsComponent::new(local));
    let _ = world.add_child(renderable_id, bounds_id);
}
