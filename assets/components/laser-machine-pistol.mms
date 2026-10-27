import { laser } from "./laser.mms"
import { muzzle_flash } from "./muzzle-flash.mms"
import { mini_explosion } from "./mini-explosion.mms"
import { ui_unlit_material } from "./materials/ui_unlit.mms"

// Semi: one shot. Burst: three fast shots. Auto: slower, continuous firing.
export fn laser_machine_pistol(options) {
    let label = "laser_machine_pistol"
    if options["name"] != null { label = options.name }
    let beats_per_second = 2.0
    if options["beats_per_second"] != null { beats_per_second = options.beats_per_second }
    let shot_interval = 2.5 / 75.0
    let auto_shot_interval = shot_interval / 0.7 // 30% fewer shots per second than Burst
    let device_state = { firing = false cancelled = false trigger_held = false mode = "semi" burst_mode = "semi" bursts_fired = 0 shots_fired = 0 ammunition = null user = null requested_user = null tool = null }
    let flash = muzzle_flash({ name = label size = options.flash_size beats_per_second = beats_per_second })
    let single_sound = AudioClip.wav("assets/audio/tool/laser_machine_pistol_A.0.wav") { name = label + "_single_sound" }
    let sound = AudioClip.wav("assets/audio/tool/laser_machine_pistol_C.0.wav") { name = label + "_sound" }
    let auto_sound = AudioClip.wav("assets/audio/tool/laser_machine_pistol_B.0.wav") { name = label + "_auto_sound" }
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
            projectile.launch({ pose = aim.world_trs() source_tool = device_state.tool source_user = device_state.user on_hit = on_hit })
            emit_data(completion, "ShotFired")
        }
    }
    let semi = Animation.paused().length(0.45 * beats_per_second) {
        name = label + "_semi"
        Keyframe.at(0.0) { single_sound.play() }
        Keyframe.at(0.0) { emit_shot(first, first_aim) }
        Keyframe.at(0.45 * beats_per_second) { emit_data(completion, "BurstComplete") }
    }
    let burst = Animation.paused().length(0.45 * beats_per_second) {
        name = label + "_burst"
        Keyframe.at(0.0) { sound.play() }
        Keyframe.at(0.0) { emit_shot(first, first_aim) }
        Keyframe.at(shot_interval * beats_per_second) { emit_shot(second, second_aim) }
        Keyframe.at(2.0 * shot_interval * beats_per_second) { emit_shot(third, third_aim) }
        Keyframe.at(0.45 * beats_per_second) { emit_data(completion, "BurstComplete") }
    }
    let automatic = Animation.paused().length(3.0 * auto_shot_interval * beats_per_second) {
        name = label + "_auto"
        // Sound is triggered by this visual event, so lookahead cannot queue an
        // extra automatic recording after the user releases the trigger.
        Keyframe.at(0.0) { emit_data(completion, "AutoCycle") }
        Keyframe.at(0.0) { emit_shot(first, first_aim) }
        Keyframe.at(auto_shot_interval * beats_per_second) { emit_shot(second, second_aim) }
        Keyframe.at(2.0 * auto_shot_interval * beats_per_second) { emit_shot(third, third_aim) }
    }
    fn begin_fire() {
        device_state.user = device_state.requested_user
        device_state.firing = true
        device_state.cancelled = false
        device_state.burst_mode = device_state.mode
        if device_state.mode == "auto" {
            automatic.loop_anim()
        } else {
            device_state.bursts_fired = device_state.bursts_fired + 1
            if device_state.mode == "burst" { burst.play() } else { semi.play() }
        }
    }
    on(completion, "DataEvent", fn(event) {
        if event == "BurstComplete" {
            device_state.firing = false
            if device_state.trigger_held && device_state.mode == "auto" { begin_fire() }
        }
        if event == "AutoCycle" && device_state.firing && !device_state.cancelled && device_state.burst_mode == "auto" {
            auto_sound.play()
            device_state.bursts_fired = device_state.bursts_fired + 1
        }
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
        AudioOutput { single_sound sound auto_sound }
        semi
        burst
        automatic
    }
    device_state.tool = root
    fn use_start(context) {
        if device_state.trigger_held { return false }
        device_state.trigger_held = true
        device_state.requested_user = context.user
        if device_state.firing { return false }
        begin_fire()
        return true
    }
    fn use_end() {
        device_state.cancelled = true
        device_state.trigger_held = false
        if device_state.burst_mode == "auto" {
            automatic.pause()
            device_state.firing = false
        }
        flash.stop()
    }
    let semi_label = Text { name = label + "_mode_semi_label" "# Semi" }
    let burst_label = Text { name = label + "_mode_burst_label" "  Burst" }
    let auto_label = Text { name = label + "_mode_auto_label" "  Auto" }
    fn set_mode(mode) {
        if mode != "semi" && mode != "burst" && mode != "auto" { return false }
        if mode != device_state.mode && device_state.burst_mode == "auto" && device_state.firing { use_end() }
        device_state.mode = mode
        semi_label.set_text("  Semi")
        burst_label.set_text("  Burst")
        auto_label.set_text("  Auto")
        if mode == "semi" { semi_label.set_text("# Semi") }
        if mode == "burst" { burst_label.set_text("# Burst") }
        if mode == "auto" { auto_label.set_text("# Auto") }
        return true
    }
    fn mode_option(mode, text) {
        let row = T {
            name = label + "_mode_" + mode
            Option {}
            Grabbable.off()
            Raycastable.click_only() { interaction_priority(150.0) }
            Style {
                display("flex") width(14.0) height(6.0) padding_xy(0.8, 0.0)
                align_items("center") font_size(1.7)
                color([0.0, 0.0, 0.0, 1.0])
                background_color([1.0, 0.86, 0.90, 1.0]) background_z(-0.04)
            }
            T.position(0.0, 0.0, 0.03) { text }
        }
        on(row, "Click", fn(event) { set_mode(mode) })
        return row
    }
    let mode_selector = T.scale(0.0018, 0.0018, 0.0018) {
        name = label + "_mode_selector"
        ui_unlit_material()
        Grabbable.off()
        LayoutRoot {
            available_width(14.0)
            T {
                name = label + "_mode_options"
                Selection.root("#" + label + "_mode_options").visual_feedback(false) { name = label + "_mode_selection" }
                Style { display("flex") flex_direction("column") width(14.0) row_gap(0.5) }
                mode_option("semi", semi_label)
                mode_option("burst", burst_label)
                mode_option("auto", auto_label)
            }
        }
    }
    return { root = root animation = burst semi_animation = semi auto_animation = automatic device_state = device_state use_start = use_start use_end = use_end flash = flash mode_selector = mode_selector set_mode = set_mode }
}
