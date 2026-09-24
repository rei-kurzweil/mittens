use super::{Component, ComponentRef};
use crate::engine::ecs::ComponentId;

/// Active linear motion state. The system integrates parent-local velocity
/// into this pose driver's immediate child transform.
#[derive(Debug, Clone, Default)]
pub struct VelocityComponent {
    pub enabled: bool,
    pub linear_local_mps: [f32; 3],
    pub rotation_basis: Option<ComponentRef>,
    pub horizontal: bool,
    pub component_id: Option<ComponentId>,
}

impl VelocityComponent {
    pub fn new() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }

    pub fn set_linear_local(&mut self, value_mps: [f32; 3]) -> Result<(), &'static str> {
        if !value_mps.iter().all(|v| v.is_finite()) {
            return Err("linear velocity must be finite");
        }
        self.linear_local_mps = value_mps;
        Ok(())
    }

    pub fn zero_linear(&mut self) {
        self.linear_local_mps = [0.0; 3];
    }
}

impl Component for VelocityComponent {
    fn name(&self) -> &'static str {
        "velocity"
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

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::{CeBuilder, ce};
        use crate::scripting::ast::Expression;

        let mut ce = ce("Velocity");
        if let Some(source) = &self.rotation_basis {
            let selector = match source {
                ComponentRef::Guid(guid) => format!("@uuid:{guid}"),
                ComponentRef::Query(query) => query.clone(),
            };
            ce = ce.with_call("rotation_basis", vec![Expression::String(selector)]);
        }
        if self.horizontal {
            ce = ce.with_call("horizontal", vec![]);
        }
        if !self.enabled {
            ce = ce.with_call("enabled", vec![Expression::Bool(false)]);
        }
        ce
    }
}
