use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RayCastMode {
    Continuous,
    EventDriven,
}

/// Ray casting request/behavior.
///
/// Semantics:
/// - Attach this anywhere (commonly under a camera rig transform).
/// - The RayCastSystem will cast from the active window camera through the cursor.
/// - In `EventDriven`, casts only when left mouse is pressed this frame.
#[derive(Debug, Clone, Copy)]
pub struct RayCastComponent {
    pub mode: RayCastMode,

    /// Max ray distance in world units.
    pub max_distance: f32,

    /// Incremented by `Action::raycast(...)` to request a cast on this frame.
    ///
    /// This is intentionally not serialized; it is a transient runtime signal.
    pub cast_requests: u32,

    component: Option<ComponentId>,
}

impl RayCastComponent {
    pub fn new(mode: RayCastMode) -> Self {
        Self {
            mode,
            max_distance: 200.0,
            cast_requests: 0,
            component: None,
        }
    }

    pub fn continuous() -> Self {
        Self::new(RayCastMode::Continuous)
    }

    pub fn event_driven() -> Self {
        Self::new(RayCastMode::EventDriven)
    }

    pub fn with_max_distance(mut self, max_distance: f32) -> Self {
        self.max_distance = max_distance;
        self
    }
}

impl Default for RayCastComponent {
    fn default() -> Self {
        Self::event_driven()
    }
}

impl Component for RayCastComponent {
    fn name(&self) -> &'static str {
        "raycast"
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
        self.component = Some(component);
        emit.push_intent_now(component, crate::engine::ecs::IntentValue::RegisterRaycast { component });
    }

    fn cleanup(
        &mut self,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        component: ComponentId,
    ) {
        emit.push_intent_now(component, crate::engine::ecs::IntentValue::RemoveRaycast { component });
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        let mode = match self.mode {
            RayCastMode::Continuous => "continuous",
            RayCastMode::EventDriven => "event_driven",
        };
        map.insert("mode".to_string(), serde_json::json!(mode));
        map.insert(
            "max_distance".to_string(),
            serde_json::json!(self.max_distance),
        );
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        self.cast_requests = 0;
        if let Some(mode) = data.get("mode") {
            let mode_str: String = serde_json::from_value(mode.clone())
                .map_err(|e| format!("Failed to decode raycast mode: {}", e))?;
            self.mode = match mode_str.as_str() {
                "continuous" => RayCastMode::Continuous,
                "event_driven" => RayCastMode::EventDriven,
                other => return Err(format!("Unknown raycast mode: {}", other)),
            };
        }
        if let Some(md) = data.get("max_distance") {
            self.max_distance = serde_json::from_value(md.clone())
                .map_err(|e| format!("Failed to decode max_distance: {}", e))?;
        }
        Ok(())
    }
}
