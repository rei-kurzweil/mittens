use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;
use crate::engine::graphics::CameraTarget;

/// 2D camera component.
///
/// This is a sibling of `Camera3DComponent` (3D-ish view/proj camera).
/// The 2D camera drives a global NDC translation used by the mesh vertex shader.
#[derive(Debug, Clone)]
pub struct Camera2DComponent {
    pub handle: Option<crate::engine::ecs::system::camera_system::CameraHandle>,
    // Cached ECS id (runtime-only). Filled in during init.
    pub component_id: Option<ComponentId>,
    /// Which output this camera targets for activation.
    pub target: CameraTarget,
}

impl Camera2DComponent {
    pub fn new() -> Self {
        Self {
            handle: None,
            component_id: None,
            target: CameraTarget::Window,
        }
    }

    /// Ask the CameraSystem to make this the active camera.
    pub fn make_active_camera(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter) {
        if self.handle.is_some() {
            if let Some(component) = self.component_id {
                emit.push(
                    component,
                    crate::engine::ecs::SignalValue::MakeActiveCamera { component },
                );
            }
        }
    }
}

impl Default for Camera2DComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Camera2DComponent {
    fn name(&self) -> &'static str {
        "camera2d"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        self.component_id = Some(component);
        emit.push(
            component,
            crate::engine::ecs::SignalValue::RegisterCamera2d { component },
        );
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        let target = match self.target {
            CameraTarget::Window => "window",
            CameraTarget::Xr => "xr",
        };
        map.insert(
            "target".to_string(),
            serde_json::Value::String(target.to_string()),
        );
        map
    }

    fn decode(
        &mut self,
        _data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        // Handle will be regenerated during init().
        if let Some(v) = _data.get("target") {
            if let Some(s) = v.as_str() {
                self.target = match s {
                    "xr" => CameraTarget::Xr,
                    "window" | _ => CameraTarget::Window,
                };
            }
        }
        Ok(())
    }
}
