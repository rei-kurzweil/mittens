use super::Component;
use crate::engine::ecs::ComponentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum NotePitch {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct MusicNote {
    duration: f32,
    #[serde(default = "MusicNote::default_velocity")]
    velocity: f32,
    pitch: NotePitch,
    octave: u16,
}

impl Default for MusicNote {
    fn default() -> Self {
        Self {
            duration: 0.25,
            velocity: 1.0,
            pitch: NotePitch::A,
            octave: 4,
        }
    }
}

impl MusicNote {
    fn default_velocity() -> f32 {
        1.0
    }

    pub fn duration_beats(&self) -> f32 {
        self.duration
    }

    pub fn velocity(&self) -> f32 {
        self.velocity
    }

    pub fn octave(&self) -> u16 {
        self.octave
    }

    pub fn pitch_name(&self) -> &'static str {
        match self.pitch {
            NotePitch::A => "a",
            NotePitch::B => "b",
            NotePitch::C => "c",
            NotePitch::D => "d",
            NotePitch::E => "e",
            NotePitch::F => "f",
            NotePitch::G => "g",
        }
    }

    pub fn with_duration_beats(mut self, duration_beats: f32) -> Self {
        self.duration = duration_beats;
        self
    }

    pub fn with_velocity(mut self, velocity: f32) -> Self {
        self.velocity = if velocity.is_finite() {
            velocity.max(0.0)
        } else {
            1.0
        };
        self
    }

    pub fn with_octave(mut self, octave: u16) -> Self {
        self.octave = octave;
        self
    }

    pub fn a(octave: u16, duration_beats: f32) -> Self {
        Self {
            duration: duration_beats,
            velocity: 1.0,
            pitch: NotePitch::A,
            octave,
        }
    }

    pub fn b(octave: u16, duration_beats: f32) -> Self {
        Self {
            duration: duration_beats,
            velocity: 1.0,
            pitch: NotePitch::B,
            octave,
        }
    }

    pub fn c(octave: u16, duration_beats: f32) -> Self {
        Self {
            duration: duration_beats,
            velocity: 1.0,
            pitch: NotePitch::C,
            octave,
        }
    }

    pub fn d(octave: u16, duration_beats: f32) -> Self {
        Self {
            duration: duration_beats,
            velocity: 1.0,
            pitch: NotePitch::D,
            octave,
        }
    }

    pub fn e(octave: u16, duration_beats: f32) -> Self {
        Self {
            duration: duration_beats,
            velocity: 1.0,
            pitch: NotePitch::E,
            octave,
        }
    }

    pub fn f(octave: u16, duration_beats: f32) -> Self {
        Self {
            duration: duration_beats,
            velocity: 1.0,
            pitch: NotePitch::F,
            octave,
        }
    }

    pub fn g(octave: u16, duration_beats: f32) -> Self {
        Self {
            duration: duration_beats,
            velocity: 1.0,
            pitch: NotePitch::G,
            octave,
        }
    }

    pub(crate) fn from_pitch(duration_beats: f32, pitch: NotePitch, octave: u16) -> Self {
        Self {
            duration: duration_beats,
            velocity: 1.0,
            pitch,
            octave,
        }
    }

    pub(crate) fn pitch(&self) -> NotePitch {
        self.pitch
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MusicNoteComponent {
    pub note: MusicNote,

    /// Pre-authored target audio source (`audio_source` per spec §6.4.1).
    /// When None, resolution falls back to context / topology / error per
    /// docs/spec/audio-sources.md §6.6. Runtime-only — serde round-trip
    /// loses this; MMS authoring sets it through the component registry
    /// resolution path (phase 3).
    #[serde(skip)]
    pub target: Option<ComponentId>,

    /// Pre-authored default beat (`scheduled_beat` per spec §6.4.1). When
    /// None, `.play()` fires immediately (beat_offset = 0).
    #[serde(default)]
    pub scheduled_beat: Option<f64>,

    /// Fire `AudioSchedulePlay` automatically when this component initializes.
    /// Defaults to false — silent unless explicitly triggered.
    #[serde(default)]
    pub play_on_attach: bool,

    #[serde(skip)]
    component: Option<ComponentId>,
}

impl MusicNoteComponent {
    pub fn new(note: MusicNote) -> Self {
        Self {
            note,
            target: None,
            scheduled_beat: None,
            play_on_attach: false,
            component: None,
        }
    }

    pub fn from_note(note: MusicNote) -> Self {
        Self::new(note)
    }

    pub fn with_target(mut self, target: ComponentId) -> Self {
        self.target = Some(target);
        self
    }

    pub fn with_scheduled_beat(mut self, beat: f64) -> Self {
        self.scheduled_beat = Some(beat);
        self
    }

    pub fn with_play_on_attach(mut self, on: bool) -> Self {
        self.play_on_attach = on;
        self
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.component
    }
}

impl Default for MusicNoteComponent {
    fn default() -> Self {
        Self::new(MusicNote::default())
    }
}

impl Component for MusicNoteComponent {
    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn name(&self) -> &'static str {
        "music_note"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(
        &mut self,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        component: ComponentId,
    ) {
        if !self.play_on_attach {
            return;
        }
        // Per docs/spec/audio-sources.md §6.4: play_on_attach fires
        // AudioSchedulePlay when the component initializes. Target resolution
        // is deferred to the executor — collect_oscillator_targets walks the
        // subtree and falls back to ancestor lookup (§6.6 rank 5).
        let target = self.target.unwrap_or(component);
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::AudioSchedulePlay {
                component_ids: vec![target],
                beat_offset: 0.0,
                beat_context: self.scheduled_beat,
                note: Some(self.note),
                gain: None,
                rate: None,
                duration: None,
            },
        );
    }

    fn to_mms_ast(&self, _world: &crate::engine::ecs::World) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let pitch = self.note.pitch_name();
        let mut c = ce_call(
            "MusicNote",
            pitch,
            vec![
                num(self.note.octave() as f64),
                num(self.note.duration_beats() as f64),
            ],
        );
        if (self.note.velocity() - 1.0).abs() > f32::EPSILON {
            c = c.with_call("velocity", vec![num(self.note.velocity() as f64)]);
        }
        if self.play_on_attach {
            c = c.with_call("play_on_attach", vec![]);
        }
        if let Some(b) = self.scheduled_beat {
            c = c.with_call("at_beat", vec![num(b)]);
        }
        c
    }
}
