use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerHand {
    Left,
    Right,
}

impl Default for ControllerHand {
    fn default() -> Self {
        Self::Left
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerPoseKind {
    /// A pointing pose, typically used for ray-based UI interaction.
    Aim,
    /// A “held object” pose, typically used for attaching models/tools.
    Grip,
}

impl Default for ControllerPoseKind {
    fn default() -> Self {
        Self::Aim
    }
}

/// Marker/config for an XR controller tracked pose.
///
/// Semantics:
/// - Attach under a `TransformComponent` that represents the controller root.
/// - `OpenXRSystem` will drive the nearest ancestor transform from OpenXR pose tracking.
#[derive(Debug, Clone, Default)]
pub struct ControllerXRComponent {
    pub enabled: bool,
    pub hand: ControllerHand,
    pub pose: ControllerPoseKind,

    // Cached ECS id (runtime-only). Filled during init.
    pub component_id: Option<ComponentId>,
}

impl ControllerXRComponent {
    pub fn new(enabled: bool, hand: ControllerHand, pose: ControllerPoseKind) -> Self {
        Self {
            enabled,
            hand,
            pose,
            component_id: None,
        }
    }

    pub fn on_left_aim() -> Self {
        Self::new(true, ControllerHand::Left, ControllerPoseKind::Aim)
    }

    pub fn on_right_aim() -> Self {
        Self::new(true, ControllerHand::Right, ControllerPoseKind::Aim)
    }
}

impl Component for ControllerXRComponent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        "controller_xr"
    }

    fn set_id(&mut self, component: ComponentId) {
        self.component_id = Some(component);
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        self.component_id = Some(component);
        emit.push_intent_now(component, crate::engine::ecs::IntentValue::RegisterControllerXr { component });
    }

    fn cleanup(
        &mut self,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        component: ComponentId,
    ) {
        emit.push_intent_now(component, crate::engine::ecs::IntentValue::RemoveControllerXr { component });
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("enabled".to_string(), serde_json::Value::Bool(self.enabled));
        let hand = match self.hand {
            ControllerHand::Left => "left",
            ControllerHand::Right => "right",
        };
        map.insert(
            "hand".to_string(),
            serde_json::Value::String(hand.to_string()),
        );

        let pose = match self.pose {
            ControllerPoseKind::Aim => "aim",
            ControllerPoseKind::Grip => "grip",
        };
        map.insert(
            "pose".to_string(),
            serde_json::Value::String(pose.to_string()),
        );
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("enabled") {
            self.enabled = v.as_bool().unwrap_or(false);
        }
        if let Some(v) = data.get("hand").and_then(|v| v.as_str()) {
            self.hand = match v {
                "left" => ControllerHand::Left,
                "right" => ControllerHand::Right,
                other => return Err(format!("Unknown ControllerHand: {other}")),
            };
        }
        if let Some(v) = data.get("pose").and_then(|v| v.as_str()) {
            self.pose = match v {
                "aim" => ControllerPoseKind::Aim,
                "grip" => ControllerPoseKind::Grip,
                other => return Err(format!("Unknown ControllerPoseKind: {other}")),
            };
        }
        Ok(())
    }
}
