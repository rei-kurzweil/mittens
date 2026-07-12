use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Marker/config for an XR headset pose driver.
///
/// Semantics:
/// - Attach a `TransformComponent` as a child of this component.
/// - the active XR runtime will drive that transform child from the headset/root pose.
#[derive(Debug, Clone)]
pub struct InputXRComponent {
    pub enabled: bool,
    /// Runtime-only: true after a valid headset pose was applied this frame.
    pub pose_valid: bool,
    pub component_id: Option<ComponentId>,
}

impl InputXRComponent {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            pose_valid: false,
            component_id: None,
        }
    }

    pub fn on() -> Self {
        Self::new(true)
    }

    pub fn off() -> Self {
        Self::new(false)
    }
}

impl Default for InputXRComponent {
    fn default() -> Self {
        Self::on()
    }
}

impl Component for InputXRComponent {
    fn name(&self) -> &'static str {
        "input_xr"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn set_id(&mut self, component: ComponentId) {
        self.component_id = Some(component);
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        self.component_id = Some(component);
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterInputXr {
                component_ids: vec![component],
            },
        );
    }

    fn cleanup(
        &mut self,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        component: ComponentId,
    ) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RemoveInputXr {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let ctor = if self.enabled { "on" } else { "off" };
        ce_call("InputXR", ctor, vec![])
    }
}
