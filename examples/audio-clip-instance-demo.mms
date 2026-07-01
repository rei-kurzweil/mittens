// audio-clip-instance-demo.mms
//
// Cloned from audio-music-demo. Two extra voices clone the
// AmenBreak clip via `.instance(start_beat)` — they share the decoded
// PCM buffer with `amen` but each has its own playhead starting at
// 0.25 / 0.5 beats into the sample.
//
// The keyframes target the clip handles directly (`amen`, `amen_q`,
// `amen_h`) rather than routing through MusicContext voice names.
//
// See docs/draft/audio-clip-instance-cloning.md.

Clock.bpm(174)

// Top-level bindings so keyframe blocks can target the live handles directly.
let amen   = AudioClip.wav("assets/audio/KAB1_174_AmenBreak_Cut_02.wav") {
    name = "amen_break"
};
let amen_q = amen.instance(0.5);
let amen_h = amen.instance(0.75);
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

AudioOutput {
    lead;
    synth_bass;
    bass;
    // Splice the AmenBreak source + both clones into the AudioOutput
    // subtree so the graph compiler picks them up.
    amen;
    amen_q;
    amen_h;
}

Animation.length(16).looping() {
    name = "beepy_loop"

    Keyframe.at(0.00) {
        MusicNote.e(4, 0.25, lead)
        MusicNote.c(2, 1.0, amen)
    }
    // Clone fills the quiet quarter-note slot after the downbeat.
    Keyframe.at(0.75) { MusicNote.c(2, 0.25, amen_q) }

    Keyframe.at(0.50) { MusicNote.c(4, 0.25, lead) }

    Keyframe.at(1.00) { MusicNote.a(4, 0.25, lead) }
    // And again before the next big hit.
    Keyframe.at(1.25) { MusicNote.c(2, 0.25, amen_h) }

    Keyframe.at(1.50) {
        MusicNote.e(3, 0.25, lead)
        MusicNote.c(2, 0.5, amen)
    }

    Keyframe.at(2.0) {  MusicNote.a(2, 0.9, synth_bass) }
    Keyframe.at(2.10) { MusicNote.a(1, 0.9, synth_bass) }
    // Quarter-clone in the gap between bass hits.
    Keyframe.at(2.50) { MusicNote.c(2, 0.5, amen_q) }

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
    // Half-clone late in the bar for a stutter effect.
    Keyframe.at(5.50) { MusicNote.c(2, 0.5, amen_h) }

    Keyframe.at(6.0) {
        MusicNote.a(2, 0.9, synth_bass)
        MusicNote.c(2, 0.5, amen_h)
    }
    Keyframe.at(6.10) { MusicNote.a(1, 0.9, synth_bass) }

    Keyframe.at(6.50) { MusicNote.a(4, 0.25, lead) }
    Keyframe.at(6.75) { MusicNote.c(2, 0.5, amen_h) }
    Keyframe.at(7.0)  { MusicNote.e(3, 0.25, lead) }

    // -------- Repeat, shifted by +8 beats --------
    // Loop length = floor(max_keyframe_beat) + 1 → 15 → 16 beats.
    Keyframe.at(8.00) {
        MusicNote.e(4, 0.25, lead)
        MusicNote.c(2, 1.0, amen)
    }
    Keyframe.at(8.25)  { MusicNote.c(2, 0.25, amen_q) }

    Keyframe.at(8.50)  { MusicNote.c(4, 0.25, lead) }

    Keyframe.at(9.00)  { MusicNote.a(4, 0.25, lead) }
    Keyframe.at(9.25)  { MusicNote.c(2, 0.25, amen_h) }

    Keyframe.at(9.5)  {
        MusicNote.e(3, 0.25, lead)
        MusicNote.c(2, 0.5, amen)
    }

    Keyframe.at(10.00) { MusicNote.a(2, 0.9, synth_bass) }
    Keyframe.at(10.10) { MusicNote.a(1, 0.9, synth_bass) }
    Keyframe.at(10.50) { MusicNote.c(2, 0.5, amen_q) }

    Keyframe.at(11.00) {
        MusicNote.a(2, 0.9, synth_bass)
        MusicNote.c(2, 0.5, amen)
    }
    Keyframe.at(11.10) { MusicNote.a(1, 0.9, synth_bass) }

    Keyframe.at(11.50) {
        MusicNote.a(2, 0.9, synth_bass)
        MusicNote.c(2, 3.5, amen)
    }
    Keyframe.at(12.10) { MusicNote.a(1, 0.9, synth_bass) }

    Keyframe.at(13.00) { MusicNote.a(2, 0.9, synth_bass) }
    Keyframe.at(13.10) { MusicNote.a(1, 0.9, synth_bass) }
    Keyframe.at(13.50) { MusicNote.c(2, 0.5, amen_h) }

    Keyframe.at(14.00) {
        MusicNote.a(2, 0.9, synth_bass)
        MusicNote.c(2, 0.5, amen_h)
    }
    Keyframe.at(14.10) { MusicNote.a(1, 0.9, synth_bass) }

    Keyframe.at(14.50) { MusicNote.a(4, 0.25, lead) }
    Keyframe.at(14.75) { MusicNote.c(2, 0.5, amen_h) }
    Keyframe.at(14.00) { MusicNote.e(3, 0.25, lead) }
    Keyframe.at(15.00) { MusicNote.c(2, 0.5, amen_h) }
    Keyframe.at(15.50) { MusicNote.c(2, 0.5, amen_h) }
    Keyframe.at(16.00) { MusicNote.c(2, 0.5, amen_h) }
}
