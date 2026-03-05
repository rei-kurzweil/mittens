use super::Component;
use crate::engine::ecs::ComponentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationState {
    Playing,
    Looping,
    Paused,
}

#[derive(Debug, Clone, Copy)]
pub struct AnimationComponent {
    pub state: AnimationState,

    component: Option<ComponentId>,
}

impl AnimationComponent {
    pub fn new() -> Self {
        Self {
            state: AnimationState::Looping,
            component: None,
        }
    }

    pub fn with_state(mut self, state: AnimationState) -> Self {
        self.state = state;
        self
    }

    /// Backward-compatible helper for older callers.
    pub fn with_playing(self, playing: bool) -> Self {
        self.with_state(if playing {
            AnimationState::Looping
        } else {
            AnimationState::Paused
        })
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.component
    }
}

impl Default for AnimationComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for AnimationComponent {
    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn name(&self) -> &'static str {
        "animation"
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push(
            component,
            crate::engine::ecs::SignalValue::RegisterAnimation { component },
        );
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        let state = match self.state {
            AnimationState::Playing => "playing",
            AnimationState::Looping => "looping",
            AnimationState::Paused => "paused",
        };
        map.insert("state".to_string(), serde_json::json!(state));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        // New schema.
        if let Some(state) = data.get("state").and_then(|v| v.as_str()) {
            self.state = match state {
                "playing" => AnimationState::Playing,
                "looping" => AnimationState::Looping,
                "paused" => AnimationState::Paused,
                other => return Err(format!("Unknown animation state: {}", other)),
            };
            return Ok(());
        }

        // Backward compatibility: old schema used { playing: bool }.
        if let Some(playing) = data.get("playing") {
            let playing: bool = serde_json::from_value(playing.clone())
                .map_err(|e| format!("Failed to decode playing: {}", e))?;
            self.state = if playing {
                AnimationState::Looping
            } else {
                AnimationState::Paused
            };
        }
        Ok(())
    }
}
