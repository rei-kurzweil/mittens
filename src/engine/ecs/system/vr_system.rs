use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::{VrBackendPreference, VrComponent};
use crate::engine::ecs::system::openvr_system::OpenVRSystem;
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
    OpenVR(OpenVRSystem),
}

#[derive(Debug)]
pub struct VrSystem {
    backend: VrBackend,
    preferred_backend: VrBackendKind,
    last_backend_error: Option<String>,
}

impl Default for VrSystem {
    fn default() -> Self {
        Self {
            backend: VrBackend::OpenXR(OpenXRSystem::default()),
            preferred_backend: Self::preferred_backend_from_env(),
            last_backend_error: None,
        }
    }
}

impl VrSystem {
    fn preferred_backend_from_env() -> VrBackendKind {
        match std::env::var("CAT_VR_BACKEND")
            .ok()
            .as_deref()
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("openvr") => VrBackendKind::OpenVR,
            _ => VrBackendKind::OpenXR,
        }
    }

    pub fn active_backend_kind(&self) -> VrBackendKind {
        match self.backend {
            VrBackend::OpenXR(_) => VrBackendKind::OpenXR,
            VrBackend::OpenVR(_) => VrBackendKind::OpenVR,
        }
    }

    pub fn preferred_backend_kind(&self) -> VrBackendKind {
        self.preferred_backend
    }

    pub fn last_backend_error(&self) -> Option<&str> {
        self.last_backend_error.as_deref()
    }

    pub fn xr_input_state(&self) -> &XrInputState {
        match &self.backend {
            VrBackend::OpenXR(system) => system.xr_input_state(),
            VrBackend::OpenVR(system) => system.xr_input_state(),
        }
    }

    pub fn xr_gamepad_state(&self) -> &XrGamepadState {
        match &self.backend {
            VrBackend::OpenXR(system) => system.xr_gamepad_state(),
            VrBackend::OpenVR(system) => system.xr_gamepad_state(),
        }
    }

    pub fn set_preferred_swapchain_format(&mut self, format: u32) {
        match &mut self.backend {
            VrBackend::OpenXR(system) => system.set_preferred_swapchain_format(format),
            VrBackend::OpenVR(system) => system.set_preferred_swapchain_format(format),
        }
    }

    pub fn required_vulkan_extensions(&self) -> Option<(Vec<String>, Vec<String>)> {
        match &self.backend {
            VrBackend::OpenXR(system) => system.required_vulkan_extensions(),
            VrBackend::OpenVR(system) => system.required_vulkan_extensions(),
        }
    }

    pub fn set_vulkan_graphics(&mut self, gfx: XrVulkanGraphics) {
        match &mut self.backend {
            VrBackend::OpenXR(system) => system.set_vulkan_graphics(gfx),
            VrBackend::OpenVR(system) => system.set_vulkan_graphics(gfx),
        }
    }

    pub fn register_vr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        let enabled = world
            .get_component_by_id_as::<VrComponent>(component)
            .map(|component| component.enabled)
            .unwrap_or(false);

        if let Some(component) = world.get_component_by_id_as::<VrComponent>(component) {
            self.preferred_backend = match component.backend {
                VrBackendPreference::Auto => Self::preferred_backend_from_env(),
                VrBackendPreference::OpenXR => VrBackendKind::OpenXR,
                VrBackendPreference::OpenVR => VrBackendKind::OpenVR,
            };
        }

        if enabled {
            self.ensure_preferred_backend_initialized();
        }

        match &mut self.backend {
            VrBackend::OpenXR(system) => system.register_openxr(world, visuals, component),
            VrBackend::OpenVR(system) => system.register_openxr(world, visuals, component),
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
            VrBackend::OpenVR(system) => system.register_controller_xr(world, visuals, component),
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
            VrBackend::OpenVR(system) => system.register_input_xr(world, visuals, component),
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
            VrBackend::OpenVR(system) => system.remove_controller_xr(world, visuals, component),
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
            VrBackend::OpenVR(system) => system.remove_input_xr(world, visuals, component),
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
            VrBackend::OpenVR(system) => system.tick_with_queue(world, visuals, input, emit, dt_sec),
        }
    }

    pub fn last_render_dt_sec(&self) -> Option<f32> {
        match &self.backend {
            VrBackend::OpenXR(system) => system.last_render_dt_sec(),
            VrBackend::OpenVR(system) => system.last_render_dt_sec(),
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
            VrBackend::OpenVR(system) => system.render_xr(world, visuals, renderer),
        }
    }

    fn ensure_preferred_backend_initialized(&mut self) {
        match self.preferred_backend {
            VrBackendKind::OpenXR => {
                self.ensure_openxr_backend();
                let init = match &mut self.backend {
                    VrBackend::OpenXR(system) => system.initialize_runtime(),
                    VrBackend::OpenVR(_) => unreachable!(),
                };
                match init {
                    Ok(()) => {
                        self.last_backend_error = None;
                    }
                    Err(openxr_err) => {
                        eprintln!(
                            "[VR] OpenXR initialization failed; falling back to OpenVR placeholder: {openxr_err}"
                        );
                        self.last_backend_error = Some(openxr_err);
                        self.ensure_openvr_backend();
                        let openvr_err = match &mut self.backend {
                            VrBackend::OpenVR(system) => system.initialize_runtime(),
                            VrBackend::OpenXR(_) => unreachable!(),
                        }
                        .err();
                        if let Some(openvr_err) = openvr_err {
                            self.last_backend_error = Some(format!(
                                "OpenXR failed, then OpenVR failed: {}",
                                openvr_err
                            ));
                        }
                    }
                }
            }
            VrBackendKind::OpenVR => {
                self.ensure_openvr_backend();
                if let VrBackend::OpenVR(system) = &mut self.backend {
                    if let Err(err) = system.initialize_runtime() {
                        self.last_backend_error = Some(err);
                    } else {
                        self.last_backend_error = None;
                    }
                }
            }
        }
    }

    fn ensure_openxr_backend(&mut self) {
        if !matches!(self.backend, VrBackend::OpenXR(_)) {
            self.backend = VrBackend::OpenXR(OpenXRSystem::default());
        }
    }

    fn ensure_openvr_backend(&mut self) {
        if !matches!(self.backend, VrBackend::OpenVR(_)) {
            self.backend = VrBackend::OpenVR(OpenVRSystem::default());
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
            VrBackend::OpenVR(system) => system.tick(world, visuals, input, dt_sec),
        }
    }
}
