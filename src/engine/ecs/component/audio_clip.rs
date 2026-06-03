use super::Component;
use crate::engine::ecs::ComponentId;

/// How an `AudioClipComponent` reacts to repeated `AudioSchedulePlay`
/// intents. See docs/spec/audio-sources.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioTriggerMode {
    /// Every trigger resets the cursor and replays from the start.
    Retrigger,
    /// Triggers are ignored once a clip has started playing (one-shot).
    OneShot,
    /// Triggers are ignored once the clip is playing; suited for ambient
    /// loops set to `playing: true` at scene start.
    Latched,
}

impl Default for AudioTriggerMode {
    fn default() -> Self {
        AudioTriggerMode::Retrigger
    }
}

/// Result of attempting to acquire decoded PCM data for this clip's URI.
///
/// Phase 4 stores only the URI shape and a load result. Phase 5 wires
/// `Pending` to the decode thread; for now the runtime synchronously
/// checks file existence so missing assets surface immediately and the
/// scene keeps going.
#[derive(Debug, Clone, PartialEq)]
pub enum AudioClipLoadState {
    /// Not yet inspected.
    Pending,
    /// Asset is on disk and has been claimed by the decode pipeline.
    /// Phase 4 collapses this into "URI is non-empty + file exists".
    Loaded,
    /// Load failed — either file missing, unsupported codec, or decode
    /// error. The `String` carries a one-line reason for diagnostics.
    /// Dispatch from `AudioSchedulePlay` against a Failed clip is silent;
    /// nothing crashes.
    Failed(String),
}

impl Default for AudioClipLoadState {
    fn default() -> Self {
        AudioClipLoadState::Pending
    }
}

/// PCM-backed audio source. Peer to `AudioOscillatorComponent` in the
/// unified `AudioSource` model.
///
/// Cloning model (docs/draft/audio-clip-instance-cloning.md):
/// `source_component = Some(other)` marks this clip as an instance
/// sharing `other`'s decoded buffer. Each instance still has its own
/// playhead on the RT side; only the asset is shared.
#[derive(Debug, Clone)]
pub struct AudioClipComponent {
    pub uri: String,
    pub trigger_mode: AudioTriggerMode,
    pub load_state: AudioClipLoadState,
    /// When `Some`, this clip is an instance of another `AudioClipComponent`
    /// and shares its decoded asset (no re-decode). The URI is inherited
    /// from the source at registration time.
    pub source_component: Option<ComponentId>,
    /// Initial cursor offset, in transport beats, applied each time the
    /// clip is triggered (`SetEnabled(true)`).
    pub start_beat: f64,
    /// Optional hard stop in beats relative to the trigger fire beat.
    /// Combined with the trigger's `duration` as the minimum of the two.
    pub stop_beat: Option<f64>,
    component: Option<ComponentId>,
}

impl AudioClipComponent {
    pub fn new(uri: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            trigger_mode: AudioTriggerMode::Retrigger,
            load_state: AudioClipLoadState::Pending,
            source_component: None,
            start_beat: 0.0,
            stop_beat: None,
            component: None,
        }
    }

    /// Build a clip that shares another clip's decoded buffer. The URI
    /// is copied from `source` directly — the caller already has the
    /// authoritative value, so registration doesn't need to chase the
    /// component graph. `source_component` is recorded as metadata
    /// (useful for inspector / future "follow source" features).
    pub fn instance_of(source: &AudioClipComponent) -> Self {
        Self {
            uri: source.uri.clone(),
            trigger_mode: AudioTriggerMode::Retrigger,
            load_state: AudioClipLoadState::Pending,
            source_component: source.component,
            start_beat: 0.0,
            stop_beat: None,
            component: None,
        }
    }

    pub fn with_trigger_mode(mut self, mode: AudioTriggerMode) -> Self {
        self.trigger_mode = mode;
        self
    }

    pub fn with_start_beat(mut self, beat: f64) -> Self {
        self.start_beat = beat;
        self
    }

    pub fn with_stop_beat(mut self, beat: f64) -> Self {
        self.stop_beat = Some(beat);
        self
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.component
    }

    /// Best-effort existence check for the URI. Phase 5 will replace this
    /// with a real decode request to `AudioAssets`.
    pub fn check_uri_exists(uri: &str) -> AudioClipLoadState {
        if uri.is_empty() {
            return AudioClipLoadState::Failed("empty uri".into());
        }
        match std::fs::metadata(uri) {
            Ok(meta) if meta.is_file() => AudioClipLoadState::Loaded,
            Ok(_) => AudioClipLoadState::Failed(format!("not a file: {uri}")),
            Err(e) => AudioClipLoadState::Failed(format!("{uri}: {e}")),
        }
    }
}

impl Component for AudioClipComponent {
    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn name(&self) -> &'static str {
        "audio_clip"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        // Phase 5: request a decode via the AudioSystem. The decode
        // worker resolves missing files / unsupported codecs and reports
        // back through the engine's completion channel, which updates
        // `load_state` to `Loaded` / `Failed`.
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterAudioClip {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let ctor = match self
            .uri
            .rsplit('.')
            .next()
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("wav") => "wav",
            Some("opus") => "opus",
            Some("ogg") => "ogg",
            Some("mp3") => "mp3",
            Some("flac") => "flac",
            _ => "new",
        };
        let mut c = ce_call("AudioClip", ctor, vec![s(&self.uri)]);
        match self.trigger_mode {
            AudioTriggerMode::Retrigger => {}
            AudioTriggerMode::OneShot => c = c.with_call("one_shot", vec![]),
            AudioTriggerMode::Latched => c = c.with_call("latched", vec![]),
        }
        c
    }
}
