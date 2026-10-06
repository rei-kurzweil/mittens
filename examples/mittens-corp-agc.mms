import { studio_stage, studio_floor } from "../assets/components/studio_stage.mms"
import { teleport_pit } from "../assets/components/teleport_pit.mms"
import { vroid_arm_ik } from "../assets/components/arm_ik/vroid.mms"

let arm_ik = vroid_arm_ik(-0.35, 1)

// mittens-corp-agc — XR car/controller regression scene and Rei(mu) pose-authoring tool.
//
// Run with:
//   cargo run --release -- load examples/mittens-corp-agc.mms
//
// The vehicle uses the generic inverse-local anchor operator rather than AVC:
// the rigid car has no humanoid specialization for AVC to perform.

import { tripod_light } from "../assets/components/tripod_light.mms"
import { bisket_anime_shading } from "../assets/components/materials/bisket_anime_shading.mms"
import { bisket_shirt_physics } from "../assets/components/secondary_motion/bisket-shirt-physics.mms"
import { rei_mu_bow_secondary_motion } from "../assets/components/secondary_motion/rei-mu-bow.mms"
import { bisket_colliders } from "../assets/components/colliders/bisket.mms"
import { bisket_humanoid_bone_map } from "../assets/components/humanoid_bone_maps/bisket.mms"
import { rei_2026_9 } from "../assets/components/mouth_response/rei_2026.9.mms"
import { ambient_eye_saccades } from "../assets/components/animations/ambient_eye_saccades.mms"
import { suspended_platform } from "../assets/components/platforms/suspended_platform.mms"
import { display_car_xr } from "../assets/components/vehicles/display_car.mms"
import { star_kawaii_background } from "../assets/components/backgrounds/star_kawaii_background.mms"
import { info_panel, info_panel_body } from "../assets/components/ui/info_panel.mms"
import { mouth_response_panel } from "../assets/components/ui/mouth_response_panel.mms"
import { button } from "../assets/components/button.mms"

// Optional sources stay neutral when the runtime or hardware is unavailable.
let microphone = AudioInput {}
let raw_voice_level = Amplitude.rolling_window(0.080).from(microphone) {}

// Keep the raw observer alive for diagnostics; AVC consumes only this adaptive
// analysis view. It changes no audible microphone samples.
let voice_level = VolumeNormalization.from(raw_voice_level) {}
let agc_mode = { enabled = true }
let mouth_panel_target = { avatar = null }
let mouth_response_preset = rei_2026_9()
let mouth_tuning = {
    center_rms = mouth_response_preset.rms_center
    range_rms = mouth_response_preset.rms_range
    amount = mouth_response_preset.mouth_movement_amount
}

// The response panel samples retained main-thread diagnostics rather than
// audio callback data. The graph is rebuilt from twelve scalar snapshots at
// 10 Hz, so the columns are always left-to-right chronological: oldest on the
// left, newest on the right. It continues to show the normalizer's decision
// even while B routes AVC to the raw meter.
fn fixed_3(value) {
    let scaled = Math.round(Math.abs(value) * 1000.0)
    let whole = Math.floor(scaled / 1000.0)
    let fraction = scaled - whole * 1000.0
    let padding = ""
    if fraction < 10.0 {
        padding = "00"
    } else if fraction < 100.0 {
        padding = "0"
    }
    let sign = ""
    if value > 0.0 {
        sign = "+"
    } else if value < 0.0 {
        sign = "-"
    }
    return sign + whole + "." + padding + fraction
}

// A blue history column represents one raw RMS snapshot entering the AGC.
fn make_level_history_bar(level_rms, index, visible, labels_enabled, colour, config) {
    let level = level_rms
    if level > config.level_extent { level = config.level_extent }
    if level < 0.0 { level = 0.0 }
    let level_height = level / config.level_extent * config.plot_height
    let level_base_y = -config.plot_height
    let level_y = level_base_y + level_height / 2.0
    let level_alpha = 0.0
    if visible { level_alpha = 1.0 }

    // Layout dimensions are glyph units but these manually authored blocks
    // are world transforms, so convert both position and size explicitly.
    let x_gu = config.column_width / 2.0 + index * (config.column_width + config.column_gap)
    return T.position(x_gu * config.unit_scale, 0.0, 0.0) {
        name = "agc_level_history_sample"
        T.position(0.0, level_y * config.unit_scale, 0.03).scale(
            config.column_width * config.unit_scale,
            level_height * config.unit_scale,
            config.column_depth * config.unit_scale,
        ) {
            name = "agc_level_history_block"
            R.cube() { C.rgba(colour[0], colour[1], colour[2], level_alpha) }
        }
        if labels_enabled && visible {
            T.position(0.0, level_y * config.unit_scale, 0.075)
                .rotation(0.0, 0.0, 1.570796)
                .scale(config.value_text_scale, config.value_text_scale, 1.0) {
                Text {
                    name = "agc_level_history_sample_text"
                    fixed_3(level_rms)
                    C.rgba(0.98, 0.99, 1.0, 1.0)
                    TextureFiltering.linear()
                }
            }
        }
    }
}

fn make_second_marker(index, config) {
    let x_gu = index * (config.column_width + config.column_gap)
    return T.position(
        x_gu * config.unit_scale,
        -config.plot_height / 2.0 * config.unit_scale,
        0.045,
    ).scale(
        config.second_marker_width * config.unit_scale,
        config.plot_height * config.unit_scale,
        config.column_depth * config.unit_scale,
    ) {
        name = "agc_second_marker"
        R.cube() { C.rgba(0.28, 0.78, 1.0, 0.90) EM.on() { intensity(1.8) } }
    }
}

fn make_input_history_view(history, labels_enabled, config) {
    let first_visible = config.max_samples - history.sample_count
    return T {
        name = "agc_input_history_view"
        make_level_history_bar(history.i0, 0.0, 0.0 >= first_visible, labels_enabled, [0.20, 0.60, 1.00], config)
        make_level_history_bar(history.i1, 1.0, 1.0 >= first_visible, labels_enabled, [0.20, 0.60, 1.00], config)
        make_level_history_bar(history.i2, 2.0, 2.0 >= first_visible, labels_enabled, [0.20, 0.60, 1.00], config)
        make_level_history_bar(history.i3, 3.0, 3.0 >= first_visible, labels_enabled, [0.20, 0.60, 1.00], config)
        make_level_history_bar(history.i4, 4.0, 4.0 >= first_visible, labels_enabled, [0.20, 0.60, 1.00], config)
        make_level_history_bar(history.i5, 5.0, 5.0 >= first_visible, labels_enabled, [0.20, 0.60, 1.00], config)
        make_level_history_bar(history.i6, 6.0, 6.0 >= first_visible, labels_enabled, [0.20, 0.60, 1.00], config)
        make_level_history_bar(history.i7, 7.0, 7.0 >= first_visible, labels_enabled, [0.20, 0.60, 1.00], config)
        make_level_history_bar(history.i8, 8.0, 8.0 >= first_visible, labels_enabled, [0.20, 0.60, 1.00], config)
        make_level_history_bar(history.i9, 9.0, 9.0 >= first_visible, labels_enabled, [0.20, 0.60, 1.00], config)
        make_level_history_bar(history.i10, 10.0, 10.0 >= first_visible, labels_enabled, [0.20, 0.60, 1.00], config)
        make_level_history_bar(history.i11, 11.0, 11.0 >= first_visible, labels_enabled, [0.20, 0.60, 1.00], config)
        if history.second_marker_index >= 0.0 {
            make_second_marker(history.second_marker_index, config)
        }
    }
}

// The gain track is deliberately dB, centered on zero: green is additional
// gain and pale red is attenuation. It is distinct from the raw-RMS track.
fn make_gain_history_bar(gain_db, index, visible, labels_enabled, config) {
    let magnitude = Math.abs(gain_db)
    if magnitude > config.db_extent { magnitude = config.db_extent }
    let height = magnitude / config.db_extent * config.plot_height / 2.0
    let zero_y = -config.plot_height / 2.0
    let y = zero_y
    let colour = [0.20, 1.00, 0.48, 0.0]
    if visible && gain_db > config.gain_epsilon_db {
        y = zero_y + height / 2.0
        colour = [0.20, 1.00, 0.48, 1.0]
    } else if visible && gain_db < -config.gain_epsilon_db {
        y = zero_y - height / 2.0
        colour = [1.00, 0.62, 0.62, 1.0]
    }
    let x_gu = config.column_width / 2.0 + index * (config.column_width + config.column_gap)
    return T.position(x_gu * config.unit_scale, 0.0, 0.0) {
        name = "agc_gain_history_sample"
        T.position(0.0, y * config.unit_scale, 0.03).scale(
            config.column_width * config.unit_scale,
            height * config.unit_scale,
            config.column_depth * config.unit_scale,
        ) {
            name = "agc_gain_history_block"
            R.cube() { C.rgba(colour[0], colour[1], colour[2], colour[3]) }
        }
        if labels_enabled && visible {
            T.position(0.0, y * config.unit_scale, 0.075)
                .rotation(0.0, 0.0, 1.570796)
                .scale(config.value_text_scale, config.value_text_scale, 1.0) {
                Text {
                    name = "agc_gain_history_sample_text"
                    fixed_3(gain_db)
                    C.rgba(0.98, 0.99, 1.0, 1.0)
                    TextureFiltering.linear()
                }
            }
        }
    }
}

fn make_gain_history_view(history, labels_enabled, config) {
    let first_visible = config.max_samples - history.sample_count
    return T {
        name = "agc_gain_history_view"
        make_gain_history_bar(history.g0, 0.0, 0.0 >= first_visible, labels_enabled, config)
        make_gain_history_bar(history.g1, 1.0, 1.0 >= first_visible, labels_enabled, config)
        make_gain_history_bar(history.g2, 2.0, 2.0 >= first_visible, labels_enabled, config)
        make_gain_history_bar(history.g3, 3.0, 3.0 >= first_visible, labels_enabled, config)
        make_gain_history_bar(history.g4, 4.0, 4.0 >= first_visible, labels_enabled, config)
        make_gain_history_bar(history.g5, 5.0, 5.0 >= first_visible, labels_enabled, config)
        make_gain_history_bar(history.g6, 6.0, 6.0 >= first_visible, labels_enabled, config)
        make_gain_history_bar(history.g7, 7.0, 7.0 >= first_visible, labels_enabled, config)
        make_gain_history_bar(history.g8, 8.0, 8.0 >= first_visible, labels_enabled, config)
        make_gain_history_bar(history.g9, 9.0, 9.0 >= first_visible, labels_enabled, config)
        make_gain_history_bar(history.g10, 10.0, 10.0 >= first_visible, labels_enabled, config)
        make_gain_history_bar(history.g11, 11.0, 11.0 >= first_visible, labels_enabled, config)
        if history.second_marker_index >= 0.0 {
            make_second_marker(history.second_marker_index, config)
        }
    }
}

fn make_level_history_plot(name, layers_name, view, config) {
    return T {
        name = name
        Style {
            display("block")
            width(config.plot_width)
            height(config.plot_height)
            margin_top(0.10)
            background_color([0.025, 0.030, 0.040, 0.96])
            background_z(-0.01)
        }
        T.position(
            config.plot_width / 2.0 * config.unit_scale,
            -config.plot_height * config.unit_scale,
            0.01,
        ).scale(
            config.plot_width * config.unit_scale,
            0.05 * config.unit_scale,
            0.04 * config.unit_scale,
        ) {
            R.cube() { C.rgba(0.70, 0.78, 0.90, 0.55) }
        }
        T {
            name = layers_name
            view
        }
    }
}

fn make_gain_history_plot(history, labels_enabled, config) {
    return T {
        name = "agc_gain_history_plot"
        Style {
            display("block")
            width(config.plot_width)
            height(config.plot_height)
            margin_top(0.10)
            background_color([0.025, 0.030, 0.040, 0.96])
            background_z(-0.01)
        }
        T.position(
            config.plot_width / 2.0 * config.unit_scale,
            -config.plot_height / 2.0 * config.unit_scale,
            0.01,
        ).scale(
            config.plot_width * config.unit_scale,
            0.05 * config.unit_scale,
            0.04 * config.unit_scale,
        ) {
            R.cube() { C.rgba(0.82, 0.86, 0.94, 0.65) }
        }
        T {
            name = "agc_gain_history_layers"
            make_gain_history_view(history, labels_enabled, config)
        }
    }
}

fn make_level_history_content(history, config) {
    let numeric_toggle_label = "hide numeric labels"
    if !history.labels_enabled {
        numeric_toggle_label = "show numeric labels"
    }
    let numeric_toggle = button(numeric_toggle_label, {
        background_color = [0.13, 0.35, 0.48, 1.0]
        color = [0.94, 0.98, 1.0, 1.0]
        compact = true
    })
    let numeric_toggle_text = numeric_toggle.query("Text")
    on(numeric_toggle, "Click", fn(event) {
        if history.labels_enabled {
            history.labels_enabled = false
            numeric_toggle_text.set_text("show numeric labels")
        } else {
            history.labels_enabled = true
            numeric_toggle_text.set_text("hide numeric labels")
        }
        // Rebuild once at the toggle edge. Subsequent 100 ms frames omit the
        // label subtree entirely while the numeric readout is off.
        let input_layers = query("#agc_input_history_layers")
        if input_layers {
            let previous_view = input_layers.query("#agc_input_history_view")
            if previous_view { previous_view.remove_subtree() }
            input_layers.attach(make_input_history_view(history, history.labels_enabled, config))
        }
        let gain_layers = query("#agc_gain_history_layers")
        if gain_layers {
            let previous_view = gain_layers.query("#agc_gain_history_view")
            if previous_view { previous_view.remove_subtree() }
            gain_layers.attach(make_gain_history_view(history, history.labels_enabled, config))
        }
    })
    return T {
        name = "agc_response_content"
        Style {
            display("flex")
            flex_direction("column")
            width(100%)
            row_gap(0.30)
        }
        Text {
            name = "agc_current_gain_text"
            ""
        }
        T {
            name = "agc_numeric_toggle"
            numeric_toggle
        }
        T {
            Style {
                display("block")
                width(100%)
                color([0.75, 0.80, 0.90, 1.0])
            }
            Text { "12 × 100 ms  ·  green = added gain  ·  pale red = removed gain  ·  blue = raw input RMS" }
        }
        T { Text { "gain adjustment (dB)" } }
        make_gain_history_plot(history, history.labels_enabled, config)
        T { Text { "input to AGC" } }
        make_level_history_plot("agc_input_history_plot", "agc_input_history_layers", make_input_history_view(history, history.labels_enabled, config), config)
        T {
            Style {
                display("block")
                width(100%)
                color([0.70, 0.74, 0.82, 1.0])
            }
            Text { "oldest <-                         -> newest" }
        }
    }
}

fn gain_db_label(gain_db) {
    let rounded = Math.round(gain_db * 10.0) / 10.0
    let sign = ""
    if rounded >= 0.0 { sign = "+" }
    return sign + rounded + " dB"
}

fn make_level_history_graph(raw_level, level, config) {
    let history = {
        elapsed_sec = 0.0
        active = true
        sample_count = 0.0
        samples_until_second = 10.0
        second_marker_index = -1.0
        labels_enabled = false
        i0 = 0.0 i1 = 0.0 i2 = 0.0 i3 = 0.0 i4 = 0.0 i5 = 0.0
        i6 = 0.0 i7 = 0.0 i8 = 0.0 i9 = 0.0 i10 = 0.0 i11 = 0.0
        g0 = 0.0 g1 = 0.0 g2 = 0.0 g3 = 0.0 g4 = 0.0 g5 = 0.0
        g6 = 0.0 g7 = 0.0 g8 = 0.0 g9 = 0.0 g10 = 0.0 g11 = 0.0
    }
    let initial_content = make_level_history_content(history, config)
    let response_panel = info_panel({
        root_name = "agc_response_panel"
        width_gu = config.panel_width
        unit_scale = config.unit_scale
        title = "AGC response"
        background_color = [0.12, 0.20, 0.17, 0.98]
        toggle_background_color = [0.18, 0.42, 0.30, 1.0]
        content = initial_content
    })
    let graph = T.position(config.panel_x, config.panel_y, config.panel_z) {
        name = "agc_response_panel_anchor"
        // This marker makes the whole info panel grip-grabbable in XR. Its
        // built-in title bar remains desktop-draggable.
        Grabbable {}
        response_panel
    }
    let body_mount = graph.query("#accordion_body_mount")

    on_global("FrameTick", fn(event) {
        // A minimized accordion has no graph body to update. More importantly,
        // it must not continue doing the retained reads or rebuilding views.
        if history.active {
            history.elapsed_sec = history.elapsed_sec + event.dt_sec
            let should_sample = false
            if config.sample_period_sec == 0.0 {
                should_sample = true
            } else if history.elapsed_sec >= config.sample_period_sec {
                // Retain only the remainder: a long frame produces one current
                // visual sample, never a burst of duplicate historical columns.
                history.elapsed_sec = history.elapsed_sec - config.sample_period_sec
                should_sample = true
            }
            if should_sample {
            let input_rms = raw_level.value()
            let gain_db = level.gain_db()
            history.i0 = history.i1
            history.i1 = history.i2
            history.i2 = history.i3
            history.i3 = history.i4
            history.i4 = history.i5
            history.i5 = history.i6
            history.i6 = history.i7
            history.i7 = history.i8
            history.i8 = history.i9
            history.i9 = history.i10
            history.i10 = history.i11
            history.i11 = input_rms
            history.g0 = history.g1
            history.g1 = history.g2
            history.g2 = history.g3
            history.g3 = history.g4
            history.g4 = history.g5
            history.g5 = history.g6
            history.g6 = history.g7
            history.g7 = history.g8
            history.g8 = history.g9
            history.g9 = history.g10
            history.g10 = history.g11
            history.g11 = gain_db
            history.samples_until_second = history.samples_until_second - 1.0
            if history.samples_until_second <= 0.0 {
                // Stamp the new whole-second boundary at the newest edge.
                // Subsequent snapshots carry it left with the rolling window.
                history.samples_until_second = 10.0
                history.second_marker_index = 11.0
            } else if history.second_marker_index >= 0.0 {
                history.second_marker_index = history.second_marker_index - 1.0
            }
            if history.sample_count < config.max_samples {
                history.sample_count = history.sample_count + 1.0
            }
            let input_layers = graph.query("#agc_input_history_layers")
            if input_layers {
                let previous_view = input_layers.query("#agc_input_history_view")
                if previous_view { previous_view.remove_subtree() }
                input_layers.attach(make_input_history_view(history, history.labels_enabled, config))
            }
            let gain_layers = graph.query("#agc_gain_history_layers")
            if gain_layers {
                let previous_view = gain_layers.query("#agc_gain_history_view")
                if previous_view { previous_view.remove_subtree() }
                gain_layers.attach(make_gain_history_view(history, history.labels_enabled, config))
            }
            if history.labels_enabled {
                let current_gain_text = graph.query("#agc_current_gain_text")
                if current_gain_text {
                    current_gain_text.set_text("input: " + fixed_3(input_rms) + "  ·  gain: " + gain_db_label(gain_db))
                }
            }
            }
        }
    })

    on(graph, "DataEvent", fn(event) {
        if event == "AccordionMinimized" {
            history.active = false
        } else if event == "AccordionRestoreRequested" {
            // The info-panel asset intentionally removes its body while
            // minimized. Recreate this dynamic body and resume sampling on
            // its next normal 100 ms update.
            history.active = true
            history.elapsed_sec = 0.0
            body_mount.attach(info_panel_body({
                content = make_level_history_content(history, config)
            }))
        }
    })

    return graph
}

let agc_level_graph = make_level_history_graph(raw_voice_level, voice_level, {
    sample_period_sec = 0.100
    max_samples = 12.0
    // Blue input-RMS display scale. The gain track uses db_extent instead.
    level_extent = 0.10
    db_extent = 24.0
    gain_epsilon_db = 0.05
    column_width = 2.05
    column_gap = 0.34
    column_depth = 0.22
    second_marker_width = 0.12
    value_text_scale = 0.018
    plot_height = 5.0
    plot_width = 28.7
    panel_width = 32.0
    unit_scale = 0.08
    panel_x = -1.30
    panel_y = 2.05
    panel_z = 1.35
})
agc_level_graph

let mouth_response_settings = mouth_response_panel({
    root_name = "agc_mouth_response_settings_panel"
    title = "Voice response"
    avatar_slot = mouth_panel_target
    tuning = mouth_tuning
    description = "Map the AGC-adjusted microphone level to mouth movement. These controls do not alter AGC policy or audio input."
})
T.position(1.60, 2.05, 1.35) {
    name = "agc_mouth_response_settings_anchor"
    Grabbable {}
    mouth_response_settings
}

// A deliberately authored layout panel, rather than a default/debug label:
// black backing makes the amber readout legible in both the studio and mirror.
// B on the XR gamepad changes this label and the actual AVC source together.
let agc_status_glow = EM.on() { intensity(2.4) }
let agc_status_text = Text {
    name = "agc_status_text"
    "AGC = ON\nB: raw amplitude"
    C.rgba(1.0, 0.56, 0.10, 1.0)
    agc_status_glow
}
let agc_status_panel = T.position(0.0, 2.35, 1.35) {
    name = "agc_status_panel"
    LayoutRoot {
        available_width(22.0)
        unit_scale(0.08)
        T {
            name = "agc_status_backing"
            Style {
                display("flex")
                width(22.0)
                height(6.5)
                align_items("center")
                justify_content("center")
                background_color([0.0, 0.0, 0.0, 0.96])
                background_z(-0.02)
                text_align("center")
                color([1.0, 0.56, 0.10, 1.0])
            }
            T.position(0.0, 0.0, 0.03) { agc_status_text }
        }
    }
}
agc_status_panel

RendererSettings { window_size(1440, 810) }
BGC.rgba(0.055, 0.055, 0.060, 1.0)
AL.rgb(0.13, 0.13, 0.15)
// The default engine clock is 120 BPM, i.e. two beats per second. Keep this
// explicit because ambient_eye_saccades authors its keyframe times in seconds.
Clock.bpm(120.0)

BG.occlusion_and_lighting() {
    star_kawaii_background([1.0, 0.84, 0.24, 1.0])
}

RenderGraph {
    EmissivePass { BlurPass { radius_ndc(0.025) half_res(true) } }
    Bloom { intensity(0.42) radius_ndc(0.025) emissive_scale(1.0) half_res(true) }
}


// Keep the car example's studio stage so this tool also retains a stable
// lighting and mirror reference while XR transforms and gizmos are exercised.
studio_floor()
studio_stage("mittens_corp_stage")
teleport_pit("studio_teleport_pit", [0.0, -14.0, 0.0], [100.0, 12.0, 100.0], [-5.0, 1.2, 0.0], "spikes")

T.position(0.0, 2.55, 8.10).scale(1.5, 1.5, 0.08).rotation(0.0, 3.1416, 0.0) {
    name = "stage_mirror"
    Grabbable {}
    R.cube() {
        Mirror.quality(1440) {}
        Raycastable.enabled()
    }
}


// Three elevated walkway sections run along Z, perpendicular to the stage's
// long X axis. Their ends meet to form one continuous suspended platform.
for platform_index in range(3) {
    T.position(11.5, 4.0, (platform_index - 1) * 15.0) {
        suspended_platform()
    }
}

let subject_light_target = [0.0, 1.75, 1.7]
tripod_light(
    "front_left_studio_light",
    [-10.5, 0.14, 4.4],
    subject_light_target,
    SL.color(1.0, 0.82, 0.70).intensity(10.0).distance(22.0).angle(0.58).penumbra(0.32),
)
tripod_light(
    "front_right_studio_light",
    [10.5, 0.14, 4.4],
    subject_light_target,
    SL.color(0.72, 0.84, 1.0).intensity(10.0).distance(22.0).angle(0.58).penumbra(0.32),
)
tripod_light(
    "rear_left_studio_light",
    [-10.5, 0.14, -5.7],
    subject_light_target,
    SL.color(0.72, 0.84, 1.0).intensity(8.0).distance(20.0).angle(0.62).penumbra(0.38),
)
tripod_light(
    "rear_right_studio_light",
    [10.5, 0.14, -5.7],
    subject_light_target,
    SL.color(1.0, 0.78, 0.68).intensity(8.0).distance(20.0).angle(0.62).penumbra(0.38),
)

// Kawaii cat spectators provide nearby scale and motion references while tuning
// the player's mouth response.
T.position(-3.8, 0.15, 3.2).rotation(0.0, 2.65, 0.0).scale(0.45, 0.45, 0.45) {
    name = "volume_tuning_cat_left"
    Grabbable {}
    GLTF.new("assets/models/color-cat.2.glb") {}
}
T.position(0.0, 0.15, 4.0).rotation(0.0, 3.14159, 0.0).scale(0.38, 0.38, 0.38) {
    name = "volume_tuning_cat_center"
    Grabbable {}
    GLTF.new("assets/models/color-cat.2.glb") {}
}
T.position(3.8, 0.15, 3.2).rotation(0.0, 3.63, 0.0).scale(0.45, 0.45, 0.45) {
    name = "volume_tuning_cat_right"
    Grabbable {}
    GLTF.new("assets/models/color-cat.2.glb") {}
}

// Rei(mu) is the player rig. InputXR continues to own tracked head translation
// and rotation; only the gamepad's built-in locomotion mapping is handed off
// when a vehicle layer takes movement authority.
ED.active() {
    let vehicle_controls = InputXRGamepad {
        name = "bisket_pedestrian_locomotion"
        locomotion()
        speed(1.5)
    }

    let player_motion = Velocity {
        name = "bisket_locomotion_root_velocity"
        T.position(-5.0, 1.0, 0.0) {
            name = "bisket_locomotion_root"
            Rider
                .anchor("[name='bisket_rider_cxr_anchor']")
                .movement_root("[name='bisket_locomotion_root']")
                .input("[name='bisket_pedestrian_locomotion']") {}
            InputXR.on() {
                vehicle_controls

                T {
                    name = "bisket_xr_driver"
                    let bisket_avatar = GLTF.new("assets/models/rei(mu).glb") {
                        bisket_anime_shading()
                        bisket_humanoid_bone_map()
                        MorphTargetMap.new()
                            .slot("left_eye_blink", "Fcl_EYE_Close_L")
                            .slot("right_eye_blink", "Fcl_EYE_Close_R")
                            .slot("viseme_aa", "Fcl_MTH_A")
                        EM.on()
                        PoseCapture { label("Rei(mu)") asset_name("rei_mu") }
                        bisket_colliders()
                        bisket_shirt_physics(false)
                        rei_mu_bow_secondary_motion()
                    }
                    let bisket_avatar_control = AVC.movement_target("[name='bisket_locomotion_root']") {
                        name = "bisket_avatar_control"
                        mouth_open_from_amplitude(voice_level)
                        mouth_open_rms_center_range(mouth_tuning.center_rms, mouth_tuning.range_rms)
                        mouth_open_amount(mouth_tuning.amount)
                        mouth_open_smoothing(16.0)
                        voice_level

                        initial_yaw(3.14159)
                        left_two_bone_ik(arm_ik.left)
                        right_two_bone_ik(arm_ik.right)
                        hand_rotation_smoothing(220.0)

                        T {
                            bisket_avatar
                        }

                        // Rider-side anchor. AVC reparents this wrapper beneath the
                        // head; mounting aligns it with the car's cockpit target.
                        T.position(0.0, 0.08, 0.12) {
                            name = "bisket_rider_cxr_anchor"
                            CXR { Pointer {} }
                        }
                        // HTC eye tracking retains closure samples for blink morphs,
                        // while authored animation owns the eye-bone direction.
                        XREyeTracking.on().priority(["htc", "vrchat_osc"]).enable_pupil_direction_tracking(false)

                        XRHand.new(true, "Left", "GripAim").laser() {
                            T {
                                RestAttachment.new("[name='J_Bip_L_Hand']", "[name='J_Bip_L_Middle3']") {
                                    Pointer {}
                                }
                            }
                        }
                        XRHand.new(true, "Right", "GripAim").laser() {
                            T {
                                RestAttachment.new("[name='J_Bip_R_Hand']", "[name='J_Bip_R_Middle3']") {
                                    Pointer {}
                                }
                            }
                        }
                    }
                    mouth_panel_target.avatar = bisket_avatar_control
                    bisket_avatar_control

                    on(vehicle_controls, "XrButtonDown", fn(event) {
                        if event.control == "ButtonB" {
                            if agc_mode.enabled {
                                // Keep normalization live for its diagnostics, but
                                // route AVC to the raw observer for A/B tuning.
                                agc_mode.enabled = false
                                bisket_avatar_control.mouth_open_from_amplitude(raw_voice_level)
                                agc_status_text.set_text("AGC = OFF\nB: normalized AGC")
                                agc_status_glow.set_intensity(0.28)
                            } else {
                                agc_mode.enabled = true
                                bisket_avatar_control.mouth_open_from_amplitude(voice_level)
                                agc_status_text.set_text("AGC = ON\nB: raw amplitude")
                                agc_status_glow.set_intensity(2.4)
                            }
                        }
                    })

                    // The explicit Rei(mu) humanoid map above declares these two
                    // skin-joint targets. Query only this GLTF instance after it
                    // finishes importing, so another avatar cannot be animated.
                    on(bisket_avatar, "GLTFInitialized", fn(event) {
                        let left_eye = event.gltf.query("[name='J_Adj_L_FaceEye']")
                        let right_eye = event.gltf.query("[name='J_Adj_R_FaceEye']")
                        if left_eye && right_eye {
                            bisket_avatar_control.attach(ambient_eye_saccades(left_eye, right_eye, 2.0))
                        } else {
                            print("GLTFInitialized: Rei(mu) mapped eye bones were not found; ambient eye animation was not attached")
                        }
                    })
                }
            }
        }
    }
    Gravity { player_motion }
    on_global("XrButtonDown", fn(event) {
        if event.control == "ButtonY" && player_motion.grounded() { player_motion.translate_world([0.0, 4.5, 0.0]) }
    })

    let car_root = display_car_xr(
        "left_display_car",
        [-19.0, -0.75, -1.5],
        0.30,
        "left_display_car_cxr_mount",
        vehicle_controls,
    )
    car_root
}

// Explicit selection disables every editor window except the two needed for
// pose-authoring and transform/gizmo diagnostics.
T.position(1.25, 2.8, -1.5) {
    name = "mittens_corp_editor_ui"
    EditorUI {
        panels([
            { panel = "settings" config = { show_zones = true } },
            { panel = "pose" },
        ])
    }
}

XR.on()
