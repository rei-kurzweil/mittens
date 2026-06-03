use crate::engine::ecs::component::Component;
use crate::engine::ecs::ComponentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KineticResponseMode {
    /// Kinematic "slide" resolution: push the body out of static overlaps using AABB penetration.
    Slide,

    /// Kinematic "push" behavior: accumulate velocity away from overlapping non-static colliders,
    /// and integrate it every tick (still resolves against static overlaps).
    Push,
}

/// Opt-in kinematic collision response behavior for a collider.
///
/// Preferred topology is nesting under the collider:
///
/// `TransformComponent -> CollisionComponent -> KineticResponseComponent`
#[derive(Debug, Clone)]
pub struct KineticResponseComponent {
    pub enabled: bool,
    pub mode: KineticResponseMode,

    /// Max number of correction iterations per tick (helps resolve corner cases).
    pub max_iterations: u32,

    /// Small extra push-out added on separation to avoid jittering on exact contact.
    pub push_out_epsilon: f32,

    /// Strength of velocity acceleration applied from overlapping non-static colliders.
    pub push_strength: f32,

    /// Optional friction-like damping applied to velocity each second.
    /// Off by default.
    /// Effective multiplier per tick is `max(0, 1 - friction * dt)`.
    pub friction: f32,

    /// Optional additional damping applied to the Y velocity component only when resolving
    /// a vertical (Y-axis) static overlap.
    /// Off by default.
    /// Effective multiplier per contact tick is `max(0, 1 - friction_y * dt)`.
    pub friction_y: f32,

    /// Clamp on velocity magnitude (world units / sec).
    pub max_speed: f32,

    /// Runtime-only velocity accumulator (not serialized).
    pub velocity: [f32; 3],

    /// Runtime-only cached gravity coefficient from the nearest enabled `GravityComponent`
    /// ancestor.
    ///
    /// This is set by `KineticResponseSystem` when the component is registered.
    pub gravity_coefficient: f32,

    component: Option<ComponentId>,
}

impl KineticResponseComponent {
    pub fn new(mode: KineticResponseMode) -> Self {
        Self {
            enabled: true,
            mode,
            max_iterations: 6,
            push_out_epsilon: 0.001,
            push_strength: 4.0,
            friction: 0.0,
            friction_y: 0.0,
            max_speed: 6.0,
            velocity: [0.0, 0.0, 0.0],
            gravity_coefficient: 0.0,
            component: None,
        }
    }

    pub fn slide() -> Self {
        let mut c = Self::new(KineticResponseMode::Slide);
        c.push_strength = 0.0;
        c.friction = 0.0;
        c.max_speed = 0.0;
        c
    }

    pub fn push() -> Self {
        Self::new(KineticResponseMode::Push)
    }

    pub fn with_push_strength(mut self, push_strength: f32) -> Self {
        self.push_strength = push_strength;
        self
    }

    pub fn with_friction(mut self, friction: f32) -> Self {
        self.friction = friction.max(0.0);
        self
    }

    pub fn with_friction_y(mut self, friction_y: f32) -> Self {
        self.friction_y = friction_y.max(0.0);
        self
    }
}

impl Default for KineticResponseComponent {
    fn default() -> Self {
        Self::slide()
    }
}

impl Component for KineticResponseComponent {
    fn name(&self) -> &'static str {
        "kinetic_response"
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

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterKineticResponse {
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
            crate::engine::ecs::IntentValue::RemoveKineticResponse {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let ctor = match self.mode {
            KineticResponseMode::Slide => "slide",
            KineticResponseMode::Push => "push",
        };
        ce_call("KineticResponse", ctor, vec![])
            .with_call("enabled", vec![b(self.enabled)])
            .with_call("max_iterations", vec![num(self.max_iterations as f64)])
            .with_call("push_out_epsilon", vec![num(self.push_out_epsilon as f64)])
            .with_call("push_strength", vec![num(self.push_strength as f64)])
            .with_call("friction", vec![num(self.friction as f64)])
            .with_call("friction_y", vec![num(self.friction_y as f64)])
            .with_call("max_speed", vec![num(self.max_speed as f64)])
    }
}
