use super::Component;

/// A collision space for the zones beneath it, kept in the coordinates of its
/// nearest transform (usually a vehicle body). Its zones stay out of the world
/// broad phase: they are a separately built, local-space tree, so moving the
/// vehicle never refits them, and only movers whose outbound context is this
/// one collide with them. With `passthrough`, those movers also collide with
/// the world, except for zones under this context's frame (the vehicle's own
/// exterior).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollisionContextComponent {
    pub passthrough: bool,
    pub enabled: bool,
}

impl Default for CollisionContextComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl CollisionContextComponent {
    pub fn new() -> Self {
        Self {
            passthrough: true,
            enabled: true,
        }
    }

    pub fn passthrough(mut self, value: bool) -> Self {
        self.passthrough = value;
        self
    }

    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }
}

impl Component for CollisionContextComponent {
    fn name(&self) -> &'static str {
        "collision_context"
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
        let mut expression = ce("CollisionContext");
        if !self.passthrough {
            expression = expression.with_call("passthrough", vec![b(false)]);
        }
        if !self.enabled {
            expression = expression.with_call("enabled", vec![b(false)]);
        }
        expression
    }
}
