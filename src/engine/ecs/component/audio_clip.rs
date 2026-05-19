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
#[derive(Debug, Clone)]
pub struct AudioClipComponent {
    pub uri: String,
    pub trigger_mode: AudioTriggerMode,
    pub load_state: AudioClipLoadState,
    component: Option<ComponentId>,
}

impl AudioClipComponent {
    pub fn new(uri: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            trigger_mode: AudioTriggerMode::Retrigger,
            load_state: AudioClipLoadState::Pending,
            component: None,
        }
    }

    pub fn with_trigger_mode(mut self, mode: AudioTriggerMode) -> Self {
        self.trigger_mode = mode;
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

    fn init(
        &mut self,
        _emit: &mut dyn crate::engine::ecs::SignalEmitter,
        _component: ComponentId,
    ) {
        // Synchronous existence check stands in for the decode thread.
        // Phase 5 will issue a `LoadClip` request and update load_state
        // when the asset thread replies.
        self.load_state = Self::check_uri_exists(&self.uri);
        if let AudioClipLoadState::Failed(reason) = &self.load_state {
            eprintln!("[AudioClip] {}: {}", self.uri, reason);
        }
    }

    fn to_mms_ast(&self, _world: &crate::engine::ecs::World) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let ctor = match self.uri.rsplit('.').next().map(str::to_ascii_lowercase).as_deref() {
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
