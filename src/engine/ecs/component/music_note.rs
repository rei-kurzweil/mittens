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

    #[serde(skip)]
    component: Option<ComponentId>,
}

impl MusicNoteComponent {
    pub fn new(note: MusicNote) -> Self {
        Self {
            note,
            component: None,
        }
    }

    pub fn from_note(note: MusicNote) -> Self {
        Self::new(note)
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

    fn to_mms_ast(&self) -> crate::meow_meow::ast::ComponentExpression {
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
        c
    }
}
