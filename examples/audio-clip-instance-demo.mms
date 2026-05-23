// audio-clip-instance-demo.mms
//
// Cloned from audio-music-context-demo. Two extra voices clone the
// AmenBreak clip via `.instance(start_beat)` — they share the decoded
// PCM buffer with `amen` but each has its own playhead starting at
// 0.25 / 0.5 beats into the sample.
//
// MusicContext addresses the clones by *live handle* (`amen_q`,
// `amen_h`) rather than by name selector. Names aren't needed since
// the bindings already point at the components.
//
// See docs/draft/audio-clip-instance-cloning.md.

Clock.bpm(174)

// Top-level bindings so MusicContext can see the clone handles.
let amen   = AudioClip.wav("assets/audio/KAB1_174_AmenBreak_Cut_02.wav") {
    name = "amen_break"
};
let amen_q = amen.instance(0.25);
let amen_h = amen.instance(0.5);

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

    // Splice the AmenBreak source + both clones into the AudioOutput
    // subtree so the graph compiler picks them up.
    amen;
    amen_q;
    amen_h;
}

MusicContext {
    voice("lead", "[name='lead']")
    voice("bass", "[name='bass']")
    voice("synth_bass", "[name='synth_bass']")
    voice("amen_break", "[name='amen_break']")
    voice("amen_quarter", amen_q)
    voice("amen_half", amen_h)

    Animation.looping() {
        name = "beepy_loop"

        Keyframe.at(0.00) {
            MusicNote.e(4, 0.25, "lead")
            MusicNote.c(2, 1.0, "amen_break")
        }
        // Clone fills the quiet quarter-note slot after the downbeat.
        Keyframe.at(0.75) { MusicNote.c(2, 0.25, "amen_quarter") }

        Keyframe.at(0.50) { MusicNote.c(4, 0.25, "lead") }

        Keyframe.at(1.00) { MusicNote.a(4, 0.25, "lead") }
        // And again before the next big hit.
        Keyframe.at(1.25) { MusicNote.c(2, 0.25, "amen_half") }

        Keyframe.at(1.50) {
            MusicNote.e(3, 0.25, "lead")
            MusicNote.c(2, 0.5, "amen_break")
        }

        Keyframe.at(2.0) {  MusicNote.a(2, 0.9, "synth_bass") }
        Keyframe.at(2.10) { MusicNote.a(1, 0.9, "synth_bass") }
        // Quarter-clone in the gap between bass hits.
        Keyframe.at(2.50) { MusicNote.c(2, 0.5, "amen_quarter") }

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
        // Half-clone late in the bar for a stutter effect.
        Keyframe.at(5.50) { MusicNote.c(2, 0.5, "amen_half") }

        Keyframe.at(6.0) {  MusicNote.a(2, 0.9, "synth_bass") }
        Keyframe.at(6.10) { MusicNote.a(1, 0.9, "synth_bass") }

        Keyframe.at(6.50) { MusicNote.a(4, 0.25, "lead") }
        Keyframe.at(7.0)  { MusicNote.e(3, 0.25, "lead") }
    }
}
