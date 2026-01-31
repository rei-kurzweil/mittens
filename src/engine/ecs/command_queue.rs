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

    /// Queue a remove collision command.
    pub fn queue_remove_collision(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REMOVE_COLLISION { component_id },
        });
    }

    /// Queue a register OpenXR command.
    pub fn queue_register_openxr(&mut self, component_id: crate::engine::ecs::ComponentId) {
        self.commands.push(ComponentCommand {
            component_id,
            command: Command::REGISTER_OPENXR { component_id },
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
                    Command::REGISTER_EMISSIVE { component_id } => {
                        systems.register_emissive(world, visuals, component_id);
                    }
                    Command::REGISTER_LIGHT_QUANTIZATION { component_id } => {
                        systems.register_light_quantization(world, visuals, component_id);
                    }
                    Command::REGISTER_COLLISION { component_id } => {
                        systems.register_collision(world, visuals, component_id);
                    }
                    Command::REGISTER_OPENXR { component_id } => {
                        systems.register_openxr(world, visuals, component_id);
                    }
                    Command::REMOVE_COLLISION { component_id } => {
                        systems.remove_collision(world, visuals, component_id);
                    }
                    Command::REMOVE_RENDERABLE { component_id: _ } => {
                        // TODO: implement when needed
                    }
                    Command::REMOVE_CAMERA { component_id: _ } => {
                        // TODO: implement when needed
                    }
                }
            }
        }
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
    REGISTER_EMISSIVE {
        component_id: crate::engine::ecs::ComponentId,
    },
    REGISTER_LIGHT_QUANTIZATION {
        component_id: crate::engine::ecs::ComponentId,
    },
    REGISTER_COLLISION {
        component_id: crate::engine::ecs::ComponentId,
    },
    REGISTER_OPENXR {
        component_id: crate::engine::ecs::ComponentId,
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

    REMOVE_COLLISION {
        component_id: crate::engine::ecs::ComponentId,
    },

    UPDATE_TRANSFORM {
        component_id: crate::engine::ecs::ComponentId,
        transform: crate::engine::graphics::primitives::Transform,
    },

    MAKE_ACTIVE_CAMERA {
        component_id: crate::engine::ecs::ComponentId,
    },
}
