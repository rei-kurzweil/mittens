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
    pub enabled: bool,

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
            enabled: true,
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

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Ask the CameraSystem to make this the active camera.
    pub fn make_active_camera(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter) {
        if self.handle.is_some() {
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
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterCamera3d {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let target = match self.target {
            CameraTarget::Window => "window",
            CameraTarget::Xr => "xr",
        };
        ce("Camera3D")
            .with_call("enabled", vec![b(self.enabled)])
            .with_call("target", vec![s(target)])
            .with_call("fov", vec![num(self.fov_y_degrees as f64)])
            .with_call("near", vec![num(self.z_near as f64)])
            .with_call("far", vec![num(self.z_far as f64)])
    }
}
