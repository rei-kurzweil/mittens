use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::system::vr_types::{XrGamepadState, XrInputState};
use crate::engine::graphics::{VisualWorld, VulkanoRenderer, XrVulkanGraphics};
use crate::engine::user_input::InputState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VrBackendKind {
    OpenXR,
    OpenVR,
}

impl std::fmt::Display for VrBackendKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OpenXR => write!(f, "OpenXR"),
            Self::OpenVR => write!(f, "OpenVR"),
        }
    }
}

pub trait VrBackend: std::fmt::Debug {
    fn kind(&self) -> VrBackendKind;
    fn initialize_runtime(&mut self) -> Result<(), String>;
    fn last_init_error(&self) -> Option<&str>;
    fn xr_input_state(&self) -> &XrInputState;
    fn xr_gamepad_state(&self) -> &XrGamepadState;
    fn set_preferred_swapchain_format(&mut self, format: u32);
    fn required_vulkan_extensions(&self) -> Option<(Vec<String>, Vec<String>)>;
    fn set_vulkan_graphics(&mut self, gfx: XrVulkanGraphics);
    fn register_vr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    );
    fn register_controller_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    );
    fn register_input_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    );
    fn remove_controller_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    );
    fn remove_input_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    );
    fn tick_with_queue(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        input: &InputState,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        dt_sec: f32,
    );
    fn last_render_dt_sec(&self) -> Option<f32>;
    fn render_xr(
        &mut self,
        world: &World,
        visuals: &mut VisualWorld,
        renderer: &mut VulkanoRenderer,
    );
    fn tick(&mut self, world: &mut World, visuals: &mut VisualWorld, input: &InputState, dt_sec: f32);
}
