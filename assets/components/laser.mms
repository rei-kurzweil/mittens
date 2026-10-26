import { mini_explosion } from "./mini-explosion.mms"

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
    let ray_origin_offset = 0.0
    if options["ray_origin_offset"] != null { ray_origin_offset = options.ray_origin_offset }
    let beats_per_second = 2.0
    if options["beats_per_second"] != null { beats_per_second = options.beats_per_second }
    let flash_0_emission = Emissive.off()
    let flash_1_emission = Emissive.off()
    let beam_emission = Emissive.off()
    let play_sound = true
    if options["sound"] != null { play_sound = options.sound }
    let shot_sound = AudioClip.wav("assets/audio/tool/laser_machine_pistol_C.0.wav") {
        name = label + "_sound"
    }
    fn play_shot_sound() { if play_sound { shot_sound.play() } }
    let raycaster = Raycast.event_driven().parent_forward(true).nearest_hit_only(true).max_distance(length) {
        name = label + "_raycaster"
    }
    let raycaster_mount = T.position(0.0, 0.0, -ray_origin_offset) {
        raycaster
    }
    on_global("RayIntersected", fn(event) {
        if event.raycaster == raycaster {
            let impact = mini_explosion(event.hit_point, beats_per_second)
            impact.root
            impact.animation.play()
        }
    })
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
    let shot = Animation.paused().length(0.04 * beats_per_second) {
        name = label + "_shot"
        // Keep audio in its own keyframe for the scheduler's audio lookahead.
        Keyframe.at(0.0) { play_shot_sound() }
        Keyframe.at(0.0) {
            muzzle_flash.update_transform([0.0, 0.0, 0.0], [0.0, 3.14159265, 0.0], [1.0, 1.0, 1.0])
            flash_0.update_transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [flash_size, flash_size, flash_size])
            flash_1.update_transform([0.0, 0.0, 0.002], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0])
            beam.update_transform([0.0, 0.0, -length / 2.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
            flash_0_emission.set_intensity(8.0)
            flash_1_emission.off()
            beam_emission.set_intensity(12.0)
        }
        Keyframe.at((1.0 / 75.0) * beats_per_second) {
            flash_0.update_transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0])
            flash_1.update_transform([0.0, 0.0, 0.002], [0.0, 0.0, 0.0], [flash_size, flash_size, flash_size])
            flash_0_emission.off()
            flash_1_emission.set_intensity(8.0)
        }
        Keyframe.at((2.0 / 75.0) * beats_per_second) {
            flash_1.update_transform([0.0, 0.0, 0.002], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0])
            flash_1_emission.off()
        }
        Keyframe.at(0.04 * beats_per_second) {
            hide_effects()
        }
    }
    // The muzzle remains attached to the weapon. Each reusable flight slot is
    // a world root, receiving a copied value only when that shot fires.
    let root = T { name = label + "_root" Raycastable.disabled() }
    let flight = T {
        name = label + "_flight"
        Raycastable.disabled()
        muzzle_flash
        beam
        raycaster_mount
        AudioOutput { shot_sound }
        shot
    }
    flight
    fn fire_laser() {
        let pose = root.world_trs()
        flight.world_trs(pose)
        raycaster.request_raycast()
        shot.play()
    }
    fn stop_laser() {
        shot.pause()
        hide_effects()
    }
    return { root = root fire_laser = fire_laser stop = stop_laser animation = shot raycaster = raycaster flight = flight }
}

// Independent sub-clips let muzzle frames and beams finish without a later shot
// rotating an earlier beam. First shot is exact; later shots add deterministic spread.
export fn laser_burst(options) {
    let label = "laser_burst"
    if options["name"] != null { label = options.name }
    let beats_per_second = 2.0
    if options["beats_per_second"] != null { beats_per_second = options.beats_per_second }
    let device_state = { firing = false cancelled = false bursts_fired = 0 }
    if options["device_state"] != null { device_state = options.device_state }
    let completion = T { name = label + "_completion" }
    let burst_sound = AudioClip.wav("assets/audio/tool/laser_machine_pistol_C.0.wav") { name = label + "_sound" }
    let first = laser({ name = label + "_first" length = options.length flash_size = options.flash_size beam_radius = options.beam_radius ray_origin_offset = options.ray_origin_offset beats_per_second = beats_per_second sound = false })
    let second = laser({ name = label + "_second" length = options.length flash_size = options.flash_size beam_radius = options.beam_radius ray_origin_offset = options.ray_origin_offset beats_per_second = beats_per_second sound = false })
    let third = laser({ name = label + "_third" length = options.length flash_size = options.flash_size beam_radius = options.beam_radius ray_origin_offset = options.ray_origin_offset beats_per_second = beats_per_second sound = false })
    fn fire_first() { if !device_state.cancelled { first.fire_laser() } }
    fn fire_second() { if !device_state.cancelled { second.fire_laser() } }
    fn fire_third() { if !device_state.cancelled { third.fire_laser() } }
    // Includes the complete 421 ms recording and all three visual shots.
    let burst = Animation.paused().length(0.45 * beats_per_second) {
        name = label + "_burst"
        Keyframe.at(0.0) { burst_sound.play() }
        Keyframe.at(0.0) { fire_first() }
        Keyframe.at((2.5 / 75.0) * beats_per_second) { fire_second() }
        Keyframe.at((5.0 / 75.0) * beats_per_second) { fire_third() }
        Keyframe.at(0.45 * beats_per_second) { emit_data(completion, "BurstComplete") }
    }
    on(completion, "DataEvent", fn(event) {
        if event == "BurstComplete" { device_state.firing = false }
    })
    fn fire_burst() {
        if device_state.firing { return false }
        device_state.firing = true
        device_state.cancelled = false
        device_state.bursts_fired = device_state.bursts_fired + 1
        burst.play()
        return true
    }
    fn stop_burst() {
        // Keep completion running so dropping/regrabbing cannot bypass cooldown.
        device_state.cancelled = true
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
        completion
        AudioOutput { burst_sound }
        burst
    }
    return { root = root fire_burst = fire_burst stop = stop_burst animation = burst device_state = device_state }
}
