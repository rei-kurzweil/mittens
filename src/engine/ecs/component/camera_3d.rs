use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;
use crate::engine::graphics::CameraTarget;

/// 3D camera component.
///
/// Contract:
/// - On init, registers a camera with `CameraSystem`.
/// - The most recently registered camera becomes active.
/// - Call `make_active_camera()` to explicitly set this camera active.
#[derive(Debug, Clone)]
pub struct Camera3DComponent {
    // Handle owned by CameraSystem. Filled in during init.
    pub handle: Option<crate::engine::ecs::system::camera_system::CameraHandle>,

    // Cached ECS id (runtime-only). Filled in during init.
    pub component_id: Option<ComponentId>,

    /// Which output this camera targets for activation.
    ///
    /// Notes:
    /// - Today, the 3D camera drives the Window camera matrices.
    /// - This field exists so `make_active_camera()` can be target-aware.
    pub target: CameraTarget,
}

impl Camera3DComponent {
    pub fn new() -> Self {
        Self {
            handle: None,
            component_id: None,
            target: CameraTarget::Window,
        }
    }

    /// Ask the CameraSystem to make this the active camera.
    pub fn make_active_camera(&mut self, queue: &mut crate::engine::ecs::CommandQueue) {
        if self.handle.is_some() {
            if let Some(component) = self.component_id {
                queue.queue_make_active_camera(component);
            }
        }
    }
}

impl Default for Camera3DComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Camera3DComponent {
    fn name(&self) -> &'static str {
        "camera3d"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, queue: &mut crate::engine::ecs::CommandQueue, component: ComponentId) {
        self.component_id = Some(component);
        queue.queue_register_camera_3d(component);
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
