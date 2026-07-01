// audio-music-demo.mms
//
// Demonstrates the unified audio source API (docs/spec/audio-sources.md):
//   - direct live handles target the same lead, synth bass, and Amen break
//     instruments as the old MusicContext demo
//   - keyframes keep the original 174 BPM beat map so the Amen break lines up
//   - A bass sample plays on beats 1 and 3
//
// The bass sample URI may not exist on disk yet — the AudioClip load path
// reports a missing source without crashing the scene (see
// AudioClipLoadState::Failed).

Clock.bpm(174)

let lead = AudioOscillator.square() {
    name = "lead"
    frequency(440)
    amplitude(0.1)
    enabled(false)
};

let synth_bass = AudioOscillator.square() {
    name = "synth_bass"
    frequency(440)
    amplitude(0.45)
    enabled(false)
};

let bass = AudioClip.wav("assets/audio/bass-c2.wav") {
    name = "bass"
};

let amen = AudioClip.wav("assets/audio/KAB1_174_AmenBreak_Cut_02.wav") {
    name = "amen_break"
};

AudioOutput {
    lead;
    synth_bass;
    bass;
    amen;
    // Sibling instance — shares amen's decoded buffer, starts 0.5
    // beats into the sample. Smoke-tests `.instance()` wiring.
    let amen_late = amen.instance(0.5);
    amen_late;
}

Animation.looping() {
    name = "beepy_loop"

    Keyframe.at(0.00) {
        MusicNote.e(4, 0.25, lead)
        MusicNote.c(2, 1.0, amen)
    }
    // Keyframe.at(0.25) { MusicNote.b(3, 0.25, lead) }
    Keyframe.at(0.50) { MusicNote.c(4, 0.25, lead) }
    // Keyframe.at(0.75) { MusicNote.d(4, 0.25, lead) }
    Keyframe.at(1.00) { MusicNote.a(4, 0.25, lead) }
    // Keyframe.at(1.25) { MusicNote.f(4, 0.25, lead) }
    Keyframe.at(1.50) {
        MusicNote.e(3, 0.25, lead)
        MusicNote.c(2, 0.5, amen)
    }
    // Keyframe.at(1.75) { MusicNote.d(2, 0.25, lead) }

    Keyframe.at(2.0) {  MusicNote.a(2, 0.9, synth_bass) }
    Keyframe.at(2.10) { MusicNote.a(1, 0.9, synth_bass) }

    Keyframe.at(3.0) {
        MusicNote.a(2, 0.9, synth_bass)
        MusicNote.c(2, 0.5, amen)
    }
    Keyframe.at(3.10) { MusicNote.a(1, 0.9, synth_bass) }

    Keyframe.at(4.0) {
        MusicNote.a(2, 0.9, synth_bass)
        MusicNote.c(2, 3.5, amen)
    }
    Keyframe.at(4.10) { MusicNote.a(1, 0.9, synth_bass) }

    Keyframe.at(5.0) {  MusicNote.a(2, 0.9, synth_bass) }
    Keyframe.at(5.10) { MusicNote.a(1, 0.9, synth_bass) }

    Keyframe.at(6.0) {  MusicNote.a(2, 0.9, synth_bass) }
    Keyframe.at(6.10) { MusicNote.a(1, 0.9, synth_bass) }

    Keyframe.at(6.50) { MusicNote.a(4, 0.25, lead) }
    Keyframe.at(7.0) { MusicNote.e(3, 0.25, lead) }

    // Keyframe.at(0.0) { MusicNote.c(2, 2.0, bass) } // Bass: C2 sample on beats 1 and 3 (half-note holds).
    // Keyframe.at(2.0) { MusicNote.c(2, 2.0, bass) }
}
