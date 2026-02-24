use crate::engine::ecs::component::{
    AudioOscillatorComponent, MusicNote, MusicNoteComponent, NotePitch,
};
use crate::engine::ecs::{ComponentId, World};

/// Music system.
///
/// Today it is intentionally minimal: it provides pitch/octave -> frequency conversion
/// and can apply MusicNoteComponents to AudioOscillatorComponents.
#[derive(Debug, Default)]
pub struct MusicSystem;

impl MusicSystem {
    pub fn new() -> Self {
        Self
    }

    pub(crate) fn frequency_hz_for_pitch(pitch: NotePitch, octave: u16) -> f32 {
        // Equal-tempered scale, A4 = 440Hz.
        // Map (pitch, octave) to MIDI note number.
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

    pub fn apply_music_note_to_oscillator(&mut self, world: &mut World, component: ComponentId) {
        // Find the first MusicNoteComponent anywhere in this oscillator's subtree.
        let mut stack = vec![component];
        let mut found_note = None;
        while let Some(node) = stack.pop() {
            for &ch in world.children_of(node) {
                if let Some(nc) = world.get_component_by_id_as::<MusicNoteComponent>(ch) {
                    found_note = Some(nc.note);
                    break;
                }
                stack.push(ch);
            }
            if found_note.is_some() {
                break;
            }
        }

        let Some(note) = found_note else {
            return;
        };

        let freq = Self::frequency_hz_for_note(note);

        let Some(osc) = world.get_component_by_id_as_mut::<AudioOscillatorComponent>(component)
        else {
            return;
        };

        for o in osc.oscillators.iter_mut() {
            if o.music_note_applied {
                continue;
            }
            o.frequency = freq;
            o.music_note_applied = true;
        }
    }
}
