// audio-music-context-demo.mms
//
// Demonstrates the unified audio source API (docs/spec/audio-sources.md):
//   - MusicContext declares two voices ("lead", "bass")
//   - 16 sixteenth notes ascend a 2-octave A natural minor scale on the
//     square-wave lead (one bar in 4/4 at 140 BPM, then loops)
//   - A bass sample plays on beats 1 and 3
//
// The bass sample URI may not exist on disk yet — the AudioClip load path
// reports a missing source without crashing the scene (see
// AudioClipLoadState::Failed).

Clock.bpm(140)

AudioOutput {
    AudioOscillator.square() {
        name = "lead"
        frequency(440)
        amplitude(0.12)
        enabled(false)
    }

    AudioClip.wav("assets/audio/bass-c2.wav") {
        name = "bass"
    }
}

MusicContext {
    voice("lead", "[name='lead']")
    voice("bass", "[name='bass']")

    Animation.looping() {
        name = "minor_scale_loop"

        for octave in range(3) {
            for note in range(6) {
                Keyframe.at(note * 0.25 + octave * 2.0) {
                    if note == 0 {
                        MusicNote.a(3 + octave, 0.25, "lead")
                    } else if note == 1 {
                        MusicNote.b(3 + octave, 0.25, "lead")
                    } else if note == 2 {
                        MusicNote.c(4 + octave, 0.25, "lead")
                    } else if note == 3 {
                        MusicNote.d(4 + octave, 0.25, "lead")
                    } else if note == 4 {
                        MusicNote.e(4 + octave, 0.25, "lead")
                    } else if note == 5 {
                        MusicNote.f(4 + octave, 0.25, "lead")
                    } else if note == 6 {
                        MusicNote.g(4 + octave, 0.25, "lead")
                    } else if note == 7 {
                        MusicNote.a(4 + octave, 0.25, "lead")
                    }
                }
            }
        }
        

        // Lead: A natural minor ascending two octaves, 16 sixteenth notes
        // = 1 bar of 4/4. Each note is 0.25 beats long, packed end-to-end.
        // Octave 1: A3 .. A4
        // Keyframe.at(0.00) { MusicNote.a(3, 0.25, "lead") }
        // Keyframe.at(0.25) { MusicNote.b(3, 0.25, "lead") }
        // Keyframe.at(0.50) { MusicNote.c(4, 0.25, "lead") }
        // Keyframe.at(0.75) { MusicNote.d(4, 0.25, "lead") }
        // Keyframe.at(1.00) { MusicNote.e(4, 0.25, "lead") }
        // Keyframe.at(1.25) { MusicNote.f(4, 0.25, "lead") }
        // Keyframe.at(1.50) { MusicNote.g(4, 0.25, "lead") }
        // Keyframe.at(1.75) { MusicNote.a(4, 0.25, "lead") }
        // // Octave 2: B4 .. B5
        // Keyframe.at(2.00) { MusicNote.b(4, 0.25, "lead") }
        // Keyframe.at(2.25) { MusicNote.c(5, 0.25, "lead") }
        // Keyframe.at(2.50) { MusicNote.d(5, 0.25, "lead") }
        // Keyframe.at(2.75) { MusicNote.e(5, 0.25, "lead") }
        // Keyframe.at(3.00) { MusicNote.f(5, 0.25, "lead") }
        // Keyframe.at(3.25) { MusicNote.g(5, 0.25, "lead") }
        // Keyframe.at(3.50) { MusicNote.a(5, 0.25, "lead") }
        // Keyframe.at(3.75) { MusicNote.b(5, 0.25, "lead") }

        // // Bass: C2 sample on beats 1 and 3 (half-note holds).
        // Keyframe.at(0.0) { MusicNote.c(2, 2.0, "bass") }
        // Keyframe.at(2.0) { MusicNote.c(2, 2.0, "bass") }
    }
}
