use crate::engine::ecs::component::{MusicNote, NotePitch};

/// Music system.
///
/// Provides musical-pitch → frequency conversion. Note-trigger dispatch lives
/// in the `AudioSchedulePlay` intent path (see docs/spec/audio-sources.md);
/// this system is now stateless pitch math.
#[derive(Debug, Default)]
pub struct MusicSystem;

impl MusicSystem {
    pub fn new() -> Self {
        Self
    }

    pub(crate) fn frequency_hz_for_pitch(pitch: NotePitch, octave: u16) -> f32 {
        // Equal-tempered scale, A4 = 440Hz.
        // MIDI: C-1 = 0, C4 = 60, A4 = 69.
        let semitone_from_c = match pitch {
            NotePitch::C => 0,
            NotePitch::D => 2,
            NotePitch::E => 4,
            NotePitch::F => 5,
            NotePitch::G => 7,
            NotePitch::A => 9,
            NotePitch::B => 11,
        };

        let octave_i32 = octave as i32;
        let midi = (octave_i32 + 1) * 12 + semitone_from_c;
        let n = (midi - 69) as f32 / 12.0;
        440.0 * 2.0_f32.powf(n)
    }

    pub fn frequency_hz_for_note(note: MusicNote) -> f32 {
        Self::frequency_hz_for_pitch(note.pitch(), note.octave())
    }
}
