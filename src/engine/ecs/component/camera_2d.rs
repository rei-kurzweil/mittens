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
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterCamera2d {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let target = match self.target {
            CameraTarget::Window => "window",
            CameraTarget::Xr => "xr",
        };
        ce("Camera2D").with_call("target", vec![s(target)])
    }
}
