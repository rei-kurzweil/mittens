// Authored eyelid animation for a locally owned desktop avatar. Received copies
// consume its published weights; they do not run another competing blink clip.
export fn looping_blink(model, beats_per_second) {
    let blink = fn(weight) {
        model.set_morph_weight("left_eye_blink", weight)
        model.set_morph_weight("right_eye_blink", weight)
    }
    return Animation.looping().length(2.8 * beats_per_second) {
        name = "desktop_blink_clip"
        Keyframe.at(0.00 * beats_per_second) { blink(0.0) }
        Keyframe.at(0.80 * beats_per_second) { blink(0.0) }
        Keyframe.at(0.84 * beats_per_second) { blink(0.4) }
        Keyframe.at(0.88 * beats_per_second) { blink(1.0) }
        Keyframe.at(0.94 * beats_per_second) { blink(0.5) }
        Keyframe.at(1.02 * beats_per_second) { blink(0.0) }
    }
}
