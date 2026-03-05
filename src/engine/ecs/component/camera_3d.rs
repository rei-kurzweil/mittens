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

    /// Vertical field of view (degrees).
    pub fov_y_degrees: f32,
    pub z_near: f32,
    pub z_far: f32,
}

impl Camera3DComponent {
    pub const DEFAULT_FOV_Y_DEGREES: f32 = 60.0;
    pub const DEFAULT_Z_NEAR: f32 = 0.1;
    pub const DEFAULT_Z_FAR: f32 = 150.0;

    pub fn new() -> Self {
        Self {
            handle: None,
            component_id: None,
            target: CameraTarget::Window,
            fov_y_degrees: Self::DEFAULT_FOV_Y_DEGREES,
            z_near: Self::DEFAULT_Z_NEAR,
            z_far: Self::DEFAULT_Z_FAR,
        }
    }

    pub fn with_fov(mut self, fov_y_degrees: f32) -> Self {
        self.fov_y_degrees = fov_y_degrees;
        self
    }

    pub fn with_near(mut self, z_near: f32) -> Self {
        self.z_near = z_near;
        self
    }

    pub fn with_far(mut self, z_far: f32) -> Self {
        self.z_far = z_far;
        self
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

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        self.component_id = Some(component);
        emit.push(
            component,
            crate::engine::ecs::SignalValue::RegisterCamera3d { component },
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

        map.insert(
            "fov_y_deg".to_string(),
            serde_json::Value::Number(
                serde_json::Number::from_f64(self.fov_y_degrees as f64).unwrap_or_else(|| {
                    serde_json::Number::from_f64(Self::DEFAULT_FOV_Y_DEGREES as f64).unwrap()
                }),
            ),
        );
        map.insert(
            "z_near".to_string(),
            serde_json::Value::Number(
                serde_json::Number::from_f64(self.z_near as f64).unwrap_or_else(|| {
                    serde_json::Number::from_f64(Self::DEFAULT_Z_NEAR as f64).unwrap()
                }),
            ),
        );
        map.insert(
            "z_far".to_string(),
            serde_json::Value::Number(
                serde_json::Number::from_f64(self.z_far as f64).unwrap_or_else(|| {
                    serde_json::Number::from_f64(Self::DEFAULT_Z_FAR as f64).unwrap()
                }),
            ),
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

        if let Some(v) = _data.get("fov_y_deg") {
            self.fov_y_degrees = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode fov_y_deg: {}", e))?;
        }
        if let Some(v) = _data.get("z_near") {
            self.z_near = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode z_near: {}", e))?;
        }
        if let Some(v) = _data.get("z_far") {
            self.z_far = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode z_far: {}", e))?;
        }

        // Basic sanity: keep near/far in a valid ordering.
        if self.z_far <= self.z_near {
            self.z_far = self.z_near + 0.01;
        }
        Ok(())
    }
}
