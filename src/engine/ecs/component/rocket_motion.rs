//! Native motion providers and opt-in projectile impact observation.
use super::Component;
#[derive(Debug, Clone)]
pub struct AccelerationComponent {
    pub local: [f32; 3],
    pub max_speed: f32,
}
#[derive(Debug, Clone, Default)]
pub struct ProjectileImpactComponent;
impl Component for AccelerationComponent {
    fn name(&self) -> &'static str {
        "acceleration"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn to_mms_ast(
        &self,
        _: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use super::ce_helpers::*;
        ce_call(
            "Acceleration",
            "local",
            vec![crate::scripting::ast::Expression::Array(nums(
                self.local.into_iter().map(f64::from),
            ))],
        )
        .with_call("max_speed", vec![num(self.max_speed as f64)])
    }
}
impl Component for ProjectileImpactComponent {
    fn name(&self) -> &'static str {
        "projectile_impact"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn to_mms_ast(
        &self,
        _: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        super::ce_helpers::ce("ProjectileImpact")
    }
}
