use super::Component;

/// Opts a Velocity's body into blast pushes. A child of the Velocity, like
/// Gravity. A damage impact with a blast that reaches the body (the
/// Velocity's driven transform) adds speed away from the blast, biased
/// upward, scaled by `1 / mass`, and a turn scaled by `spin / mass`: about
/// the vertical axis, turning the body's front away, unless `tumble` lets it
/// rotate about the horizontal axis across the push.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlastableComponent {
    pub mass: f32,
    pub spin: f32,
    pub tumble: bool,
    pub enabled: bool,
}

impl Default for BlastableComponent {
    fn default() -> Self {
        Self {
            mass: 1.0,
            spin: 1.0,
            tumble: false,
            enabled: true,
        }
    }
}

impl Component for BlastableComponent {
    fn name(&self) -> &'static str {
        "blastable"
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
        let expression = ce("Blastable")
            .with_call("mass", vec![num(self.mass as f64)])
            .with_call("spin", vec![num(self.spin as f64)]);
        if self.tumble {
            expression.with_call("tumble", vec![])
        } else {
            expression
        }
    }
}
