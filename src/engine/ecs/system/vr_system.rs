use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::OpenXRComponent;
use crate::engine::ecs::system::openxr_system::OpenXRSystem;
use crate::engine::ecs::system::System;
use crate::engine::ecs::World;
use crate::engine::graphics::{VisualWorld, VulkanoRenderer, XrVulkanGraphics};
use crate::engine::user_input::InputState;

pub use crate::engine::ecs::system::openxr_system::{
    XrGamepadState, XrHandGamepadState, XrInputState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VrBackendKind {
    OpenXR,
    OpenVR,
}

#[derive(Debug)]
enum VrBackend {
    OpenXR(OpenXRSystem),
}

#[derive(Debug)]
pub struct VrSystem {
    backend: VrBackend,
}

impl Default for VrSystem {
    fn default() -> Self {
        Self {
            backend: VrBackend::OpenXR(OpenXRSystem::default()),
        }
    }
}

impl VrSystem {
    pub fn active_backend_kind(&self) -> VrBackendKind {
        match self.backend {
            VrBackend::OpenXR(_) => VrBackendKind::OpenXR,
        }
    }

    pub fn xr_input_state(&self) -> &XrInputState {
        match &self.backend {
            VrBackend::OpenXR(system) => system.xr_input_state(),
        }
    }

    pub fn xr_gamepad_state(&self) -> &XrGamepadState {
        match &self.backend {
            VrBackend::OpenXR(system) => system.xr_gamepad_state(),
        }
    }

    pub fn set_preferred_swapchain_format(&mut self, format: u32) {
        match &mut self.backend {
            VrBackend::OpenXR(system) => system.set_preferred_swapchain_format(format),
        }
    }

    pub fn required_vulkan_extensions(&self) -> Option<(Vec<String>, Vec<String>)> {
        match &self.backend {
            VrBackend::OpenXR(system) => system.required_vulkan_extensions(),
        }
    }

    pub fn set_vulkan_graphics(&mut self, gfx: XrVulkanGraphics) {
        match &mut self.backend {
            VrBackend::OpenXR(system) => system.set_vulkan_graphics(gfx),
        }
    }

    pub fn register_openxr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        let enabled = world
            .get_component_by_id_as::<OpenXRComponent>(component)
            .map(|component| component.enabled)
            .unwrap_or(false);

        if enabled {
            self.ensure_openxr_backend();
        }

        match &mut self.backend {
            VrBackend::OpenXR(system) => system.register_openxr(world, visuals, component),
        }
    }

    pub fn register_controller_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        match &mut self.backend {
            VrBackend::OpenXR(system) => system.register_controller_xr(world, visuals, component),
        }
    }

    pub fn register_input_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        match &mut self.backend {
            VrBackend::OpenXR(system) => system.register_input_xr(world, visuals, component),
        }
    }

    pub fn remove_controller_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        match &mut self.backend {
            VrBackend::OpenXR(system) => system.remove_controller_xr(world, visuals, component),
        }
    }

    pub fn remove_input_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        match &mut self.backend {
            VrBackend::OpenXR(system) => system.remove_input_xr(world, visuals, component),
        }
    }

    pub fn tick_with_queue(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        input: &InputState,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        dt_sec: f32,
    ) {
        match &mut self.backend {
            VrBackend::OpenXR(system) => {
                system.tick_with_queue(world, visuals, input, emit, dt_sec)
            }
        }
    }

    pub fn last_render_dt_sec(&self) -> Option<f32> {
        match &self.backend {
            VrBackend::OpenXR(system) => system.last_render_dt_sec(),
        }
    }

    pub fn render_xr(
        &mut self,
        world: &World,
        visuals: &mut VisualWorld,
        renderer: &mut VulkanoRenderer,
    ) {
        match &mut self.backend {
            VrBackend::OpenXR(system) => system.render_xr(world, visuals, renderer),
        }
    }

    fn ensure_openxr_backend(&mut self) {
        if !matches!(self.backend, VrBackend::OpenXR(_)) {
            self.backend = VrBackend::OpenXR(OpenXRSystem::default());
        }
    }
}

impl System for VrSystem {
    fn tick(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        input: &InputState,
        dt_sec: f32,
    ) {
        match &mut self.backend {
            VrBackend::OpenXR(system) => system.tick(world, visuals, input, dt_sec),
        }
    }
}
