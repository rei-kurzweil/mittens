//! Marks externally supplied pose targets (scripts, telemetry, replay), without polling hardware.
use super::Component;
use crate::engine::ecs::{ComponentId, SignalEmitter};
#[derive(Debug, Clone, Default)]
pub struct PoseInputComponent;
impl Component for PoseInputComponent {
    fn name(&self) -> &'static str {
        "pose_input"
    }
    fn set_id(&mut self, _: ComponentId) {}
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn init(&mut self, _: &mut dyn SignalEmitter, _: ComponentId) {}
    fn to_mms_ast(
        &self,
        _: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        super::ce_helpers::ce("PoseInput")
    }
}
