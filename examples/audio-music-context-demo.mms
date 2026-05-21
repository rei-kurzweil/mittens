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

Clock.bpm(174)

AudioOutput {
    AudioOscillator.square() {
        name = "lead"
        frequency(440)
        amplitude(0.1)
        enabled(false)
    }

    AudioOscillator.square() {
        name = "synth_bass"
        frequency(440)
        amplitude(0.45)
        enabled(false)
    }

    AudioClip.wav("assets/audio/bass-c2.wav") {
        name = "bass"
    }

    AudioClip.wav("assets/audio/KAB1_174_AmenBreak_Cut_02.wav") {
        name = "amen_break"
    }
}

MusicContext {
    voice("lead", "[name='lead']")
    voice("bass", "[name='bass']")
    voice("synth_bass", "[name='synth_bass']")
    voice("amen_break", "[name='amen_break']")

    Animation.looping() {
        name = "beepy_loop"

        Keyframe.at(0.00) { 
            MusicNote.e(4, 0.25, "lead") 
            MusicNote.c(2, 1.0, "amen_break")
        }
        // Keyframe.at(0.25) { MusicNote.b(3, 0.25, "lead") }
        Keyframe.at(0.50) { MusicNote.c(4, 0.25, "lead") }
        // Keyframe.at(0.75) { MusicNote.d(4, 0.25, "lead") }
        Keyframe.at(1.00) { MusicNote.a(4, 0.25, "lead") }
        // Keyframe.at(1.25) { MusicNote.f(4, 0.25, "lead") }
        Keyframe.at(1.50) { 
            MusicNote.e(3, 0.25, "lead") 
            MusicNote.c(2, 0.5, "amen_break")
        }
        // Keyframe.at(1.75) { MusicNote.d(2, 0.25, "lead") }
        
        Keyframe.at(2.0) {  MusicNote.a(2, 0.9, "synth_bass") }
        Keyframe.at(2.10) { MusicNote.a(1, 0.9, "synth_bass") }

        Keyframe.at(3.0) {  
            MusicNote.a(2, 0.9, "synth_bass") 
            MusicNote.c(2, 0.5, "amen_break")
        }
        Keyframe.at(3.10) { MusicNote.a(1, 0.9, "synth_bass") }

        Keyframe.at(4.0) {  
            MusicNote.a(2, 0.9, "synth_bass") 
            MusicNote.c(2, 3.5, "amen_break")
        }
        Keyframe.at(4.10) { MusicNote.a(1, 0.9, "synth_bass") }

        Keyframe.at(5.0) {  MusicNote.a(2, 0.9, "synth_bass") }
        Keyframe.at(5.10) { MusicNote.a(1, 0.9, "synth_bass") }

        Keyframe.at(6.0) {  MusicNote.a(2, 0.9, "synth_bass") }
        Keyframe.at(6.10) { MusicNote.a(1, 0.9, "synth_bass") }

        Keyframe.at(6.50) { MusicNote.a(4, 0.25, "lead") }
        Keyframe.at(7.0) { MusicNote.e(3, 0.25, "lead") }

        // Keyframe.at(0.0) { MusicNote.c(2, 2.0, "bass") } // Bass: C2 sample on beats 1 and 3 (half-note holds).
        // Keyframe.at(2.0) { MusicNote.c(2, 2.0, "bass") }
    }
}
