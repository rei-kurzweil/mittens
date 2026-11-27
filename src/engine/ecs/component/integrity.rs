use super::Component;

/// How intact a part is, by damage received (not by volume). Damage impacts
/// whose sphere reaches within `reach` of the nearest transform above this
/// component lower `current` by their amount. Each named threshold (a
/// fraction of `max`) emits one `DataEvent` with its name on this component
/// the first time integrity falls to it; `repair` restores full integrity
/// and re-arms them. What a threshold means (smoke, a dead wheel, a wreck)
/// is the owner's decision, not the engine's.
#[derive(Debug, Clone, PartialEq)]
pub struct IntegrityComponent {
    pub max: f32,
    pub current: f32,
    pub reach: f32,
    /// (name, fraction of max, already crossed)
    pub thresholds: Vec<(String, f32, bool)>,
    pub enabled: bool,
}

impl Default for IntegrityComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl IntegrityComponent {
    pub fn new() -> Self {
        Self {
            max: 100.0,
            current: 100.0,
            reach: 0.5,
            thresholds: Vec::new(),
            enabled: true,
        }
    }

    pub fn fraction(&self) -> f32 {
        if self.max <= 0.0 {
            return 0.0;
        }
        (self.current / self.max).clamp(0.0, 1.0)
    }

    /// Lower integrity; returns the names of thresholds crossed by this hit.
    pub fn damage(&mut self, amount: f32) -> Vec<String> {
        if !self.enabled || !amount.is_finite() || amount <= 0.0 {
            return Vec::new();
        }
        self.current = (self.current - amount).max(0.0);
        let fraction = self.fraction();
        let mut crossed = Vec::new();
        for (name, at, done) in &mut self.thresholds {
            if !*done && fraction <= *at {
                *done = true;
                crossed.push(name.clone());
            }
        }
        crossed
    }

    pub fn repair(&mut self) {
        self.current = self.max;
        for threshold in &mut self.thresholds {
            threshold.2 = false;
        }
    }
}

impl Component for IntegrityComponent {
    fn name(&self) -> &'static str {
        "integrity"
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
    ) -> crate::scripting::ast::ComponentExpression {
        use super::ce_helpers::*;
        let mut expression = ce("Integrity")
            .with_call("max", nums([self.max as f64]))
            .with_call("reach", nums([self.reach as f64]));
        for (name, at, _) in &self.thresholds {
            expression = expression.with_call("threshold", vec![s(name), num(*at as f64)]);
        }
        if !self.enabled {
            expression = expression.with_call("enabled", vec![b(false)]);
        }
        expression
    }
}
