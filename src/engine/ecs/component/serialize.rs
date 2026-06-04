use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Save visibility marker for filtered world serialization.
///
/// Filtered world save includes components by default. `SerializeComponent::off()` excludes the
/// component subtree, while `SerializeComponent::on()` can explicitly re-include a subtree inside
/// an excluded ancestor.
#[derive(Debug, Clone, Copy)]
pub struct SerializeComponent {
    pub enabled: bool,
    component: Option<ComponentId>,
}

impl SerializeComponent {
    pub fn on() -> Self {
        Self {
            enabled: true,
            component: None,
        }
    }

    pub fn off() -> Self {
        Self {
            enabled: false,
            component: None,
        }
    }
}

impl Default for SerializeComponent {
    fn default() -> Self {
        Self::on()
    }
}

impl Component for SerializeComponent {
    fn set_id(&mut self, id: ComponentId) {
        self.component = Some(id);
    }

    fn name(&self) -> &'static str {
        "serialize"
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
        use crate::engine::ecs::component::ce_helpers::*;
        let ctor = if self.enabled { "on" } else { "off" };
        ce_call("Serialize", ctor, vec![])
    }
}
