use super::Component;

/// Selects a camera-dependent settings transform for its parent transform anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformCameraSpecificMode {
    Monoscopic,
    Stereoscopic,
}

#[derive(Debug, Clone, Copy)]
pub struct TransformCameraSpecificComponent {
    pub mode: TransformCameraSpecificMode,
}

impl TransformCameraSpecificComponent {
    pub fn active_monoscopic() -> Self {
        Self {
            mode: TransformCameraSpecificMode::Monoscopic,
        }
    }

    pub fn active_stereoscopic() -> Self {
        Self {
            mode: TransformCameraSpecificMode::Stereoscopic,
        }
    }
}

impl Component for TransformCameraSpecificComponent {
    fn name(&self) -> &'static str {
        "transform_camera_specific"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::ce_call;
        let ctor = match self.mode {
            TransformCameraSpecificMode::Monoscopic => "active_monoscopic",
            TransformCameraSpecificMode::Stereoscopic => "active_stereoscopic",
        };
        ce_call("TransformCameraSpecific", ctor, vec![])
    }
}
