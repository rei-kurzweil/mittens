use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Persistent world-space acceleration for descendant Velocity components.
/// The nearest Gravity wins, including disabled overrides. Every intervening
/// Velocity is a scope boundary; providers never store private falling speed.
#[derive(Debug, Clone)]
pub struct GravityComponent {
    pub enabled: bool,

    /// Multiplier applied to the system gravity (m/s^2).
    ///
    /// - `1.0` = earth-like gravity
    /// - `0.0` = no gravity
    /// - negative values invert gravity
    pub coefficient: f32,

    component: Option<ComponentId>,
}

impl GravityComponent {
    pub fn new() -> Self {
        Self {
            enabled: true,
            coefficient: 1.0,
            component: None,
        }
    }

    pub fn off() -> Self {
        Self {
            enabled: false,
            coefficient: 0.0,
            component: None,
        }
    }

    pub fn with_coefficient(mut self, coefficient: f32) -> Self {
        self.set_coefficient(coefficient)
            .expect("gravity coefficient must be finite");
        self
    }

    pub fn set_coefficient(&mut self, coefficient: f32) -> Result<(), &'static str> {
        if !coefficient.is_finite() {
            return Err("gravity coefficient must be finite");
        }
        self.coefficient = coefficient;
        Ok(())
    }
}

impl Default for GravityComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for GravityComponent {
    fn name(&self) -> &'static str {
        "gravity"
    }

    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, _emit: &mut dyn crate::engine::ecs::SignalEmitter, _component: ComponentId) {
    }

    fn cleanup(
        &mut self,
        _emit: &mut dyn crate::engine::ecs::SignalEmitter,
        _component: ComponentId,
    ) {
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        ce("Gravity")
            .with_call("enabled", vec![b(self.enabled)])
            .with_call("coefficient", vec![num(self.coefficient as f64)])
    }
}
