use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicsBodyMode {
    /// Kinematic "slide" resolution: push the body out of static overlaps using AABB penetration.
    KinematicSlide,
}

/// Opt-in physics behavior for a transform subtree.
///
/// Attach this as a direct child of a `TransformComponent`. If that transform also has a
/// `CollisionComponent` child, the PhysicsSystem may move the transform to resolve overlaps.
#[derive(Debug, Clone)]
pub struct PhysicsBodyComponent {
    pub enabled: bool,
    pub mode: PhysicsBodyMode,

    /// Max number of correction iterations per tick (helps resolve corner cases).
    pub max_iterations: u32,

    /// Small extra push-out added on separation to avoid jittering on exact contact.
    pub push_out_epsilon: f32,

    component: Option<ComponentId>,
}

impl PhysicsBodyComponent {
    pub fn new(mode: PhysicsBodyMode) -> Self {
        Self {
            enabled: true,
            mode,
            max_iterations: 6,
            push_out_epsilon: 0.001,
            component: None,
        }
    }

    pub fn kinematic_slide() -> Self {
        Self::new(PhysicsBodyMode::KinematicSlide)
    }
}

impl Default for PhysicsBodyComponent {
    fn default() -> Self {
        Self::kinematic_slide()
    }
}

impl Component for PhysicsBodyComponent {
    fn name(&self) -> &'static str {
        "physics_body"
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

    fn init(&mut self, queue: &mut crate::engine::ecs::CommandQueue, component: ComponentId) {
        queue.queue_register_physics_body(component);
    }

    fn cleanup(&mut self, queue: &mut crate::engine::ecs::CommandQueue, component: ComponentId) {
        queue.queue_remove_physics_body(component);
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("enabled".to_string(), serde_json::json!(self.enabled));
        map.insert(
            "mode".to_string(),
            serde_json::json!(match self.mode {
                PhysicsBodyMode::KinematicSlide => "kinematic_slide",
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
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(enabled) = data.get("enabled") {
            self.enabled = serde_json::from_value(enabled.clone())
                .map_err(|e| format!("Failed to decode physics_body.enabled: {e}"))?;
        }
        if let Some(mode) = data.get("mode") {
            let mode_str: String = serde_json::from_value(mode.clone())
                .map_err(|e| format!("Failed to decode physics_body.mode: {e}"))?;
            self.mode = match mode_str.as_str() {
                "kinematic_slide" => PhysicsBodyMode::KinematicSlide,
                other => return Err(format!("Unknown physics body mode: {other}")),
            };
        }
        if let Some(max_it) = data.get("max_iterations") {
            self.max_iterations = serde_json::from_value(max_it.clone())
                .map_err(|e| format!("Failed to decode physics_body.max_iterations: {e}"))?;
        }
        if let Some(eps) = data.get("push_out_epsilon") {
            self.push_out_epsilon = serde_json::from_value(eps.clone())
                .map_err(|e| format!("Failed to decode physics_body.push_out_epsilon: {e}"))?;
        }
        Ok(())
    }
}
