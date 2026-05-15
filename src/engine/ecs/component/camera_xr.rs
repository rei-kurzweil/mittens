use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;
use crate::engine::graphics::CameraTarget;

#[derive(Debug, Clone, Copy)]
pub struct CameraXRComponent {
    pub enabled: bool,

    // Cached ECS id (runtime-only). Filled in during init.
    pub component_id: Option<ComponentId>,

    /// Which output this camera targets for activation.
    ///
    /// For XR, this is typically `CameraTarget::Xr` and represents the XR rig selection.
    pub target: CameraTarget,
}

impl CameraXRComponent {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            component_id: None,
            target: CameraTarget::Xr,
        }
    }

    pub fn on() -> Self {
        Self::new(true)
    }

    pub fn off() -> Self {
        Self::new(false)
    }

    /// Ask the CameraSystem to make this the active XR camera rig.
    pub fn make_active_camera(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter) {
        if let Some(component) = self.component_id {
            emit.push_intent_now(
                component,
                crate::engine::ecs::IntentValue::MakeActiveCamera {
                    component_ids: vec![component],
                },
            );
        }
    }
}

impl Default for CameraXRComponent {
    fn default() -> Self {
        Self::on()
    }
}

impl Component for CameraXRComponent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        "camera_xr"
    }

    fn init(&mut self, _emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        self.component_id = Some(component);
    }

    fn to_mms_ast(&self) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let ctor = if self.enabled { "on" } else { "off" };
        let target_str = match self.target {
            CameraTarget::Window => "window",
            CameraTarget::Xr => "xr",
        };
        let mut ce = ce_call("CameraXR", ctor, vec![]);
        if !matches!(self.target, CameraTarget::Xr) {
            ce = ce.with_call("target", vec![s(target_str)]);
        }
        ce
    }
}
