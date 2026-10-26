// World-space impact effect. Attach root, then play animation.
export fn mini_explosion(position, beats_per_second) {
    let emission = Emissive.on().intensity(18.0)
    let sphere = T.scale(0.07, 0.07, 0.07) {
        R.sphere() {
            Material(MaterialDefinition.toon())
            C.rgba(1.0, 0.93, 0.66, 1.0)
            emission
        }
    }
    let root = T.position(position[0], position[1], position[2]) {
        name = "mini_explosion"
        Raycastable.disabled()
        sphere
    }
    let clip = Animation.paused().length(0.30 * beats_per_second) {
        Keyframe.at(0.0) { emission.set_intensity(18.0) }
        Keyframe.at(0.06 * beats_per_second) {
            sphere.update_transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.20, 0.20, 0.20])
            sphere.set_color([1.0, 0.78, 0.08, 1.0])
            emission.set_intensity(8.0)
        }
        Keyframe.at(0.14 * beats_per_second) { emission.set_intensity(3.0) }
        Keyframe.at(0.22 * beats_per_second) {
            sphere.update_transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.05, 0.05, 0.05])
            emission.set_intensity(0.5)
        }
        Keyframe.at(0.30 * beats_per_second) { root.remove_subtree() }
    }
    root.attach(clip)
    return { root = root animation = clip }
}
