/**
 * Queue for commands (methods on components)
 * which reach systems after all components have been interacted, before rendering the next frame.
 *
 */

pub struct CommandQueue {
    commands: Vec<ComponentCommand>,

    // Per-frame transport context.
    frame_beat_now: f64,
    frame_bpm: f64,
}

impl CommandQueue {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            frame_beat_now: 0.0,
            frame_bpm: 120.0,
        }
    }

    pub fn set_transport(&mut self, beat_now: f64, bpm: f64) {
        self.frame_beat_now = beat_now;
        self.frame_bpm = bpm;
    }

    pub fn beat_now(&self) -> f64 {
        self.frame_beat_now
    }

    pub fn bpm(&self) -> f64 {
        self.frame_bpm
    }

    /// Queue a register renderable command.
    pub fn queue_register_renderable(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterRenderable { component_id },
        });
    }

    /// Queue a remove renderable command.
    pub fn queue_remove_renderable(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RemoveRenderable { component_id },
        });
    }

    /// Queue a register transform command.
    pub fn queue_register_transform(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterTransform { component_id },
        });
    }

    /// Queue an update transform command.
    pub fn queue_update_transform(
        &mut self,
        component_id: crate::engine::ecs::ComponentId,
        transform: crate::engine::graphics::primitives::Transform,
    ) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::UpdateTransform {
                component_id,
                transform,
            },
        });
    }

    /// Queue a register 3D camera command.
    pub fn queue_register_camera_3d(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterCamera3d { component_id },
        });
    }

    /// Queue a register camera2d command.
    pub fn queue_register_camera2d(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterCamera2d { component_id },
        });
    }

    /// Queue a make active camera command.
    pub fn queue_make_active_camera(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::MakeActiveCamera { component_id },
        });
    }

    /// Queue a register input command.
    pub fn queue_register_input(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterInput { component_id },
        });
    }

    /// Queue a register UV command.
    pub fn queue_register_uv(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterUv { component_id },
        });
    }

    /// Queue a register point light command.
    pub fn queue_register_light(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterLight { component_id },
        });
    }

    /// Queue a register color command.
    pub fn queue_register_color(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterColor { component_id },
        });
    }

    /// Queue a register opacity command.
    pub fn queue_register_opacity(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterOpacity { component_id },
        });
    }

    /// Queue a register transparent cutout command.
    pub fn queue_register_transparent_cutout(
        &mut self,
        component_id: crate::engine::ecs::ComponentId,
    ) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterTransparentCutout { component_id },
        });
    }

    /// Queue a register background color command.
    pub fn queue_register_background_color(
        &mut self,
        component_id: crate::engine::ecs::ComponentId,
    ) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterBackgroundColor { component_id },
        });
    }

    /// Queue a register ambient light command.
    pub fn queue_register_ambient_light(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterAmbientLight { component_id },
        });
    }

    /// Queue a register texture command.
    pub fn queue_register_texture(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterTexture { component_id },
        });
    }

    /// Queue a register texture filtering command.
    pub fn queue_register_texture_filtering(
        &mut self,
        component_id: crate::engine::ecs::ComponentId,
    ) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterTextureFiltering { component_id },
        });
    }

    /// Queue a register text command.
    pub fn queue_register_text(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterText { component_id },
        });
    }

    /// Queue a text update command.
    ///
    /// This updates the `TextComponent.text` value and rebuilds its glyph subtree.
    pub fn queue_set_text(&mut self, component_id: crate::engine::ecs::ComponentId, text: String) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::SetText { component_id, text },
        });
    }

    /// Queue a register emissive command.
    pub fn queue_register_emissive(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterEmissive { component_id },
        });
    }

    /// Queue a register light quantization command.
    pub fn queue_register_light_quantization(
        &mut self,
        component_id: crate::engine::ecs::ComponentId,
    ) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterLightQuantization { component_id },
        });
    }

    /// Queue a register collision command.
    pub fn queue_register_collision(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterCollision { component_id },
        });
    }

    /// Queue a register kinetic response command.
    pub fn queue_register_kinetic_response(
        &mut self,
        component_id: crate::engine::ecs::ComponentId,
    ) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterKineticResponse { component_id },
        });
    }

    /// Queue a remove kinetic response command.
    pub fn queue_remove_kinetic_response(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RemoveKineticResponse { component_id },
        });
    }

    /// Queue a remove collision command.
    pub fn queue_remove_collision(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RemoveCollision { component_id },
        });
    }

    /// Queue a remove subtree command.
    ///
    /// This removes any system/visual state for components in the subtree and then deletes the
    /// components from the World.
    pub fn queue_remove_subtree(&mut self, root: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id: root,
            command: Command::RemoveSubtree { root },
        });
    }

    /// Queue a register OpenXR command.
    pub fn queue_register_openxr(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterOpenxr { component_id },
        });
    }

    /// Queue a register ControllerXR command.
    pub fn queue_register_controller_xr(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterControllerXr { component_id },
        });
    }

    /// Queue a register RayCast command.
    pub fn queue_register_raycast(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterRaycast { component_id },
        });
    }

    pub fn queue_register_animation(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterAnimation { component_id },
        });
    }

    pub fn queue_register_keyframe(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterKeyframe { component_id },
        });
    }

    pub fn queue_register_audio_output(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterAudioOutput { component_id },
        });
    }

    /// Mark an audio graph subtree as dirty so compilation can be deferred/batched.
    pub fn queue_audio_graph_dirty(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::AudioGraphDirty { component_id },
        });
    }

    pub fn queue_register_audio_oscillator(
        &mut self,
        component_id: crate::engine::ecs::ComponentId,
    ) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterAudioOscillator { component_id },
        });
    }

    pub fn queue_register_audio_buffer_size(
        &mut self,
        component_id: crate::engine::ecs::ComponentId,
    ) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterAudioBufferSize { component_id },
        });
    }

    pub fn queue_schedule_audio_op(
        &mut self,
        component_id: crate::engine::ecs::ComponentId,
        beat: f64,
        op: crate::engine::ecs::system::audio_system::AudioOp,
    ) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::ScheduleAudioOp {
                component_id,
                beat,
                op,
            },
        });
    }

    pub fn queue_schedule_audio_graph_swap(
        &mut self,
        component_id: crate::engine::ecs::ComponentId,
        beat: f64,
    ) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::ScheduleAudioGraphSwap { component_id, beat },
        });
    }

    pub fn queue_schedule_audio_pitch_set_hz(
        &mut self,
        component_id: crate::engine::ecs::ComponentId,
        beat: f64,
        frequency_hz: f32,
    ) {
        self.queue_schedule_audio_op(
            component_id,
            beat,
            crate::engine::ecs::system::audio_system::AudioOp::SetHz(frequency_hz),
        );
    }

    pub fn queue_schedule_audio_oscillator_enabled(
        &mut self,
        component_id: crate::engine::ecs::ComponentId,
        beat: f64,
        enabled: bool,
    ) {
        self.queue_schedule_audio_op(
            component_id,
            beat,
            crate::engine::ecs::system::audio_system::AudioOp::SetEnabled(enabled),
        );
    }

    pub fn queue_schedule_audio_gain_set(
        &mut self,
        component_id: crate::engine::ecs::ComponentId,
        beat: f64,
        gain: f32,
    ) {
        self.queue_schedule_audio_op(
            component_id,
            beat,
            crate::engine::ecs::system::audio_system::AudioOp::SetGain(gain),
        );
    }

    pub fn queue_register_clock(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterClock { component_id },
        });
    }

    /// Queue a register transform gizmo command.
    pub fn queue_register_transform_gizmo(
        &mut self,
        component_id: crate::engine::ecs::ComponentId,
    ) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RegisterTransformGizmo { component_id },
        });
    }

    /// Queue a remove RayCast command.
    pub fn queue_remove_raycast(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RemoveRaycast { component_id },
        });
    }

    /// Queue a remove ControllerXR command.
    pub fn queue_remove_controller_xr(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::RemoveControllerXr { component_id },
        });
    }

    /// Flush all queued commands, executing them through the systems.
    pub fn flush(
        &mut self,
        world: &mut crate::engine::ecs::World,
        systems: &mut crate::engine::ecs::system::SystemWorld,
        visuals: &mut crate::engine::graphics::VisualWorld,
    ) {
        // Drain until empty so commands queued by handlers (e.g. init-time expansion)
        // are processed in the same frame.
        let mut passes = 0usize;
        while !self.commands.is_empty() {
            passes += 1;
            if passes > 1000 {
                println!("[CommandQueue] aborting flush: too many passes (possible infinite loop)");
                break;
            }

            let commands = std::mem::take(&mut self.commands);
            for cmd in commands {
                match cmd.command {
                    Command::RegisterTransform { component_id } => {
                        systems.transform_changed(world, visuals, component_id);
                    }
                    Command::UpdateTransform {
                        component_id,
                        transform,
                    } => {
                        systems.update_transform(world, visuals, component_id, transform);
                    }
                    Command::RemoveTransform { component_id } => {
                        systems.remove_transform(world, visuals, component_id);
                    }
                    Command::RegisterCamera3d { component_id } => {
                        systems.register_camera(world, visuals, component_id);
                    }
                    Command::RegisterCamera2d { component_id } => {
                        systems.register_camera2d(world, visuals, component_id);
                    }
                    Command::MakeActiveCamera { component_id } => {
                        systems.make_active_camera(world, visuals, component_id);
                    }
                    Command::RegisterInput { component_id } => {
                        systems.register_input(component_id);
                    }
                    Command::RegisterRenderable { component_id } => {
                        systems.register_renderable(world, visuals, component_id);
                    }
                    Command::RegisterUv { component_id } => {
                        systems.register_uv(world, visuals, component_id);
                    }
                    Command::RegisterLight { component_id } => {
                        systems.register_light(world, visuals, component_id);
                    }
                    Command::RegisterColor { component_id } => {
                        systems.register_color(world, visuals, component_id);
                    }
                    Command::RegisterOpacity { component_id } => {
                        systems.register_opacity(world, visuals, component_id);
                    }
                    Command::RegisterTransparentCutout { component_id } => {
                        systems.register_transparent_cutout(world, visuals, component_id);
                    }
                    Command::RegisterBackgroundColor { component_id } => {
                        systems.register_background_color(world, visuals, component_id);
                    }
                    Command::RegisterAmbientLight { component_id } => {
                        systems.register_ambient_light(world, visuals, component_id);
                    }
                    Command::RegisterTexture { component_id } => {
                        systems.register_texture(world, visuals, component_id);
                    }
                    Command::RegisterTextureFiltering { component_id } => {
                        systems.register_texture_filtering(world, visuals, component_id);
                    }
                    Command::RegisterText { component_id } => {
                        systems.register_text(world, visuals, component_id, self);
                    }
                    Command::SetText { component_id, text } => {
                        use crate::engine::ecs::component::{
                            RenderableComponent, TextComponent, TransformComponent,
                        };

                        let Some(_existing) =
                            world.get_component_by_id_as::<TextComponent>(component_id)
                        else {
                            continue;
                        };

                        // Collect glyph roots (TransformComponent children of the TextComponent).
                        let glyph_roots: Vec<crate::engine::ecs::ComponentId> = world
                            .children_of(component_id)
                            .iter()
                            .copied()
                            .filter(|&ch| {
                                world
                                    .get_component_by_id_as::<TransformComponent>(ch)
                                    .is_some()
                            })
                            .collect();

                        // Collect renderables to remove from VisualWorld before deleting subtrees.
                        let mut renderables_to_remove: Vec<crate::engine::ecs::ComponentId> =
                            Vec::new();
                        for &root in glyph_roots.iter() {
                            let mut stack: Vec<crate::engine::ecs::ComponentId> = vec![root];
                            while let Some(node) = stack.pop() {
                                if world
                                    .get_component_by_id_as::<RenderableComponent>(node)
                                    .is_some()
                                {
                                    renderables_to_remove.push(node);
                                }
                                for &ch in world.children_of(node).iter() {
                                    stack.push(ch);
                                }
                            }
                        }

                        // Remove renderable instances (and BVH entries) first.
                        for rid in renderables_to_remove {
                            systems.remove_renderable(world, visuals, rid);
                        }

                        // Delete glyph subtrees.
                        for root in glyph_roots {
                            let _ = world.remove_component_subtree(root);
                        }

                        // Update text and force rebuild.
                        if let Some(tc) =
                            world.get_component_by_id_as_mut::<TextComponent>(component_id)
                        {
                            tc.text = text;
                            tc.mark_unbuilt();
                        }

                        // Expand and register the new glyph tree.
                        systems.register_text(world, visuals, component_id, self);
                    }
                    Command::RegisterEmissive { component_id } => {
                        systems.register_emissive(world, visuals, component_id);
                    }
                    Command::RegisterLightQuantization { component_id } => {
                        systems.register_light_quantization(world, visuals, component_id);
                    }
                    Command::RegisterCollision { component_id } => {
                        systems.register_collision(world, visuals, component_id);
                    }
                    Command::RegisterKineticResponse { component_id } => {
                        systems.register_kinetic_response(world, visuals, component_id);
                    }
                    Command::RegisterOpenxr { component_id } => {
                        systems.register_openxr(world, visuals, component_id);
                    }
                    Command::RegisterControllerXr { component_id } => {
                        systems.register_controller_xr(world, visuals, component_id);
                    }
                    Command::RegisterRaycast { component_id } => {
                        systems.register_raycast(world, visuals, component_id);
                    }
                    Command::RegisterAnimation { component_id } => {
                        systems.register_animation(world, visuals, component_id);
                    }
                    Command::RegisterKeyframe { component_id } => {
                        systems.register_keyframe(world, visuals, component_id);
                    }
                    Command::RegisterAudioOutput { component_id } => {
                        systems.register_audio_output(world, visuals, component_id);
                    }
                    Command::RegisterAudioOscillator { component_id } => {
                        systems.register_audio_oscillator(world, visuals, component_id);
                    }
                    Command::RegisterAudioBufferSize { component_id } => {
                        systems.register_audio_buffer_size(world, visuals, component_id);
                    }

                    Command::AudioGraphDirty { component_id } => {
                        systems.audio_graph_dirty(world, visuals, component_id);
                    }
                    Command::RegisterClock { component_id } => {
                        systems.register_clock(world, visuals, component_id);
                    }
                    Command::RegisterTransformGizmo { component_id } => {
                        systems.register_transform_gizmo(world, visuals, component_id, self);
                    }
                    Command::ScheduleAudioOp {
                        component_id,
                        beat,
                        op,
                    } => {
                        systems.audio.schedule_audio_op(component_id, beat, op);
                    }
                    Command::ScheduleAudioGraphSwap { component_id, beat } => {
                        systems.audio.schedule_graph_swap(world, component_id, beat);
                    }
                    Command::RemoveCollision { component_id } => {
                        systems.remove_collision(world, visuals, component_id);
                    }
                    Command::RemoveKineticResponse { component_id } => {
                        systems.remove_kinetic_response(world, visuals, component_id);
                    }
                    Command::RemoveRaycast { component_id } => {
                        systems.remove_raycast(world, visuals, component_id);
                    }
                    Command::RemoveControllerXr { component_id } => {
                        systems.remove_controller_xr(world, visuals, component_id);
                    }
                    Command::RemoveRenderable { component_id: _ } => {
                        systems.remove_renderable(world, visuals, cmd.component_id);
                    }
                    Command::RemoveSubtree { root } => {
                        use crate::engine::ecs::component::{
                            CollisionComponent, RayCastComponent, RenderableComponent,
                        };

                        if world.get_component_record(root).is_none() {
                            continue;
                        }

                        // Collect subtree node ids.
                        let mut stack: Vec<crate::engine::ecs::ComponentId> = vec![root];
                        let mut subtree: Vec<crate::engine::ecs::ComponentId> = Vec::new();
                        while let Some(node) = stack.pop() {
                            subtree.push(node);
                            for &ch in world.children_of(node).iter() {
                                stack.push(ch);
                            }
                        }

                        // Remove system/visual state before deleting nodes.
                        for cid in subtree.iter().copied() {
                            if world
                                .get_component_by_id_as::<RenderableComponent>(cid)
                                .is_some()
                            {
                                systems.remove_renderable(world, visuals, cid);
                            }
                            if world
                                .get_component_by_id_as::<CollisionComponent>(cid)
                                .is_some()
                            {
                                systems.remove_collision(world, visuals, cid);
                            }
                            if world
                                .get_component_by_id_as::<RayCastComponent>(cid)
                                .is_some()
                            {
                                systems.remove_raycast(world, visuals, cid);
                            }
                        }

                        let _ = world.remove_component_subtree(root);
                    }
                    Command::RemoveCamera { component_id: _ } => {
                        // TODO: implement when needed
                    }
                }
            }
        }

        // Keep the renderable BVH in sync with any renderable/transform changes applied above.
        // This is intentionally done once after the queue is fully drained to avoid N rebuilds
        // during init-time expansion (e.g. text glyph spawning).
        systems.bvh.flush_pending(&*world);
    }
}

pub struct ComponentCommand {
    component_id: crate::engine::ecs::ComponentId,
    command: Command,
    //
}

enum Command {
    RegisterRenderable {
        component_id: crate::engine::ecs::ComponentId,
    },
    RegisterTransform {
        component_id: crate::engine::ecs::ComponentId,
    },
    RegisterInput {
        component_id: crate::engine::ecs::ComponentId,
    },
    RegisterCamera3d {
        component_id: crate::engine::ecs::ComponentId,
    },
    RegisterCamera2d {
        component_id: crate::engine::ecs::ComponentId,
    },
    RegisterUv {
        component_id: crate::engine::ecs::ComponentId,
    },
    RegisterLight {
        component_id: crate::engine::ecs::ComponentId,
    },
    RegisterColor {
        component_id: crate::engine::ecs::ComponentId,
    },
    RegisterOpacity {
        component_id: crate::engine::ecs::ComponentId,
    },
    RegisterTransparentCutout {
        component_id: crate::engine::ecs::ComponentId,
    },
    RegisterBackgroundColor {
        component_id: crate::engine::ecs::ComponentId,
    },
    RegisterAmbientLight {
        component_id: crate::engine::ecs::ComponentId,
    },
    RegisterTexture {
        component_id: crate::engine::ecs::ComponentId,
    },
    RegisterTextureFiltering {
        component_id: crate::engine::ecs::ComponentId,
    },
    RegisterText {
        component_id: crate::engine::ecs::ComponentId,
    },
    SetText {
        component_id: crate::engine::ecs::ComponentId,
        text: String,
    },
    RegisterEmissive {
        component_id: crate::engine::ecs::ComponentId,
    },
    RegisterLightQuantization {
        component_id: crate::engine::ecs::ComponentId,
    },
    RegisterCollision {
        component_id: crate::engine::ecs::ComponentId,
    },
    RegisterKineticResponse {
        component_id: crate::engine::ecs::ComponentId,
    },
    RegisterOpenxr {
        component_id: crate::engine::ecs::ComponentId,
    },

    RegisterControllerXr {
        component_id: crate::engine::ecs::ComponentId,
    },

    RegisterRaycast {
        component_id: crate::engine::ecs::ComponentId,
    },

    RegisterAnimation {
        component_id: crate::engine::ecs::ComponentId,
    },
    RegisterKeyframe {
        component_id: crate::engine::ecs::ComponentId,
    },
    RegisterAudioOutput {
        component_id: crate::engine::ecs::ComponentId,
    },

    RegisterAudioOscillator {
        component_id: crate::engine::ecs::ComponentId,
    },

    RegisterAudioBufferSize {
        component_id: crate::engine::ecs::ComponentId,
    },

    AudioGraphDirty {
        component_id: crate::engine::ecs::ComponentId,
    },

    RegisterClock {
        component_id: crate::engine::ecs::ComponentId,
    },

    RegisterTransformGizmo {
        component_id: crate::engine::ecs::ComponentId,
    },

    ScheduleAudioOp {
        component_id: crate::engine::ecs::ComponentId,
        beat: f64,
        op: crate::engine::ecs::system::audio_system::AudioOp,
    },
    ScheduleAudioGraphSwap {
        component_id: crate::engine::ecs::ComponentId,
        beat: f64,
    },

    RemoveRenderable {
        component_id: crate::engine::ecs::ComponentId,
    },
    RemoveTransform {
        component_id: crate::engine::ecs::ComponentId,
    },
    RemoveCamera {
        component_id: crate::engine::ecs::ComponentId,
    },

    RemoveRaycast {
        component_id: crate::engine::ecs::ComponentId,
    },

    RemoveControllerXr {
        component_id: crate::engine::ecs::ComponentId,
    },

    RemoveCollision {
        component_id: crate::engine::ecs::ComponentId,
    },
    RemoveKineticResponse {
        component_id: crate::engine::ecs::ComponentId,
    },

    RemoveSubtree {
        root: crate::engine::ecs::ComponentId,
    },

    UpdateTransform {
        component_id: crate::engine::ecs::ComponentId,
        transform: crate::engine::graphics::primitives::Transform,
    },

    MakeActiveCamera {
        component_id: crate::engine::ecs::ComponentId,
    },
}
