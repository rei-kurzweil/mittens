// Idle tail wag for a resolved `tail` joint transform, the driver joint above
// the tail's spring chain. Only this root joint is animated; the spring chain
// below it (rooted at J_Bip_C_Hips.003, see secondary_motion/bisket.mms) lags
// behind, so the tip catches up after each swing.
//
// style "canine": swings side to side continuously; each swing starts the
//   moment the previous one ends.
// style "feline": the same swing, but holds at each side for two thirds of the
//   swing time before swinging back.
//
// `beats_per_second` converts the authored seconds to the scene clock's beat
// unit: pass `Clock BPM / 60` (for example, `2.0` for a 120 BPM clock).
// Rotation is about the joint's local X axis, which points roughly up on the
// mitten/Bisket rig, so the swing is left-right.

export fn tail_wag(tail, style, beats_per_second) {
    let swing_seconds = 0.38
    let amplitude = 0.35
    let hold_seconds = 0.0
    if style == "feline" {
        swing_seconds = 0.9
        amplitude = 0.3
        hold_seconds = swing_seconds * 2.0 / 3.0
    }
    let transition = Transition {
        duration_beats(swing_seconds * beats_per_second)
        ease_in_out_sine()
        replace_same_target()
    }
    tail.attach(transition)
    let swing = fn(angle) { tail.rest_relative_rotation([angle, 0.0, 0.0]) }
    let side = (swing_seconds + hold_seconds) * beats_per_second
    return Animation.looping().length(2.0 * side) {
        Keyframe.at(0.0) { swing(amplitude) }
        Keyframe.at(side) { swing(-amplitude) }
    }
}
