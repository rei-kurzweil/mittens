// Reusable weapon-mounted sprite animation. Attach root beneath a muzzle.
export fn muzzle_flash(options) {
    let label = "laser"
    if options["name"] != null { label = options.name }
    let size = 0.35
    if options["size"] != null { size = options.size }
    let beats_per_second = 2.0
    if options["beats_per_second"] != null { beats_per_second = options.beats_per_second }
    let textures = ["assets/images/flash_red_0.png", "assets/images/flash_red_1.png"]
    if options["textures"] != null { textures = options.textures }
    let intensity = 8.0
    if options["intensity"] != null { intensity = options.intensity }
    let frame_seconds = 1.0 / 75.0
    if options["frame_seconds"] != null { frame_seconds = options.frame_seconds }
    let first_emission = Emissive.off()
    let second_emission = Emissive.off()
    let first = T.scale(0.0, 0.0, 0.0) {
        name = label + "_muzzle_flash_0"
        R.square() {
            Material(MaterialDefinition.toon())
            Texture.with_uri(textures[0])
            TextureFiltering.linear()
            TransparentCutout {}
            first_emission
        }
    }
    let second = T.position(0.0, 0.0, 0.002).scale(0.0, 0.0, 0.0) {
        name = label + "_muzzle_flash_1"
        R.square() {
            Material(MaterialDefinition.toon())
            Texture.with_uri(textures[1])
            TextureFiltering.linear()
            TransparentCutout {}
            second_emission
        }
    }
    fn hide() {
        first.update_transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0])
        second.update_transform([0.0, 0.0, 0.002], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0])
        first_emission.off()
        second_emission.off()
    }
    let animation = Animation.paused().length(2.0 * frame_seconds * beats_per_second) {
        name = label + "_muzzle_flash_clip"
        Keyframe.at(0.0) {
            first.update_transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [size, size, size])
            second.update_transform([0.0, 0.0, 0.002], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0])
            first_emission.set_intensity(intensity)
            second_emission.off()
        }
        Keyframe.at(frame_seconds * beats_per_second) {
            first.update_transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0])
            second.update_transform([0.0, 0.0, 0.002], [0.0, 0.0, 0.0], [size, size, size])
            first_emission.off()
            second_emission.set_intensity(intensity)
        }
        Keyframe.at(2.0 * frame_seconds * beats_per_second) { hide() }
    }
    let root = T.rotation(0.0, 3.14159265, 0.0) {
        name = label + "_muzzle_flash"
        Raycastable.disabled()
        first
        second
        animation
    }
    fn play() { animation.play() }
    fn stop() { animation.pause() hide() }
    return { root = root animation = animation play = play stop = stop }
}
