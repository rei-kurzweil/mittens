use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Whether this provider supplies acceleration, blocks it, or inherits it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GravityMode {
    On,
    Off,
    Auto,
}
/// Persistent world-space acceleration. Explicit providers override inherited
/// gravity; Auto providers are transparent. A direct child of Velocity is a
/// body-local override. Other descendant providers do not affect their ancestors.
#[derive(Debug, Clone)]
pub struct GravityComponent {
    pub enabled: bool,
    /// Auto providers defer to the next provider, ignoring their stored values.
    pub inherit: bool,

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
            inherit: false,
            coefficient: 1.0,
            component: None,
        }
    }

    pub fn on() -> Self {
        Self::new()
    }
    pub fn off() -> Self {
        let mut gravity = Self::new();
        gravity.set_mode(GravityMode::Off);
        gravity
    }
    pub fn auto() -> Self {
        let mut gravity = Self::new();
        gravity.set_mode(GravityMode::Auto);
        gravity
    }
    pub fn mode(&self) -> GravityMode {
        if self.inherit {
            GravityMode::Auto
        } else if self.enabled {
            GravityMode::On
        } else {
            GravityMode::Off
        }
    }
    pub fn set_mode(&mut self, mode: GravityMode) {
        self.inherit = mode == GravityMode::Auto;
        match mode {
            GravityMode::On => self.enabled = true,
            GravityMode::Off => self.enabled = false,
            GravityMode::Auto => {}
        }
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.set_mode(if enabled {
            GravityMode::On
        } else {
            GravityMode::Off
        });
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
        let expression = if self.inherit {
            ce("Gravity").with_call("auto", vec![])
        } else {
            ce("Gravity").with_call("enabled", vec![b(self.enabled)])
        };
        expression.with_call("coefficient", vec![num(self.coefficient as f64)])
    }
}
