use super::Component;
use crate::engine::ecs::ComponentId;
use slotmap::{Key, KeyData};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionMethod {
    Noop,
    Print,
    SetColor,
    OscillatorSetEnabled,
    OscillatorSetPitch,
    OscillatorScheduleSetPitch,
    OscillatorScheduleSetNote,
    OscillatorScheduleMusicNote,
    MusicSetNote,
    /// Placeholder for future unification with the command queue.
    ///
    /// Encoded as: method="command_queue", command_name="...".
    CommandQueue {
        command_name: String,
    },
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
            ActionMethod::OscillatorSetEnabled => {
                map.insert(
                    "method".to_string(),
                    serde_json::json!("oscillator_set_enabled"),
                );
            }
            ActionMethod::OscillatorSetPitch => {
                map.insert(
                    "method".to_string(),
                    serde_json::json!("oscillator_set_pitch"),
                );
            }
            ActionMethod::OscillatorScheduleSetPitch => {
                map.insert(
                    "method".to_string(),
                    serde_json::json!("oscillator_schedule_set_pitch"),
                );
            }
            ActionMethod::OscillatorScheduleSetNote => {
                map.insert(
                    "method".to_string(),
                    serde_json::json!("oscillator_schedule_set_note"),
                );
            }
            ActionMethod::OscillatorScheduleMusicNote => {
                map.insert(
                    "method".to_string(),
                    serde_json::json!("oscillator_schedule_music_note"),
                );
            }
            ActionMethod::MusicSetNote => {
                map.insert("method".to_string(), serde_json::json!("music_set_note"));
            }
            ActionMethod::CommandQueue { command_name } => {
                map.insert("method".to_string(), serde_json::json!("command_queue"));
                map.insert("command_name".to_string(), serde_json::json!(command_name));
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

    pub fn oscillator_set_enabled(target: Vec<ComponentId>, enabled: bool) -> Self {
        Self {
            target,
            method: ActionMethod::OscillatorSetEnabled,
            params: vec![serde_json::json!(enabled)],
        }
    }

    /// Set oscillator frequency directly (in Hz).
    pub fn oscillator_set_pitch(target: Vec<ComponentId>, frequency_hz: f32) -> Self {
        Self {
            target,
            method: ActionMethod::OscillatorSetPitch,
            params: vec![serde_json::json!(frequency_hz)],
        }
    }

    /// Schedule an oscillator frequency set (Hz) at a beat offset.
    ///
    /// The `beat` parameter is interpreted as an offset relative to the `beat_now`
    /// value passed to `ActionSystem::execute(...)`.
    pub fn oscillator_schedule_set_pitch(
        target: Vec<ComponentId>,
        beat: f64,
        frequency_hz: f32,
    ) -> Self {
        Self {
            target,
            method: ActionMethod::OscillatorScheduleSetPitch,
            params: vec![serde_json::json!(beat), serde_json::json!(frequency_hz)],
        }
    }

    /// Schedule an oscillator to play a musical note at a beat offset.
    ///
    /// `note.duration_beats()` is interpreted in beats, and will schedule a note-off
    /// (disable) at `beat + duration`.
    ///
    /// The `beat` parameter is interpreted as an offset relative to the `beat_now`
    /// value passed to `ActionSystem::execute(...)`.
    pub fn oscillator_schedule_music_note(
        target: Vec<ComponentId>,
        beat: f64,
        note: crate::engine::ecs::component::MusicNote,
    ) -> Self {
        Self {
            target,
            method: ActionMethod::OscillatorScheduleMusicNote,
            params: vec![serde_json::json!(beat), serde_json::json!(note)],
        }
    }

    /// Update the first `MusicNoteComponent` found under each target oscillator (subtree search),
    /// and re-apply its pitch/octave to the oscillator frequency.
    pub fn music_set_note(
        target: Vec<ComponentId>,
        note: crate::engine::ecs::component::MusicNote,
    ) -> Self {
        Self {
            target,
            method: ActionMethod::MusicSetNote,
            params: vec![serde_json::json!(note)],
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
            "oscillator_set_enabled" => ActionMethod::OscillatorSetEnabled,
            // Deprecated/removed: keep backward compatibility but do nothing.
            "oscillator_multiply_pitch" => ActionMethod::Noop,
            "oscillator_set_pitch" => ActionMethod::OscillatorSetPitch,
            // Deprecated/removed: keep backward compatibility but do nothing.
            "oscillator_schedule_multiply_pitch" => ActionMethod::Noop,
            "oscillator_schedule_set_pitch" => ActionMethod::OscillatorScheduleSetPitch,
            "oscillator_schedule_set_note" => ActionMethod::OscillatorScheduleSetNote,
            "oscillator_schedule_music_note" => ActionMethod::OscillatorScheduleMusicNote,
            "music_set_note" => ActionMethod::MusicSetNote,
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
