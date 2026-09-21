// Placeholder mouth-response preset for Rei's 2026.9 microphone setup.
//
// It deliberately returns plain data rather than constructing or mutating an
// AVC. That keeps this first preset limited to the three calibrated response
// values and lets a scene choose its own source, smoothing, and morph target.

export fn rei_2026_9() {
    return {
        rms_center = 0.038
        rms_range = 0.060
        mouth_movement_amount = 1.0
    }
}
