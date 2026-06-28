use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::{VrBackendPreference, VrComponent};
use crate::engine::ecs::system::System;
use crate::engine::ecs::system::openvr_system::OpenVRSystem;
use crate::engine::ecs::system::openxr_system::OpenXRSystem;
use crate::engine::ecs::system::vr_backend::{VrBackend, VrBackendKind};
use crate::engine::ecs::system::vr_types::{XrGamepadState, XrInputState};
use crate::engine::graphics::{VisualWorld, VulkanoRenderer, XrVulkanGraphics};
use crate::engine::user_input::InputState;

#[derive(Debug)]
pub struct VrSystem {
    backend: Box<dyn VrBackend>,
    preferred_backend: VrBackendKind,
    last_backend_error: Option<String>,
    announced_backend: Option<VrBackendKind>,
    preferred_swapchain_format: Option<u32>,
    vulkan_graphics: Option<XrVulkanGraphics>,
}

impl Default for VrSystem {
    fn default() -> Self {
        Self {
            backend: Box::new(OpenXRSystem::default()),
            preferred_backend: Self::preferred_backend_from_env(),
            last_backend_error: None,
            announced_backend: None,
            preferred_swapchain_format: None,
            vulkan_graphics: None,
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

    fn make_backend(kind: VrBackendKind) -> Box<dyn VrBackend> {
        match kind {
            VrBackendKind::OpenXR => Box::new(OpenXRSystem::default()),
            VrBackendKind::OpenVR => Box::new(OpenVRSystem::default()),
        }
    }

    pub fn active_backend_kind(&self) -> VrBackendKind {
        self.backend.kind()
    }

    pub fn preferred_backend_kind(&self) -> VrBackendKind {
        self.preferred_backend
    }

    pub fn last_backend_error(&self) -> Option<&str> {
        self.last_backend_error.as_deref()
    }

    pub fn xr_input_state(&self) -> &XrInputState {
        self.backend.xr_input_state()
    }

    pub fn xr_gamepad_state(&self) -> &XrGamepadState {
        self.backend.xr_gamepad_state()
    }

    pub fn set_preferred_swapchain_format(&mut self, format: u32) {
        self.preferred_swapchain_format = Some(format);
        self.backend.set_preferred_swapchain_format(format);
    }

    pub fn required_vulkan_extensions(&self) -> Option<(Vec<String>, Vec<String>)> {
        self.backend.required_vulkan_extensions()
    }

    pub fn set_vulkan_graphics(&mut self, gfx: XrVulkanGraphics) {
        self.vulkan_graphics = Some(gfx);
        self.backend.set_vulkan_graphics(gfx);
    }

    pub fn prepare_for_renderer_init(&mut self, world: &World) {
        let preferred_from_world = world.all_components().find_map(|cid| {
            let component = world.get_component_by_id_as::<VrComponent>(cid)?;
            if !component.enabled {
                return None;
            }
            Some(match component.backend {
                VrBackendPreference::Auto => Self::preferred_backend_from_env(),
                VrBackendPreference::OpenXR => VrBackendKind::OpenXR,
                VrBackendPreference::OpenVR => VrBackendKind::OpenVR,
            })
        });

        if let Some(kind) = preferred_from_world {
            self.preferred_backend = kind;
            self.ensure_preferred_backend_initialized();
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

        self.backend.register_vr(world, visuals, component);
    }

    pub fn register_controller_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.backend.register_controller_xr(world, visuals, component);
    }

    pub fn register_input_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.backend.register_input_xr(world, visuals, component);
    }

    pub fn remove_controller_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.backend.remove_controller_xr(world, visuals, component);
    }

    pub fn remove_input_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.backend.remove_input_xr(world, visuals, component);
    }

    pub fn tick_with_queue(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        input: &InputState,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        dt_sec: f32,
    ) {
        self.backend
            .tick_with_queue(world, visuals, input, emit, dt_sec);
    }

    pub fn last_render_dt_sec(&self) -> Option<f32> {
        self.backend.last_render_dt_sec()
    }

    pub fn render_xr(
        &mut self,
        world: &World,
        visuals: &mut VisualWorld,
        renderer: &mut VulkanoRenderer,
    ) {
        self.backend.render_xr(world, visuals, renderer);
    }

    fn switch_backend(&mut self, kind: VrBackendKind) {
        if self.backend.kind() != kind {
            self.backend = Self::make_backend(kind);
            if let Some(format) = self.preferred_swapchain_format {
                self.backend.set_preferred_swapchain_format(format);
            }
            if let Some(gfx) = self.vulkan_graphics {
                self.backend.set_vulkan_graphics(gfx);
            }
        }
    }

    fn announce_backend(&mut self, context: &str) {
        let active_backend = self.backend.kind();
        if self.announced_backend == Some(active_backend) {
            return;
        }

        println!(
            "[VR] Using {active_backend} backend ({context}; requested: {})",
            self.preferred_backend
        );
        self.announced_backend = Some(active_backend);
    }

    fn ensure_preferred_backend_initialized(&mut self) {
        match self.preferred_backend {
            VrBackendKind::OpenXR => {
                self.switch_backend(VrBackendKind::OpenXR);
                match self.backend.initialize_runtime() {
                    Ok(()) => {
                        self.last_backend_error = None;
                        self.announce_backend("initialized successfully");
                    }
                    Err(openxr_err) => {
                        eprintln!(
                            "[VR] OpenXR initialization failed; falling back to OpenVR placeholder: {openxr_err}"
                        );
                        self.last_backend_error = Some(openxr_err);
                        self.switch_backend(VrBackendKind::OpenVR);
                        let openvr_err = self.backend.initialize_runtime().err();
                        if let Some(openvr_err) = openvr_err {
                            self.last_backend_error =
                                Some(format!("OpenXR failed, then OpenVR failed: {}", openvr_err));
                        } else {
                            self.last_backend_error = None;
                            self.announce_backend("fallback after OpenXR initialization failure");
                        }
                    }
                }
            }
            VrBackendKind::OpenVR => {
                self.switch_backend(VrBackendKind::OpenVR);
                if let Err(err) = self.backend.initialize_runtime() {
                    self.last_backend_error = Some(err);
                } else {
                    self.last_backend_error = None;
                    self.announce_backend("initialized successfully");
                }
            }
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
        self.backend.tick(world, visuals, input, dt_sec);
    }
}
