use super::World;
use crate::engine::ecs::ComponentId;
use crate::engine::ecs::RxWorld;
use crate::engine::ecs::SignalKind;
use crate::engine::ecs::system::BvhSystem;
use crate::engine::ecs::system::CameraSystem;
use crate::engine::ecs::system::ClockSystem;
use crate::engine::ecs::system::CollisionSystem;
use crate::engine::ecs::system::GLTFSystem;
use crate::engine::ecs::system::InputSystem;
use crate::engine::ecs::system::KineticResponseSystem;
use crate::engine::ecs::system::LightSystem;
use crate::engine::ecs::system::MusicSystem;
use crate::engine::ecs::system::OpenXRSystem;
use crate::engine::ecs::system::PipelineSystem;
use crate::engine::ecs::system::PointerSystem;
use crate::engine::ecs::system::RayCastSystem;
use crate::engine::ecs::system::RenderToTextureSystem;
use crate::engine::ecs::system::RenderableSystem;
use crate::engine::ecs::system::RendererStatsSystem;
use crate::engine::ecs::system::RouterSystem;
use crate::engine::ecs::system::ScrollingSystem;
use crate::engine::ecs::system::SkinnedMeshSystem;
use crate::engine::ecs::system::System;
use crate::engine::ecs::system::TextSystem;
use crate::engine::ecs::system::TextureSystem;
use crate::engine::ecs::system::TransitionSystem;
use crate::engine::ecs::system::TransformPipelineSystem;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::system::{AnimationSystem, AudioSystem};
use crate::engine::ecs::system::{AvatarBodyYawSystem, AvatarControlSystem, EditorSystem, GestureSystem, IKSystem, InspectorSystem, LayoutSystem, TransformGizmoSystem};
use crate::engine::graphics::{RenderAssets, RenderUploader, VisualWorld};
use crate::engine::user_input::InputState;

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

    pub transform_pipeline: TransformPipelineSystem,
    pub transform: TransformSystem,
    pub bvh: BvhSystem,
    pub collision: CollisionSystem,
    pub kinetic_response: KineticResponseSystem,
    pub skinned_mesh: SkinnedMeshSystem,
    pub renderable: RenderableSystem,
    pub renderer_stats: RendererStatsSystem,
    pub router: RouterSystem,
    pub scrolling: ScrollingSystem,

    pub pointer: PointerSystem,
    pub raycast: RayCastSystem,

    pub editor: EditorSystem,
    pub inspector: InspectorSystem,
    pub avatar_body_yaw: AvatarBodyYawSystem,
    pub avatar_control: AvatarControlSystem,
    pub ik: IKSystem,

    pub gesture: GestureSystem,
    pub transform_gizmo: TransformGizmoSystem,

    pub layout: LayoutSystem,

    pub gltf: GLTFSystem,

    pub openxr: OpenXRSystem,

    pub pipeline: PipelineSystem,

    pub camera: CameraSystem,
    pub input: InputSystem,
    pub light: LightSystem,

    pub text: TextSystem,
    pub render_to_texture: RenderToTextureSystem,
    pub texture: TextureSystem,
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
            PointerComponent, RayCastComponent, RenderableComponent, SignalRouteUpwardComponent, TransformComponent,
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
        let _ = self.rx.remove_all_scoped_handlers_for_scopes(nodes);
    }

    pub fn new() -> Self {
        Self::default()
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
                        intent_executor.execute(world, queue, &env);
                    } else {
                        mutation_executor.execute(self, world, visuals, queue, &env);
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

            IntentValue::RegisterOpenxr { component } => {
                self.register_openxr(world, visuals, *component);
            }
            IntentValue::RegisterControllerXr { component } => {
                self.register_controller_xr(world, visuals, *component);
            }
            IntentValue::RemoveControllerXr { component } => {
                self.remove_controller_xr(world, visuals, *component);
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

    pub fn register_editor(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
    ) {
        let panel_cfg = world
            .get_component_by_id_as::<crate::engine::ecs::component::EditorComponent>(component)
            .map(|ed| (ed.spawn_panels, ed.world_panel_pos, ed.inspector_panel_pos));

        let Some((spawn_panels, world_panel_pos, inspector_panel_pos)) = panel_cfg else {
            return;
        };

        self.editor
            .materialize_editor_raycastables(world, emit, component);
        self.editor
            .install_scoped_handlers_for_editor(&mut self.rx, component);

        if spawn_panels {
            self.inspector.setup_panels_for_editor(
                &mut self.rx,
                world,
                emit,
                component,
                world_panel_pos,
                inspector_panel_pos,
            );
        }
    }

    pub fn register_scrolling(
        &mut self,
        world: &mut World,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        component: ComponentId,
    ) {
        self.scrolling.deferred_register(&mut self.rx, world, emit, component);
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

        // Keep BVH in sync (defer actual build/refit to CommandQueue flush).
        if BvhSystem::renderable_is_raycastable(world, component) {
            self.bvh.queue_renderable_added(component);
        }

        // Keep RayCastSystem's eligibility index in sync for brute-force fallback.
        self.raycast.notify_renderable_added(&*world, component);

        Self::sync_renderable_stencil_ref(world, visuals, component);
    }

    /// Register a StencilClipComponent: find the sibling RenderableComponent in the same
    /// TC subtree, count ancestor clip depth, and mark the VisualInstance as a clip source.
    pub fn register_stencil_clip(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        use crate::engine::ecs::component::StencilClipComponent;

        // Determine nesting depth by counting StencilClipComponent ancestors.
        let mut depth: u8 = 0;
        let mut cursor = world.parent_of(component);
        while let Some(p) = cursor {
            if world.get_component_by_id_as::<StencilClipComponent>(p).is_some() {
                depth = depth.saturating_add(1);
            }
            cursor = world.parent_of(p);
        }
        let stencil_ref = depth + 1; // 1-indexed; 0 = unclipped

        let handle = Self::find_stencil_clip_renderable_handle(world, component);
        if let Some(handle) = handle {
            visuals.register_stencil_clip(handle, stencil_ref);
        }

        if let Some(scope_root) = Self::stencil_clip_scope_root(world, component) {
            Self::sync_stencil_refs_in_subtree(world, visuals, scope_root);
        }
    }

    /// Unregister a StencilClipComponent: clear `is_stencil_clip` on the associated VisualInstance.
    pub fn unregister_stencil_clip(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        let handle = Self::find_stencil_clip_renderable_handle(world, component);
        if let Some(handle) = handle {
            visuals.unregister_stencil_clip(handle);
        }

        if let Some(scope_root) = Self::stencil_clip_scope_root(world, component) {
            Self::sync_stencil_refs_in_subtree(world, visuals, scope_root);
        }
    }

    fn sync_renderable_stencil_ref(
        world: &World,
        visuals: &mut VisualWorld,
        renderable_component: ComponentId,
    ) {
        use crate::engine::ecs::component::RenderableComponent;

        let Some(renderable) = world.get_component_by_id_as::<RenderableComponent>(renderable_component)
        else {
            return;
        };
        let Some(handle) = renderable.get_handle() else {
            return;
        };

        let stencil_ref = Self::stencil_ref_for_renderable(world, renderable_component);
        let _ = visuals.update_stencil_ref(handle, stencil_ref);
    }

    fn sync_stencil_refs_in_subtree(
        world: &World,
        visuals: &mut VisualWorld,
        root: ComponentId,
    ) {
        use crate::engine::ecs::component::RenderableComponent;

        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            if world.get_component_by_id_as::<RenderableComponent>(node).is_some() {
                Self::sync_renderable_stencil_ref(world, visuals, node);
            }
            for &child in world.children_of(node) {
                stack.push(child);
            }
        }
    }

    fn stencil_ref_for_renderable(world: &World, renderable_component: ComponentId) -> u8 {
        use crate::engine::ecs::component::StencilClipComponent;

        let mut depth: u8 = 0;
        let mut cursor = Some(renderable_component);
        while let Some(node) = cursor {
            if world.get_component_by_id_as::<StencilClipComponent>(node).is_some()
                || Self::is_layout_clip_scope_root(world, node)
            {
                depth = depth.saturating_add(1);
            }
            cursor = world.parent_of(node);
        }
        depth
    }

    fn is_layout_clip_scope_root(world: &World, node: ComponentId) -> bool {
        world.children_of(node).iter().copied().any(|child| {
            world.component_label(child) == Some("__bg")
                && Self::subtree_contains_stencil_clip(world, child)
        })
    }

    fn subtree_contains_stencil_clip(world: &World, root: ComponentId) -> bool {
        use crate::engine::ecs::component::StencilClipComponent;

        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            if world.get_component_by_id_as::<StencilClipComponent>(node).is_some() {
                return true;
            }
            for &child in world.children_of(node) {
                stack.push(child);
            }
        }
        false
    }

    fn stencil_clip_scope_root(world: &World, component: ComponentId) -> Option<ComponentId> {
        let parent = world.parent_of(component)?;
        if world.component_label(parent) == Some("__bg") {
            return world.parent_of(parent);
        }
        Some(parent)
    }

    fn find_stencil_clip_renderable_handle(
        world: &World,
        component: ComponentId,
    ) -> Option<crate::engine::graphics::primitives::InstanceHandle> {
        use crate::engine::ecs::component::RenderableComponent;

        if let Some(parent) = world.parent_of(component) {
            let mut stack = vec![parent];
            while let Some(cid) = stack.pop() {
                if let Some(r) = world.get_component_by_id_as::<RenderableComponent>(cid) {
                    if let Some(handle) = r.get_handle() {
                        return Some(handle);
                    }
                }
                for &ch in world.children_of(cid) {
                    stack.push(ch);
                }
            }
        }

        let mut cursor = world.parent_of(component);
        while let Some(cid) = cursor {
            if let Some(r) = world.get_component_by_id_as::<RenderableComponent>(cid) {
                if let Some(handle) = r.get_handle() {
                    return Some(handle);
                }
            }
            cursor = world.parent_of(cid);
        }

        None
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
                    if let Some(texture) = world.get_component_by_id_as::<
                        crate::engine::ecs::component::TextureComponent,
                    >(grandchild) {
                        if emissive_pass.output_texture.is_none() {
                            emissive_pass.output_texture = Some(
                                texture
                                    .render_image
                                    .clone()
                                    .unwrap_or_else(|| "render_graph.emissive_pass.output".to_string()),
                            );
                        }
                    }

                    if let Some(blur_pass) = world.get_component_by_id_as::<
                        crate::engine::ecs::component::BlurPassComponent,
                    >(grandchild) {
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

            let Some(bloom) =
                world.get_component_by_id_as::<crate::engine::ecs::component::BloomComponent>(child)
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

    /// Register an OpenXRComponent (initializes OpenXR runtime if enabled).
    pub fn register_openxr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.openxr.register_openxr(world, visuals, component);
    }

    /// Register an InputXRComponent (tracks the headset/root XR pose and drives a transform).
    pub fn register_input_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.openxr.register_input_xr(world, visuals, component);
    }

    /// Register a ControllerXRComponent (tracks an XR controller pose and drives a transform).
    pub fn register_controller_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.openxr
            .register_controller_xr(world, visuals, component);
    }

    /// Remove a ControllerXRComponent from OpenXRSystem tracking.
    pub fn remove_controller_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.openxr.remove_controller_xr(world, visuals, component);
    }

    /// Remove an InputXRComponent from OpenXRSystem tracking.
    pub fn remove_input_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.openxr.remove_input_xr(world, visuals, component);
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
        self.music.apply_music_note_to_oscillator(world, component);
        self.audio.register_audio_oscillator(world, component);
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

        self.renderable
            .flush_pending(world, visuals, render_assets, uploader, queue);

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
            &mut self.transform_pipeline,
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

        // Process input first - it may queue commands
        self.input.process_input(world, input, queue, dt_sec);

        // Spawn any GLTF component trees. This may queue component registrations.
        self.gltf
            .tick_with_queue(world, visuals, &mut self.skinned_mesh, queue, dt_sec);

        // Flush queued registrations/transform updates *before* systems that need current
        // world matrices / acceleration structures (e.g. raycasting).
        queue.flush(world, self, visuals);

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
        let _ = self.process_signals(world, visuals, queue, 100_000);
        queue.flush(world, self, visuals);
        self.tick_transition_runtime(world, visuals);

        self.transform_pipeline.tick(world, visuals, input, dt_sec);

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
        queue.flush(world, self, visuals);
        self.tick_transition_runtime(world, visuals);

        // Physics may have moved renderables; refit BVH so raycasts see the resolved state.
        self.bvh.tick(world, visuals, input, dt_sec);

        // Update window camera + select active XR camera rig before OpenXR consumes it.
        self.camera.tick(world, visuals, input, dt_sec);
        // OpenXR consumes the latest rig transform + publishes per-eye cameras.
        self.openxr
            .tick_with_queue(world, visuals, input, queue, dt_sec);
        // Controller pose updates should be visible to raycasting/gestures this frame.
        queue.flush(world, self, visuals);
        self.tick_transition_runtime(world, visuals);

        self.raycast
            .tick_with_queue(world, visuals, input, &mut self.rx, &self.bvh, dt_sec);

        // Execute/dispatch any signals produced by raycast immediately (e.g. RayIntersected).
        let _ = self.process_signals(world, visuals, queue, 100_000);

        // Gestures interpret ray hits + input into drag events.
        self.gesture.tick_with_rx(visuals, input, &mut self.rx);

        // Execute/dispatch gesture-produced signals immediately (e.g. DragStart/DragMove/DragEnd).
        let _ = self.process_signals(world, visuals, queue, 100_000);

        // Gizmos consume drag events and apply transform changes.
        self.transform_gizmo
            .tick_with_queue(world, input, queue, &mut self.rx);

        // Execute/dispatch gizmo-produced signals immediately (if any).
        let _ = self.process_signals(world, visuals, queue, 100_000);

        // Apply gizmo transform updates immediately so visuals reflect the drag this frame.
        queue.flush(world, self, visuals);
        self.tick_transition_runtime(world, visuals);

        // Avatar body yaw: smoothly rotate body to follow head when yaw diverges.
        // Runs after OpenXR + raycasts + gestures so avatar_driven_t.matrix_world is current.
        self.avatar_body_yaw.tick(world, queue, dt_sec);
        self.avatar_control.tick(world, queue, dt_sec);
        self.ik.tick(world, queue, dt_sec);
        queue.flush(world, self, visuals);
        self.tick_transition_runtime(world, visuals);

        // Flex-column position pass: emit UpdateTransform for dirty LayoutComponent subtrees.
        // Runs after transforms are propagated so initial world matrices are valid.
        self.layout.tick(world, queue);
        queue.flush(world, self, visuals);

        self.renderable.tick(world, visuals, input, dt_sec);

        self.renderer_stats
            .tick_with_queue(world, visuals, queue, dt_sec);

        self.text.tick(world, visuals, input, dt_sec);

        self.light.tick(world, visuals, input, dt_sec);
    }

    /// Process commands from the command queue.
    pub fn process_commands(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        commands: &mut crate::engine::ecs::CommandQueue,
    ) {
        commands.flush(world, self, visuals);

        // Drain-point: ensure any remaining undispatched signals get handled.
        // This covers signals emitted after the last explicit dispatch point.
        let _ = self.process_signals(world, visuals, commands, 100_000);

        // Signal handlers may have queued commands (e.g. register_color). Apply them now so
        // the effects are visible this frame.
        commands.flush(world, self, visuals);

        // Batch audio graph rebuild work once after all mutations for this frame.
        self.audio.rebuild_dirty_audio_graphs(world);
    }
}
