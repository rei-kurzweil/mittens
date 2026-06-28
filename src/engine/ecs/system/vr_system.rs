use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::{VrBackendPreference, VrComponent};
use crate::engine::ecs::system::System;
use crate::engine::ecs::system::openxr_system::OpenXRSystem;
use crate::engine::ecs::system::vr_backend::{VrBackend, VrBackendKind};
use crate::engine::ecs::system::vr_types::{XrGamepadState, XrInputState};
use crate::engine::graphics::{VisualWorld, VulkanoRenderer, XrVulkanGraphics};
use crate::engine::user_input::InputState;

#[derive(Debug)]
pub struct VrSystem {
    backend: Box<dyn VrBackend>,
    last_backend_error: Option<String>,
    announced_backend: Option<VrBackendKind>,
    preferred_swapchain_format: Option<u32>,
    vulkan_graphics: Option<XrVulkanGraphics>,
}

impl Default for VrSystem {
    fn default() -> Self {
        Self {
            backend: Box::new(OpenXRSystem::default()),
            last_backend_error: None,
            announced_backend: None,
            preferred_swapchain_format: None,
            vulkan_graphics: None,
        }
    }
}

impl VrSystem {
    pub fn active_backend_kind(&self) -> VrBackendKind {
        self.backend.kind()
    }

    pub fn preferred_backend_kind(&self) -> VrBackendKind {
        VrBackendKind::OpenXR
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
        let should_enable = world.all_components().any(|cid| {
            world
                .get_component_by_id_as::<VrComponent>(cid)
                .map(|component| {
                    component.enabled
                        && matches!(
                            component.backend,
                            VrBackendPreference::Auto | VrBackendPreference::OpenXR
                        )
                })
                .unwrap_or(false)
        });

        if should_enable {
            self.ensure_openxr_initialized();
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

        if enabled {
            self.ensure_openxr_initialized();
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

    fn announce_backend(&mut self, context: &str) {
        let active_backend = self.backend.kind();
        if self.announced_backend == Some(active_backend) {
            return;
        }

        println!("[VR] Using {active_backend} backend ({context})");
        self.announced_backend = Some(active_backend);
    }

    fn ensure_openxr_initialized(&mut self) {
        match self.backend.initialize_runtime() {
            Ok(()) => {
                self.last_backend_error = None;
                self.announce_backend("initialized successfully");
            }
            Err(err) => {
                self.last_backend_error = Some(err);
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
