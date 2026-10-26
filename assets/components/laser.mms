// Reusable visual laser. Local -Z is the firing direction; units are metres.
// laser(options) returns { root, fire_laser, stop, animation }.
// laser_burst(options) returns { root, fire_burst, stop, animation }.
// Timing options use beats_per_second (default 2, matching Clock.bpm(120)).
export fn laser(options) {
    let label = "laser"
    if options["name"] != null { label = options.name }
    let beam_name = label + "_beam"
    if options["beam_name"] != null { beam_name = options.beam_name }
    let length = 40.0
    if options["length"] != null { length = options.length }
    let flash_size = 0.35
    if options["flash_size"] != null { flash_size = options.flash_size }
    let beam_radius = 0.008
    if options["beam_radius"] != null { beam_radius = options.beam_radius }
    let beats_per_second = 2.0
    if options["beats_per_second"] != null { beats_per_second = options.beats_per_second }
    let flash_0_emission = Emissive.off()
    let flash_1_emission = Emissive.off()
    let beam_emission = Emissive.off()
    let shot_sound = AudioClip.wav("assets/audio/tool/laser_machine_pistol_C.0.wav") {
        name = label + "_sound"
    }
    let flash_0 = T.scale(0.0, 0.0, 0.0) {
        name = label + "_muzzle_flash_0"
        R.square() {
            Material(MaterialDefinition.toon())
            Texture.with_uri("assets/images/flash_red_0.png")
            TextureFiltering.linear()
            TransparentCutout {}
            flash_0_emission
        }
    }
    let flash_1 = T.position(0.0, 0.0, 0.002).scale(0.0, 0.0, 0.0) {
        name = label + "_muzzle_flash_1"
        R.square() {
            Material(MaterialDefinition.toon())
            Texture.with_uri("assets/images/flash_red_1.png")
            TextureFiltering.linear()
            TransparentCutout {}
            flash_1_emission
        }
    }
    let muzzle_flash = T.rotation(0.0, 3.14159265, 0.0) {
        name = label + "_muzzle_flash"
        Raycastable.disabled()
        flash_0
        flash_1
    }
    // A real cylinder stays visible when viewed along either side. Its nearest
    // end is at the muzzle, while the owning frame sits at the beam midpoint.
    let beam = T.position(0.0, 0.0, -length / 2.0).scale(0.0, 0.0, 0.0) {
        name = beam_name
        Raycastable.disabled()
        T.position(0.0, 0.0, length / 2.0).rotation(-1.57079633, 0.0, 0.0) {
            R.cylinder(beam_radius, length, 8) {
                Material(MaterialDefinition.toon())
                C.rgba(1.0, 0.12, 0.06, 1.0)
                beam_emission
            }
        }
    }
    fn hide_effects() {
        muzzle_flash.update_transform([0.0, 0.0, 0.0], [0.0, 3.14159265, 0.0], [0.0, 0.0, 0.0])
        flash_0.update_transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0])
        flash_1.update_transform([0.0, 0.0, 0.002], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0])
        beam.update_transform([0.0, 0.0, -length / 2.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0])
        flash_0_emission.off()
        flash_1_emission.off()
        beam_emission.off()
    }
    let shot = Animation.paused().length(0.12 * beats_per_second) {
        name = label + "_shot"
        // Keep audio in its own keyframe for the scheduler's audio lookahead.
        Keyframe.at(0.0) { shot_sound.play() }
        Keyframe.at(0.0) {
            muzzle_flash.update_transform([0.0, 0.0, 0.0], [0.0, 3.14159265, 0.0], [1.0, 1.0, 1.0])
            flash_0.update_transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [flash_size, flash_size, flash_size])
            flash_1.update_transform([0.0, 0.0, 0.002], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0])
            beam.update_transform([0.0, 0.0, -length / 2.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
            flash_0_emission.set_intensity(8.0)
            flash_1_emission.off()
            beam_emission.set_intensity(12.0)
        }
        Keyframe.at(0.03 * beats_per_second) {
            flash_0.update_transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0])
            flash_1.update_transform([0.0, 0.0, 0.002], [0.0, 0.0, 0.0], [flash_size, flash_size, flash_size])
            flash_0_emission.off()
            flash_1_emission.set_intensity(8.0)
        }
        Keyframe.at(0.09 * beats_per_second) {
            hide_effects()
        }
    }
    fn fire_laser() {
        shot.play()
    }
    fn stop_laser() {
        shot.pause()
        hide_effects()
    }
    let root = T {
        name = label + "_root"
        Raycastable.disabled()
        muzzle_flash
        beam
        AudioOutput { shot_sound }
        shot
    }
    return { root = root fire_laser = fire_laser stop = stop_laser animation = shot }
}

// Independent sub-clips let muzzle frames and beams finish without a later shot
// rotating an earlier beam. First shot is exact; later shots add deterministic spread.
export fn laser_burst(options) {
    let label = "laser_burst"
    if options["name"] != null { label = options.name }
    let beats_per_second = 2.0
    if options["beats_per_second"] != null { beats_per_second = options.beats_per_second }
    let first = laser({ name = label + "_first" length = options.length flash_size = options.flash_size beam_radius = options.beam_radius beats_per_second = beats_per_second })
    let second = laser({ name = label + "_second" length = options.length flash_size = options.flash_size beam_radius = options.beam_radius beats_per_second = beats_per_second })
    let third = laser({ name = label + "_third" length = options.length flash_size = options.flash_size beam_radius = options.beam_radius beats_per_second = beats_per_second })
    let burst = Animation.paused().length(0.41 * beats_per_second) {
        name = label + "_burst"
        Keyframe.at(0.0) { first.fire_laser() }
        Keyframe.at(0.12 * beats_per_second) { second.fire_laser() }
        Keyframe.at(0.24 * beats_per_second) { third.fire_laser() }
    }
    fn fire_burst() {
        burst.play()
    }
    fn stop_burst() {
        burst.pause()
        first.stop()
        second.stop()
        third.stop()
    }
    let root = T {
        name = label + "_root"
        Raycastable.disabled()
        first.root
        T.rotation(0.009, 0.012, 0.0) { second.root }
        T.rotation(0.018, -0.025, 0.0) { third.root }
        burst
    }
    return { root = root fire_burst = fire_burst stop = stop_burst animation = burst }
}
