use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::system::System;
use crate::engine::ecs::system::vr_backend::{VrBackend, VrBackendKind};
use crate::engine::ecs::system::vr_types::{XrGamepadState, XrInputState};
use crate::engine::graphics::{VisualWorld, VulkanoRenderer, XrVulkanGraphics};
use crate::engine::user_input::InputState;

#[derive(Debug, Default)]
pub struct OpenVRSystem {
    last_init_error: Option<String>,
    did_log_unavailable: bool,
    xr_input_state: XrInputState,
    xr_gamepad_state: XrGamepadState,
}

impl OpenVRSystem {
    pub fn initialize_runtime(&mut self) -> Result<(), String> {
        let err = "OpenVR backend is not implemented yet".to_string();
        self.last_init_error = Some(err.clone());
        if !self.did_log_unavailable {
            eprintln!("[OpenVR] {err}");
            self.did_log_unavailable = true;
        }
        Err(err)
    }

    pub fn last_init_error(&self) -> Option<&str> {
        self.last_init_error.as_deref()
    }

    pub fn xr_input_state(&self) -> &XrInputState {
        &self.xr_input_state
    }

    pub fn xr_gamepad_state(&self) -> &XrGamepadState {
        &self.xr_gamepad_state
    }

    pub fn set_preferred_swapchain_format(&mut self, _format: u32) {}

    pub fn required_vulkan_extensions(&self) -> Option<(Vec<String>, Vec<String>)> {
        None
    }

    pub fn set_vulkan_graphics(&mut self, _gfx: XrVulkanGraphics) {}

    pub fn register_vr(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _component: ComponentId,
    ) {
    }

    pub fn register_controller_xr(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _component: ComponentId,
    ) {
    }

    pub fn register_input_xr(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _component: ComponentId,
    ) {
    }

    pub fn remove_controller_xr(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _component: ComponentId,
    ) {
    }

    pub fn remove_input_xr(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _component: ComponentId,
    ) {
    }

    pub fn tick_with_queue(
        &mut self,
        _world: &mut World,
        visuals: &mut VisualWorld,
        _input: &InputState,
        _emit: &mut dyn crate::engine::ecs::SignalEmitter,
        _dt_sec: f32,
    ) {
        self.xr_input_state = XrInputState::default();
        self.xr_gamepad_state = XrGamepadState::default();
        visuals.set_xr_frame_dt_sec(None);
    }

    pub fn last_render_dt_sec(&self) -> Option<f32> {
        None
    }

    pub fn render_xr(
        &mut self,
        _world: &World,
        visuals: &mut VisualWorld,
        _renderer: &mut VulkanoRenderer,
    ) {
        visuals.set_xr_frame_dt_sec(None);
    }
}

impl VrBackend for OpenVRSystem {
    fn kind(&self) -> VrBackendKind {
        VrBackendKind::OpenVR
    }

    fn initialize_runtime(&mut self) -> Result<(), String> {
        OpenVRSystem::initialize_runtime(self)
    }

    fn last_init_error(&self) -> Option<&str> {
        OpenVRSystem::last_init_error(self)
    }

    fn xr_input_state(&self) -> &XrInputState {
        OpenVRSystem::xr_input_state(self)
    }

    fn xr_gamepad_state(&self) -> &XrGamepadState {
        OpenVRSystem::xr_gamepad_state(self)
    }

    fn set_preferred_swapchain_format(&mut self, format: u32) {
        OpenVRSystem::set_preferred_swapchain_format(self, format)
    }

    fn required_vulkan_extensions(&self) -> Option<(Vec<String>, Vec<String>)> {
        OpenVRSystem::required_vulkan_extensions(self)
    }

    fn set_vulkan_graphics(&mut self, gfx: XrVulkanGraphics) {
        OpenVRSystem::set_vulkan_graphics(self, gfx)
    }

    fn register_vr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        OpenVRSystem::register_vr(self, world, visuals, component)
    }

    fn register_controller_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        OpenVRSystem::register_controller_xr(self, world, visuals, component)
    }

    fn register_input_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        OpenVRSystem::register_input_xr(self, world, visuals, component)
    }

    fn remove_controller_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        OpenVRSystem::remove_controller_xr(self, world, visuals, component)
    }

    fn remove_input_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        OpenVRSystem::remove_input_xr(self, world, visuals, component)
    }

    fn tick_with_queue(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        input: &InputState,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        dt_sec: f32,
    ) {
        OpenVRSystem::tick_with_queue(self, world, visuals, input, emit, dt_sec)
    }

    fn last_render_dt_sec(&self) -> Option<f32> {
        OpenVRSystem::last_render_dt_sec(self)
    }

    fn render_xr(
        &mut self,
        world: &World,
        visuals: &mut VisualWorld,
        renderer: &mut VulkanoRenderer,
    ) {
        OpenVRSystem::render_xr(self, world, visuals, renderer)
    }

    fn tick(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        input: &InputState,
        dt_sec: f32,
    ) {
        System::tick(self, world, visuals, input, dt_sec)
    }
}

impl System for OpenVRSystem {
    fn tick(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
    }
}
