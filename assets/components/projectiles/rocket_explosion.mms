// Independent world-space expansion; the animation only starts the transition
// and removes the effect after it has finished.
export fn rocket_explosion(position) {
    let duration = 0.5
    let beats_per_second = 2.0
    let sphere = T.scale(0.04, 0.04, 0.04) {
        Transition { duration_beats(duration * beats_per_second) ease_out_quad() replace_same_target() }
        R.sphere() {
            Material(MaterialDefinition.toon())
            C.rgba(1.0, 0.65, 0.12, 1.0)
            Emissive.on().intensity(5.0)
        }
    }
    let root = T.position(position[0], position[1], position[2]) {
        name = "rocket_explosion"
        Raycastable.disabled()
        sphere
    }
    let animation = Animation.paused().length(duration * beats_per_second) {
        Keyframe.at(0.0) {
            sphere.update_transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [1.5, 1.5, 1.5])
        }
        Keyframe.at(duration * beats_per_second) { root.remove_subtree() }
    }
    root.attach(animation)
    return { root = root animation = animation }
}
