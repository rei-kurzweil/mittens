use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::system::System;
use crate::engine::ecs::system::openxr_system::{XrGamepadState, XrInputState};
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

    pub fn register_openxr(
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
