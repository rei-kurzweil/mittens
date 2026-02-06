use super::Component;
use crate::engine::ecs::ComponentId;
use slotmap::{Key, KeyData};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionMethod {
    Noop,
    Print,
    SetColor,
    /// Placeholder for future unification with the command queue.
    ///
    /// Encoded as: method="command_queue", command_name="...".
    CommandQueue { command_name: String },
}

impl ActionMethod {
    fn encode(&self, map: &mut std::collections::HashMap<String, serde_json::Value>) {
        match self {
            ActionMethod::Noop => {
                map.insert("method".to_string(), serde_json::json!("noop"));
            }
            ActionMethod::Print => {
                map.insert("method".to_string(), serde_json::json!("print"));
            }
            ActionMethod::SetColor => {
                map.insert("method".to_string(), serde_json::json!("set_color"));
            }
            ActionMethod::CommandQueue { command_name } => {
                map.insert("method".to_string(), serde_json::json!("command_queue"));
                map.insert(
                    "command_name".to_string(),
                    serde_json::json!(command_name),
                );
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Action {
    pub target: Vec<ComponentId>,
    pub method: ActionMethod,
    pub params: Vec<serde_json::Value>,
}

impl Default for Action {
    fn default() -> Self {
        Self {
            target: Vec::new(),
            method: ActionMethod::Noop,
            params: Vec::new(),
        }
    }
}

impl Action {
    pub fn print(message: impl Into<String>) -> Self {
        Self {
            target: Vec::new(),
            method: ActionMethod::Print,
            params: vec![serde_json::json!(message.into())],
        }
    }

    pub fn set_color(target: Vec<ComponentId>, rgba: [f32; 4]) -> Self {
        Self {
            target,
            method: ActionMethod::SetColor,
            params: vec![serde_json::json!(rgba)],
        }
    }
}

#[derive(Debug, Clone)]
pub struct ActionComponent {
    pub action: Action,
}

impl ActionComponent {
    pub fn new(action: Action) -> Self {
        Self { action }
    }

    pub fn print(message: impl Into<String>) -> Self {
        Self::new(Action::print(message))
    }
}

impl Default for ActionComponent {
    fn default() -> Self {
        Self {
            action: Action::default(),
        }
    }
}

impl Component for ActionComponent {
    fn name(&self) -> &'static str {
        "action"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();

        let target_ffi: Vec<u64> = self
            .action
            .target
            .iter()
            .map(|cid| cid.data().as_ffi())
            .collect();
        map.insert("target".to_string(), serde_json::json!(target_ffi));
        self.action.method.encode(&mut map);
        map.insert("params".to_string(), serde_json::json!(self.action.params));

        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        // Backward compatibility: old schema used { message: String }.
        if let Some(message) = data.get("message") {
            let msg: String = serde_json::from_value(message.clone())
                .map_err(|e| format!("Failed to decode message: {}", e))?;
            self.action = Action::print(msg);
            return Ok(());
        }

        if let Some(target) = data.get("target") {
            let target_ffi: Vec<u64> = serde_json::from_value(target.clone())
                .map_err(|e| format!("Failed to decode target: {}", e))?;
            self.action.target = target_ffi
                .into_iter()
                .map(|ffi| KeyData::from_ffi(ffi).into())
                .collect();
        }

        if let Some(params) = data.get("params") {
            self.action.params = serde_json::from_value(params.clone())
                .map_err(|e| format!("Failed to decode params: {}", e))?;
        }

        let method = data
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("noop");

        self.action.method = match method {
            "noop" => ActionMethod::Noop,
            "print" => ActionMethod::Print,
            "set_color" => ActionMethod::SetColor,
            "command_queue" => ActionMethod::CommandQueue {
                command_name: data
                    .get("command_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            },
            other => {
                return Err(format!("Unknown action method: {}", other));
            }
        };

        Ok(())
    }
}
