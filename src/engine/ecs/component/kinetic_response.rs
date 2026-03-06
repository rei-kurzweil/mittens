use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

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
        emit.push_intent_now(component, crate::engine::ecs::IntentValue::RegisterKineticResponse { component });
    }

    fn cleanup(
        &mut self,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        component: ComponentId,
    ) {
        emit.push_intent_now(component, crate::engine::ecs::IntentValue::RemoveKineticResponse { component });
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("enabled".to_string(), serde_json::json!(self.enabled));
        map.insert(
            "mode".to_string(),
            serde_json::json!(match self.mode {
                KineticResponseMode::Slide => "slide",
                KineticResponseMode::Push => "push",
            }),
        );
        map.insert(
            "max_iterations".to_string(),
            serde_json::json!(self.max_iterations),
        );
        map.insert(
            "push_out_epsilon".to_string(),
            serde_json::json!(self.push_out_epsilon),
        );
        map.insert(
            "push_strength".to_string(),
            serde_json::json!(self.push_strength),
        );
        map.insert("friction".to_string(), serde_json::json!(self.friction));
        map.insert("friction_y".to_string(), serde_json::json!(self.friction_y));
        map.insert("max_speed".to_string(), serde_json::json!(self.max_speed));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(enabled) = data.get("enabled") {
            self.enabled = serde_json::from_value(enabled.clone())
                .map_err(|e| format!("Failed to decode kinetic_response.enabled: {e}"))?;
        }
        if let Some(mode) = data.get("mode") {
            let mode_str: String = serde_json::from_value(mode.clone())
                .map_err(|e| format!("Failed to decode kinetic_response.mode: {e}"))?;
            self.mode = match mode_str.as_str() {
                "slide" => KineticResponseMode::Slide,
                "push" => KineticResponseMode::Push,
                other => return Err(format!("Unknown kinetic response mode: {other}")),
            };
        }
        if let Some(max_it) = data.get("max_iterations") {
            self.max_iterations = serde_json::from_value(max_it.clone())
                .map_err(|e| format!("Failed to decode kinetic_response.max_iterations: {e}"))?;
        }
        if let Some(eps) = data.get("push_out_epsilon") {
            self.push_out_epsilon = serde_json::from_value(eps.clone())
                .map_err(|e| format!("Failed to decode kinetic_response.push_out_epsilon: {e}"))?;
        }

        if let Some(v) = data.get("push_strength") {
            self.push_strength = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode kinetic_response.push_strength: {e}"))?;
        }
        if let Some(v) = data.get("friction") {
            self.friction = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode kinetic_response.friction: {e}"))?;
        }
        if let Some(v) = data.get("friction_y") {
            self.friction_y = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode kinetic_response.friction_y: {e}"))?;
        }
        if let Some(v) = data.get("max_speed") {
            self.max_speed = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode kinetic_response.max_speed: {e}"))?;
        }
        Ok(())
    }
}
