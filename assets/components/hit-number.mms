// Temporary world-space hit feedback. Local +Z faces the shooter's launch point.
export fn hit_number(hit) {
    let duration = 0.60
    let beats_per_second = 2.0
    let lift = T.position(-0.055, 0.08, 0.025) {
        name = "hit_number_lift"
        Transition { duration_beats(duration * beats_per_second) ease_out_quad() replace_same_target() }
        Text { name = "hit_number_text" font_size(0.065) "1" C.rgba(1.0, 0.90, 0.35, 1.0) Unlit {} }
    }
    let root = T.position(hit.hit_point[0], hit.hit_point[1], hit.hit_point[2]) {
        name = "hit_number"
        Raycastable.disabled()
        lift
    }
    let animation = Animation.paused().length(duration * beats_per_second) {
        name = "hit_number_animation"
        Keyframe.at(0.0) {
            lift.update_transform([0.045, 0.43, 0.025], [0.0, 0.0, 0.0], [0.65, 0.65, 0.65])
        }
        Keyframe.at(duration * beats_per_second) { root.remove_subtree() }
    }
    root.attach(animation)
    root.look_at(hit.origin)
    return { root = root animation = animation }
}
