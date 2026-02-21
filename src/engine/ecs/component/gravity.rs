use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Gravity field component.
///
/// Any `KineticResponseComponent` nested under a `GravityComponent` will have gravity applied
/// by `KineticResponseSystem`.
///
/// This component can live anywhere in the scene graph and can have arbitrary descendants.
/// If multiple `GravityComponent`s exist in the ancestor chain, the nearest enabled one wins.
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
        self.coefficient = coefficient;
        self
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

    fn init(&mut self, _queue: &mut crate::engine::ecs::CommandQueue, _component: ComponentId) {}

    fn cleanup(&mut self, _queue: &mut crate::engine::ecs::CommandQueue, _component: ComponentId) {}

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("enabled".to_string(), serde_json::json!(self.enabled));
        map.insert(
            "coefficient".to_string(),
            serde_json::json!(self.coefficient),
        );
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(enabled) = data.get("enabled") {
            self.enabled = serde_json::from_value(enabled.clone())
                .map_err(|e| format!("Failed to decode gravity.enabled: {e}"))?;
        }
        if let Some(coef) = data.get("coefficient") {
            self.coefficient = serde_json::from_value(coef.clone())
                .map_err(|e| format!("Failed to decode gravity.coefficient: {e}"))?;
        }
        Ok(())
    }
}
