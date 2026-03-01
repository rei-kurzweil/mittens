/**
 * Queue for commands (methods on components)
 * which reach systems after all components have been interacted, before rendering the next frame.
 *
 */

pub struct CommandQueue {
    commands: Vec<ComponentCommand>,
}

impl CommandQueue {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    /// Queue a register renderable command.
    pub fn queue_register_renderable(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_RENDERABLE { component_id },
        });
    }

    /// Queue a remove renderable command.
    pub fn queue_remove_renderable(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REMOVE_RENDERABLE { component_id },
        });
    }

    /// Queue a register transform command.
    pub fn queue_register_transform(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_TRANSFORM { component_id },
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
            command: Command::UPDATE_TRANSFORM {
                component_id,
                transform,
            },
        });
    }

    /// Queue a register 3D camera command.
    pub fn queue_register_camera_3d(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_CAMERA_3D { component_id },
        });
    }

    /// Queue a register camera2d command.
    pub fn queue_register_camera2d(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_CAMERA2D { component_id },
        });
    }

    /// Queue a make active camera command.
    pub fn queue_make_active_camera(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::MAKE_ACTIVE_CAMERA { component_id },
        });
    }

    /// Queue a register input command.
    pub fn queue_register_input(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_INPUT { component_id },
        });
    }

    /// Queue a register UV command.
    pub fn queue_register_uv(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_UV { component_id },
        });
    }

    /// Queue a register point light command.
    pub fn queue_register_light(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_LIGHT { component_id },
        });
    }

    /// Queue a register color command.
    pub fn queue_register_color(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_COLOR { component_id },
        });
    }

    /// Queue a register opacity command.
    pub fn queue_register_opacity(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_OPACITY { component_id },
        });
    }

    /// Queue a register transparent cutout command.
    pub fn queue_register_transparent_cutout(
        &mut self,
        component_id: crate::engine::ecs::ComponentId,
    ) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_TRANSPARENT_CUTOUT { component_id },
        });
    }

    /// Queue a register background color command.
    pub fn queue_register_background_color(
        &mut self,
        component_id: crate::engine::ecs::ComponentId,
    ) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_BACKGROUND_COLOR { component_id },
        });
    }

    /// Queue a register ambient light command.
    pub fn queue_register_ambient_light(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_AMBIENT_LIGHT { component_id },
        });
    }

    /// Queue a register texture command.
    pub fn queue_register_texture(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_TEXTURE { component_id },
        });
    }

    /// Queue a register texture filtering command.
    pub fn queue_register_texture_filtering(
        &mut self,
        component_id: crate::engine::ecs::ComponentId,
    ) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_TEXTURE_FILTERING { component_id },
        });
    }

    /// Queue a register text command.
    pub fn queue_register_text(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_TEXT { component_id },
        });
    }

    /// Queue a text update command.
    ///
    /// This updates the `TextComponent.text` value and rebuilds its glyph subtree.
    pub fn queue_set_text(&mut self, component_id: crate::engine::ecs::ComponentId, text: String) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::SET_TEXT { component_id, text },
        });
    }

    /// Queue a register emissive command.
    pub fn queue_register_emissive(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_EMISSIVE { component_id },
        });
    }

    /// Queue a register light quantization command.
    pub fn queue_register_light_quantization(
        &mut self,
        component_id: crate::engine::ecs::ComponentId,
    ) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_LIGHT_QUANTIZATION { component_id },
        });
    }

    /// Queue a register collision command.
    pub fn queue_register_collision(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_COLLISION { component_id },
        });
    }

    /// Queue a register kinetic response command.
    pub fn queue_register_kinetic_response(
        &mut self,
        component_id: crate::engine::ecs::ComponentId,
    ) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_KINETIC_RESPONSE { component_id },
        });
    }

    /// Queue a remove kinetic response command.
    pub fn queue_remove_kinetic_response(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REMOVE_KINETIC_RESPONSE { component_id },
        });
    }

    /// Queue a remove collision command.
    pub fn queue_remove_collision(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REMOVE_COLLISION { component_id },
        });
    }

    /// Queue a remove subtree command.
    ///
    /// This removes any system/visual state for components in the subtree and then deletes the
    /// components from the World.
    pub fn queue_remove_subtree(&mut self, root: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id: root,
            command: Command::REMOVE_SUBTREE { root },
        });
    }

    /// Queue a register OpenXR command.
    pub fn queue_register_openxr(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_OPENXR { component_id },
        });
    }

    /// Queue a register ControllerXR command.
    pub fn queue_register_controller_xr(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_CONTROLLER_XR { component_id },
        });
    }

    /// Queue a register RayCast command.
    pub fn queue_register_raycast(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_RAYCAST { component_id },
        });
    }

    pub fn queue_register_animation(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_ANIMATION { component_id },
        });
    }

    pub fn queue_register_keyframe(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_KEYFRAME { component_id },
        });
    }

    pub fn queue_register_audio_output(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_AUDIO_OUTPUT { component_id },
        });
    }

    /// Mark an audio graph subtree as dirty so compilation can be deferred/batched.
    pub fn queue_audio_graph_dirty(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::AUDIO_GRAPH_DIRTY { component_id },
        });
    }

    pub fn queue_register_audio_oscillator(
        &mut self,
        component_id: crate::engine::ecs::ComponentId,
    ) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_AUDIO_OSCILLATOR { component_id },
        });
    }

    pub fn queue_register_audio_buffer_size(
        &mut self,
        component_id: crate::engine::ecs::ComponentId,
    ) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_AUDIO_BUFFER_SIZE { component_id },
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
            command: Command::SCHEDULE_AUDIO_OP {
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
            command: Command::SCHEDULE_AUDIO_GRAPH_SWAP { component_id, beat },
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
            command: Command::REGISTER_CLOCK { component_id },
        });
    }

    /// Queue a register gizmo command.
    pub fn queue_register_gizmo(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_GIZMO { component_id },
        });
    }

    /// Queue a remove RayCast command.
    pub fn queue_remove_raycast(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REMOVE_RAYCAST { component_id },
        });
    }

    /// Queue a remove ControllerXR command.
    pub fn queue_remove_controller_xr(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REMOVE_CONTROLLER_XR { component_id },
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
                    Command::REGISTER_TRANSFORM { component_id } => {
                        systems.transform_changed(world, visuals, component_id);
                    }
                    Command::UPDATE_TRANSFORM {
                        component_id,
                        transform,
                    } => {
                        systems.update_transform(world, visuals, component_id, transform);
                    }
                    Command::REMOVE_TRANSFORM { component_id } => {
                        systems.remove_transform(world, visuals, component_id);
                    }
                    Command::REGISTER_CAMERA_3D { component_id } => {
                        systems.register_camera(world, visuals, component_id);
                    }
                    Command::REGISTER_CAMERA2D { component_id } => {
                        systems.register_camera2d(world, visuals, component_id);
                    }
                    Command::MAKE_ACTIVE_CAMERA { component_id } => {
                        systems.make_active_camera(world, visuals, component_id);
                    }
                    Command::REGISTER_INPUT { component_id } => {
                        systems.register_input(component_id);
                    }
                    Command::REGISTER_RENDERABLE { component_id } => {
                        systems.register_renderable(world, visuals, component_id);
                    }
                    Command::REGISTER_UV { component_id } => {
                        systems.register_uv(world, visuals, component_id);
                    }
                    Command::REGISTER_LIGHT { component_id } => {
                        systems.register_light(world, visuals, component_id);
                    }
                    Command::REGISTER_COLOR { component_id } => {
                        systems.register_color(world, visuals, component_id);
                    }
                    Command::REGISTER_OPACITY { component_id } => {
                        systems.register_opacity(world, visuals, component_id);
                    }
                    Command::REGISTER_TRANSPARENT_CUTOUT { component_id } => {
                        systems.register_transparent_cutout(world, visuals, component_id);
                    }
                    Command::REGISTER_BACKGROUND_COLOR { component_id } => {
                        systems.register_background_color(world, visuals, component_id);
                    }
                    Command::REGISTER_AMBIENT_LIGHT { component_id } => {
                        systems.register_ambient_light(world, visuals, component_id);
                    }
                    Command::REGISTER_TEXTURE { component_id } => {
                        systems.register_texture(world, visuals, component_id);
                    }
                    Command::REGISTER_TEXTURE_FILTERING { component_id } => {
                        systems.register_texture_filtering(world, visuals, component_id);
                    }
                    Command::REGISTER_TEXT { component_id } => {
                        systems.register_text(world, visuals, component_id, self);
                    }
                    Command::SET_TEXT { component_id, text } => {
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
                    Command::REGISTER_EMISSIVE { component_id } => {
                        systems.register_emissive(world, visuals, component_id);
                    }
                    Command::REGISTER_LIGHT_QUANTIZATION { component_id } => {
                        systems.register_light_quantization(world, visuals, component_id);
                    }
                    Command::REGISTER_COLLISION { component_id } => {
                        systems.register_collision(world, visuals, component_id);
                    }
                    Command::REGISTER_KINETIC_RESPONSE { component_id } => {
                        systems.register_kinetic_response(world, visuals, component_id);
                    }
                    Command::REGISTER_OPENXR { component_id } => {
                        systems.register_openxr(world, visuals, component_id);
                    }
                    Command::REGISTER_CONTROLLER_XR { component_id } => {
                        systems.register_controller_xr(world, visuals, component_id);
                    }
                    Command::REGISTER_RAYCAST { component_id } => {
                        systems.register_raycast(world, visuals, component_id);
                    }
                    Command::REGISTER_ANIMATION { component_id } => {
                        systems.register_animation(world, visuals, component_id);
                    }
                    Command::REGISTER_KEYFRAME { component_id } => {
                        systems.register_keyframe(world, visuals, component_id);
                    }
                    Command::REGISTER_AUDIO_OUTPUT { component_id } => {
                        systems.register_audio_output(world, visuals, component_id);
                    }
                    Command::REGISTER_AUDIO_OSCILLATOR { component_id } => {
                        systems.register_audio_oscillator(world, visuals, component_id);
                    }
                    Command::REGISTER_AUDIO_BUFFER_SIZE { component_id } => {
                        systems.register_audio_buffer_size(world, visuals, component_id);
                    }

                    Command::AUDIO_GRAPH_DIRTY { component_id } => {
                        systems.audio_graph_dirty(world, visuals, component_id);
                    }
                    Command::REGISTER_CLOCK { component_id } => {
                        systems.register_clock(world, visuals, component_id);
                    }
                    Command::REGISTER_GIZMO { component_id } => {
                        systems.register_gizmo(world, visuals, component_id, self);
                    }
                    Command::SCHEDULE_AUDIO_OP {
                        component_id,
                        beat,
                        op,
                    } => {
                        systems.audio.schedule_audio_op(component_id, beat, op);
                    }
                    Command::SCHEDULE_AUDIO_GRAPH_SWAP { component_id, beat } => {
                        systems.audio.schedule_graph_swap(world, component_id, beat);
                    }
                    Command::REMOVE_COLLISION { component_id } => {
                        systems.remove_collision(world, visuals, component_id);
                    }
                    Command::REMOVE_KINETIC_RESPONSE { component_id } => {
                        systems.remove_kinetic_response(world, visuals, component_id);
                    }
                    Command::REMOVE_RAYCAST { component_id } => {
                        systems.remove_raycast(world, visuals, component_id);
                    }
                    Command::REMOVE_CONTROLLER_XR { component_id } => {
                        systems.remove_controller_xr(world, visuals, component_id);
                    }
                    Command::REMOVE_RENDERABLE { component_id: _ } => {
                        systems.remove_renderable(world, visuals, cmd.component_id);
                    }
                    Command::REMOVE_SUBTREE { root } => {
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
                    Command::REMOVE_CAMERA { component_id: _ } => {
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
    REGISTER_RENDERABLE {
        component_id: crate::engine::ecs::ComponentId,
    },
    REGISTER_TRANSFORM {
        component_id: crate::engine::ecs::ComponentId,
    },
    REGISTER_INPUT {
        component_id: crate::engine::ecs::ComponentId,
    },
    REGISTER_CAMERA_3D {
        component_id: crate::engine::ecs::ComponentId,
    },
    REGISTER_CAMERA2D {
        component_id: crate::engine::ecs::ComponentId,
    },
    REGISTER_UV {
        component_id: crate::engine::ecs::ComponentId,
    },
    REGISTER_LIGHT {
        component_id: crate::engine::ecs::ComponentId,
    },
    REGISTER_COLOR {
        component_id: crate::engine::ecs::ComponentId,
    },
    REGISTER_OPACITY {
        component_id: crate::engine::ecs::ComponentId,
    },
    REGISTER_TRANSPARENT_CUTOUT {
        component_id: crate::engine::ecs::ComponentId,
    },
    REGISTER_BACKGROUND_COLOR {
        component_id: crate::engine::ecs::ComponentId,
    },
    REGISTER_AMBIENT_LIGHT {
        component_id: crate::engine::ecs::ComponentId,
    },
    REGISTER_TEXTURE {
        component_id: crate::engine::ecs::ComponentId,
    },
    REGISTER_TEXTURE_FILTERING {
        component_id: crate::engine::ecs::ComponentId,
    },
    REGISTER_TEXT {
        component_id: crate::engine::ecs::ComponentId,
    },
    SET_TEXT {
        component_id: crate::engine::ecs::ComponentId,
        text: String,
    },
    REGISTER_EMISSIVE {
        component_id: crate::engine::ecs::ComponentId,
    },
    REGISTER_LIGHT_QUANTIZATION {
        component_id: crate::engine::ecs::ComponentId,
    },
    REGISTER_COLLISION {
        component_id: crate::engine::ecs::ComponentId,
    },
    REGISTER_KINETIC_RESPONSE {
        component_id: crate::engine::ecs::ComponentId,
    },
    REGISTER_OPENXR {
        component_id: crate::engine::ecs::ComponentId,
    },

    REGISTER_CONTROLLER_XR {
        component_id: crate::engine::ecs::ComponentId,
    },

    REGISTER_RAYCAST {
        component_id: crate::engine::ecs::ComponentId,
    },

    REGISTER_ANIMATION {
        component_id: crate::engine::ecs::ComponentId,
    },
    REGISTER_KEYFRAME {
        component_id: crate::engine::ecs::ComponentId,
    },
    REGISTER_AUDIO_OUTPUT {
        component_id: crate::engine::ecs::ComponentId,
    },

    REGISTER_AUDIO_OSCILLATOR {
        component_id: crate::engine::ecs::ComponentId,
    },

    REGISTER_AUDIO_BUFFER_SIZE {
        component_id: crate::engine::ecs::ComponentId,
    },

    AUDIO_GRAPH_DIRTY {
        component_id: crate::engine::ecs::ComponentId,
    },

    REGISTER_CLOCK {
        component_id: crate::engine::ecs::ComponentId,
    },

    REGISTER_GIZMO {
        component_id: crate::engine::ecs::ComponentId,
    },

    SCHEDULE_AUDIO_OP {
        component_id: crate::engine::ecs::ComponentId,
        beat: f64,
        op: crate::engine::ecs::system::audio_system::AudioOp,
    },
    SCHEDULE_AUDIO_GRAPH_SWAP {
        component_id: crate::engine::ecs::ComponentId,
        beat: f64,
    },

    REMOVE_RENDERABLE {
        component_id: crate::engine::ecs::ComponentId,
    },
    REMOVE_TRANSFORM {
        component_id: crate::engine::ecs::ComponentId,
    },
    REMOVE_CAMERA {
        component_id: crate::engine::ecs::ComponentId,
    },

    REMOVE_RAYCAST {
        component_id: crate::engine::ecs::ComponentId,
    },

    REMOVE_CONTROLLER_XR {
        component_id: crate::engine::ecs::ComponentId,
    },

    REMOVE_COLLISION {
        component_id: crate::engine::ecs::ComponentId,
    },
    REMOVE_KINETIC_RESPONSE {
        component_id: crate::engine::ecs::ComponentId,
    },

    REMOVE_SUBTREE {
        root: crate::engine::ecs::ComponentId,
    },

    UPDATE_TRANSFORM {
        component_id: crate::engine::ecs::ComponentId,
        transform: crate::engine::graphics::primitives::Transform,
    },

    MAKE_ACTIVE_CAMERA {
        component_id: crate::engine::ecs::ComponentId,
    },
}
