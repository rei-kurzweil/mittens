use super::Component;
use crate::engine::ecs::ComponentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OscillatorType {
    Sin,
    Triangle,
    Square,
    #[serde(rename = "square_3")]
    Square3,
    Drum,
    Saw,
    Noise,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AudioOscillator {
    #[serde(rename = "type")]
    pub oscillator_type: OscillatorType,
    pub frequency: f32,
    pub amplitude: f32,
    pub enabled: bool,

    /// If true, `MusicSystem` will not overwrite this oscillator's frequency.
    /// This is set to true once a MusicNote has been applied, or after any
    /// action mutates the frequency (set/multiply pitch).
    #[serde(default)]
    pub music_note_applied: bool,
}

impl Default for AudioOscillator {
    fn default() -> Self {
        Self {
            oscillator_type: OscillatorType::Sin,
            frequency: 440.0,
            amplitude: 0.2,
            enabled: true,
            music_note_applied: false,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AudioOscillatorComponent {
    pub oscillators: Vec<AudioOscillator>,

    #[serde(skip)]
    component: Option<ComponentId>,
}

impl AudioOscillatorComponent {
    pub fn new(oscillators: Vec<AudioOscillator>) -> Self {
        Self {
            oscillators,
            component: None,
        }
    }

    pub fn single(osc: AudioOscillator) -> Self {
        Self::new(vec![osc])
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.component
    }
}

impl Default for AudioOscillatorComponent {
    fn default() -> Self {
        Self::new(vec![AudioOscillator::default()])
    }
}

impl Component for AudioOscillatorComponent {
    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn name(&self) -> &'static str {
        "audio_oscillator"
    }

    fn init(&mut self, queue: &mut crate::engine::ecs::CommandQueue, component: ComponentId) {
        queue.queue_register_audio_oscillator(component);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert(
            "oscillators".to_string(),
            serde_json::to_value(&self.oscillators).unwrap_or_else(|_| serde_json::json!([])),
        );
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("oscillators") {
            self.oscillators = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode oscillators: {}", e))?;
        }
        Ok(())
    }
}
