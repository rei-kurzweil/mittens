import { laser } from "./laser.mms"
import { muzzle_flash } from "./muzzle-flash.mms"
import { mini_explosion } from "./mini-explosion.mms"

// Device policy belongs to the tool; callers translate input into use requests.
export fn laser_machine_pistol(options) {
    let label = "laser_machine_pistol"
    if options["name"] != null { label = options.name }
    let beats_per_second = 2.0
    if options["beats_per_second"] != null { beats_per_second = options.beats_per_second }
    let device_state = { firing = false cancelled = false bursts_fired = 0 shots_fired = 0 ammunition = null user = null tool = null }
    let flash = muzzle_flash({ name = label size = options.flash_size beats_per_second = beats_per_second })
    let sound = AudioClip.wav("assets/audio/tool/laser_machine_pistol_C.0.wav") { name = label + "_sound" }
    let completion = T { name = label + "_completion" }
    let first_aim = T { name = label + "_first_aim" }
    let second_aim = T.rotation(0.009, 0.012, 0.0) { name = label + "_second_aim" }
    let third_aim = T.rotation(0.018, -0.025, 0.0) { name = label + "_third_aim" }
    let first = laser({ name = label + "_first" length = options.length beam_radius = options.beam_radius ray_origin_offset = options.ray_origin_offset beats_per_second = beats_per_second })
    let second = laser({ name = label + "_second" length = options.length beam_radius = options.beam_radius ray_origin_offset = options.ray_origin_offset beats_per_second = beats_per_second })
    let third = laser({ name = label + "_third" length = options.length beam_radius = options.beam_radius ray_origin_offset = options.ray_origin_offset beats_per_second = beats_per_second })
    fn default_impact(hit) {
        let impact = mini_explosion(hit.hit_point, beats_per_second)
        impact.root
        impact.animation.play()
    }
    let on_hit = default_impact
    if options["on_hit"] != null { on_hit = options.on_hit }
    fn emit_shot(projectile, aim) {
        if !device_state.cancelled {
            flash.play()
            projectile.launch({
                pose = aim.world_trs()
                source_tool = device_state.tool source_user = device_state.user
                on_hit = on_hit
            })
            emit_data(completion, "ShotFired")
        }
    }
    let burst = Animation.paused().length(0.45 * beats_per_second) {
        name = label + "_burst"
        Keyframe.at(0.0) { sound.play() }
        Keyframe.at(0.0) { emit_shot(first, first_aim) }
        Keyframe.at((2.5 / 75.0) * beats_per_second) { emit_shot(second, second_aim) }
        Keyframe.at((5.0 / 75.0) * beats_per_second) { emit_shot(third, third_aim) }
        Keyframe.at(0.45 * beats_per_second) { emit_data(completion, "BurstComplete") }
    }
    on(completion, "DataEvent", fn(event) {
        if event == "BurstComplete" { device_state.firing = false }
        if event == "ShotFired" { device_state.shots_fired = device_state.shots_fired + 1 }
    })
    let root = T {
        name = label + "_root"
        Raycastable.disabled()
        flash.root
        first_aim
        second_aim
        third_aim
        completion
        AudioOutput { sound }
        burst
    }
    device_state.tool = root
    fn use_start(context) {
        if device_state.firing { return false }
        device_state.user = context.user
        device_state.firing = true
        device_state.cancelled = false
        device_state.bursts_fired = device_state.bursts_fired + 1
        burst.play()
        return true
    }
    fn use_end() {
        // Suppress pending shots; launched projectiles finish independently.
        device_state.cancelled = true
        flash.stop()
    }
    return { root = root animation = burst device_state = device_state use_start = use_start use_end = use_end flash = flash }
}
