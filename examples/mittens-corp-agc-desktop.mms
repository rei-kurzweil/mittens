// mittens-corp-agc-desktop — desktop microphone AGC tuning with Bisket.
//
// Run with:
//   cargo run --release -- load examples/mittens-corp-agc-desktop.mms
//
// WASD + mouse drive Bisket through the ordinary desktop Input pose driver.
// The sliders publish policy changes to the existing audio callback unit;
// they do not reopen the microphone or recreate its rolling buffers.

import { bisket_anime_shading } from "../assets/components/materials/bisket_anime_shading.mms"
import { bisket_humanoid_bone_map } from "../assets/components/humanoid_bone_maps/bisket.mms"
import { rei_2026_9 } from "../assets/components/mouth_response/rei_2026.9.mms"
import { pose as relaxed_pose_factory } from "../assets/components/poses/bisket/000-relaxed.pose.mms"
import { bisket_shirt_physics } from "../assets/components/secondary_motion/bisket-shirt-physics.mms"

import { ambient_eye_saccades } from "../assets/components/animations/ambient_eye_saccades.mms"
import { tripod_light } from "../assets/components/tripod_light.mms"
import { truss } from "../assets/components/truss.mms"
import { suspended_platform } from "../assets/components/platforms/suspended_platform.mms"
import { star_kawaii_background } from "../assets/components/backgrounds/star_kawaii_background.mms"
import { button } from "../assets/components/button.mms"
import { info_panel, info_panel_body } from "../assets/components/ui/info_panel.mms"
import { mouth_response_panel } from "../assets/components/ui/mouth_response_panel.mms"

let microphone = AudioInput {}
let raw_voice_level = Amplitude.rolling_window(0.080).from(microphone) {}
let voice_level = VolumeNormalization.from(raw_voice_level) {}

// Keep AGC policy at the component defaults for this pass. Its detailed
// controls are deliberately out of the panel while we establish a small,
// reliable mouth-response baseline below.
let mouth_response_preset = rei_2026_9()
let mouth_tuning = {
    center_rms = mouth_response_preset.rms_center
    range_rms = mouth_response_preset.rms_range
    amount = mouth_response_preset.mouth_movement_amount
}

fn fixed_3(value) {
    let scaled = Math.round(Math.abs(value) * 1000.0)
    let whole = Math.floor(scaled / 1000.0)
    let fraction = scaled - whole * 1000.0
    let padding = ""
    if fraction < 10.0 { padding = "00" } else if fraction < 100.0 { padding = "0" }
    let sign = ""
    if value > 0.0 { sign = "+" } else if value < 0.0 { sign = "-" }
    return sign + whole + "." + padding + fraction
}

// The response monitor is intentionally independent from the mouth-response
// settings panel. It samples retained main-thread diagnostics at 10 Hz: blue
// bars are input RMS and the dB track is the normalizer's signed gain.
fn make_level_history_bar(level_rms, index, visible, labels_enabled, colour, config) {
    let level = level_rms
    if level > config.level_extent { level = config.level_extent }
    if level < 0.0 { level = 0.0 }
    let height = level / config.level_extent * config.plot_height
    let y = -config.plot_height + height / 2.0
    let alpha = 0.0
    if visible { alpha = 1.0 }
    let x_gu = config.column_width / 2.0 + index * (config.column_width + config.column_gap)
    return T.position(x_gu * config.unit_scale, 0.0, 0.0) {
        name = "agc_level_history_sample"
        T.position(0.0, y * config.unit_scale, 0.03).scale(
            config.column_width * config.unit_scale,
            height * config.unit_scale,
            config.column_depth * config.unit_scale,
        ) {
            R.cube() { C.rgba(colour[0], colour[1], colour[2], alpha) }
        }
        if labels_enabled && visible {
            T.position(0.0, y * config.unit_scale, 0.075)
                .rotation(0.0, 0.0, 1.570796)
                .scale(config.value_text_scale, config.value_text_scale, 1.0) {
                Text { fixed_3(level_rms) C.rgba(0.98, 0.99, 1.0, 1.0) TextureFiltering.linear() }
            }
        }
    }
}

fn make_second_marker(index, config) {
    let x_gu = index * (config.column_width + config.column_gap)
    return T.position(x_gu * config.unit_scale, -config.plot_height / 2.0 * config.unit_scale, 0.045).scale(
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
        if history.second_marker_index >= 0.0 { make_second_marker(history.second_marker_index, config) }
    }
}

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
            R.cube() { C.rgba(colour[0], colour[1], colour[2], colour[3]) }
        }
        if labels_enabled && visible {
            T.position(0.0, y * config.unit_scale, 0.075)
                .rotation(0.0, 0.0, 1.570796)
                .scale(config.value_text_scale, config.value_text_scale, 1.0) {
                Text { fixed_3(gain_db) C.rgba(0.98, 0.99, 1.0, 1.0) TextureFiltering.linear() }
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
        if history.second_marker_index >= 0.0 { make_second_marker(history.second_marker_index, config) }
    }
}

fn make_history_plot(name, layers_name, view, zero_line_y, config) {
    return T {
        name = name
        Style {
            display("block") width(config.plot_width) height(config.plot_height) margin_top(0.10)
            background_color([0.025, 0.030, 0.040, 0.96]) background_z(-0.01)
        }
        T.position(config.plot_width / 2.0 * config.unit_scale, zero_line_y * config.unit_scale, 0.01).scale(
            config.plot_width * config.unit_scale, 0.05 * config.unit_scale, 0.04 * config.unit_scale,
        ) { R.cube() { C.rgba(0.76, 0.82, 0.94, 0.62) } }
        T { name = layers_name view }
    }
}

fn rebuild_history_views(history, config) {
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
}

fn make_agc_response_content(history, config) {
    let toggle_label = "show numeric labels"
    if history.labels_enabled { toggle_label = "hide numeric labels" }
    let numeric_toggle = button(toggle_label, {
        background_color = [0.13, 0.35, 0.48, 1.0] color = [0.94, 0.98, 1.0, 1.0] compact = true
    })
    let toggle_text = numeric_toggle.query("Text")
    on(numeric_toggle, "Click", fn(event) {
        history.labels_enabled = !history.labels_enabled
        if history.labels_enabled { toggle_text.set_text("hide numeric labels") } else { toggle_text.set_text("show numeric labels") }
        rebuild_history_views(history, config)
    })
    return T {
        name = "agc_response_content"
        Style { display("flex") flex_direction("column") width(100%) row_gap(0.30) }
        Text { name = "agc_current_gain_text" "" }
        T { name = "agc_numeric_toggle" numeric_toggle }
        T {
            Style { display("block") width(100%) color([0.75, 0.80, 0.90, 1.0]) font_size(0.60) }
            Text { "12 × 100 ms · green = added gain · pale red = removed gain · blue = input RMS" }
        }
        T { Text { "gain adjustment (dB)" } }
        make_history_plot("agc_gain_history_plot", "agc_gain_history_layers", make_gain_history_view(history, history.labels_enabled, config), -config.plot_height / 2.0, config)
        T { Text { "input to AGC" } }
        make_history_plot("agc_input_history_plot", "agc_input_history_layers", make_input_history_view(history, history.labels_enabled, config), -config.plot_height, config)
        T {
            Style { display("block") width(100%) color([0.70, 0.74, 0.82, 1.0]) }
            Text { "oldest <-                     -> newest" }
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
        elapsed_sec = 0.0 sample_count = 0.0 samples_until_second = 10.0 second_marker_index = -1.0 labels_enabled = false
        i0 = 0.0 i1 = 0.0 i2 = 0.0 i3 = 0.0 i4 = 0.0 i5 = 0.0 i6 = 0.0 i7 = 0.0 i8 = 0.0 i9 = 0.0 i10 = 0.0 i11 = 0.0
        g0 = 0.0 g1 = 0.0 g2 = 0.0 g3 = 0.0 g4 = 0.0 g5 = 0.0 g6 = 0.0 g7 = 0.0 g8 = 0.0 g9 = 0.0 g10 = 0.0 g11 = 0.0
    }
    let response_panel = info_panel({
        root_name = "agc_response_panel" width_gu = config.panel_width unit_scale = config.unit_scale title = "AGC response"
        background_color = [0.12, 0.20, 0.17, 0.98] toggle_background_color = [0.18, 0.42, 0.30, 1.0]
        content = make_agc_response_content(history, config)
    })
    let graph = T.position(config.panel_x, config.panel_y, config.panel_z) {
        name = "agc_response_panel_anchor"
        response_panel
    }
    let body_mount = graph.query("#accordion_body_mount")

    on_global("FrameTick", fn(event) {
        history.elapsed_sec = history.elapsed_sec + event.dt_sec
        let should_sample = false
        if config.sample_period_sec == 0.0 {
            should_sample = true
        } else if history.elapsed_sec >= config.sample_period_sec {
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
                history.samples_until_second = 10.0
                history.second_marker_index = 11.0
            } else if history.second_marker_index >= 0.0 {
                history.second_marker_index = history.second_marker_index - 1.0
            }
            if history.sample_count < config.max_samples { history.sample_count = history.sample_count + 1.0 }
            rebuild_history_views(history, config)
            if history.labels_enabled {
                let current_text = graph.query("#agc_current_gain_text")
                if current_text { current_text.set_text("input: " + fixed_3(input_rms) + " · gain: " + gain_db_label(gain_db)) }
            }
        }
    })
    on(graph, "DataEvent", fn(event) {
        if event == "AccordionRestoreRequested" {
            body_mount.attach(info_panel_body({ content = make_agc_response_content(history, config) }))
        }
    })
    return graph
}

RendererSettings { window_size(1440, 810) }
BGC.rgba(0.055, 0.055, 0.060, 1.0)
AL.rgb(0.13, 0.13, 0.15)
Clock.bpm(120.0)

BG.occlusion_and_lighting() {
    star_kawaii_background([1.0, 0.84, 0.24, 1.0])
}

RenderGraph {
    EmissivePass { BlurPass { radius_ndc(0.025) half_res(true) } }
    Bloom { intensity(0.42) radius_ndc(0.025) emissive_scale(1.0) half_res(true) }
}

fn stage_box(box_name, position, size, color) {
    return T.position(position[0], position[1], position[2])
        .scale(size[0], size[1], size[2]) {
        name = box_name
        R.cube() { C.rgba(color[0], color[1], color[2], 1.0) }
    }
}

// Keep the complete studio dressing from the XR tuning scene. It gives the
// desktop first-person view an immediate landmark, reflected avatar view, and
// sensible lighting instead of a clear-colour-only window.
stage_box("studio_floor", [0.0, -0.92, 1.0], [54.0, 0.14, 32.0], [0.035, 0.037, 0.043])
stage_box("stage_deck", [0.0, 0.00, -1.5], [32.0, 0.24, 14.0], [0.18, 0.18, 0.20])
stage_box("stage_upper_step", [0.0, -0.24, 5.7], [32.0, 0.28, 0.8], [0.14, 0.14, 0.16])
stage_box("stage_lower_step", [0.0, -0.56, 6.3], [32.0, 0.36, 0.8], [0.10, 0.10, 0.12])
stage_box("stage_back_wall", [0.0, 4.00, -8.35], [32.0, 8.00, 0.35], [0.105, 0.105, 0.12])

T.position(0.0, 2.55, 8.10).scale(1.5, 1.5, 0.08).rotation(0.0, 3.1416, 0.0) {
    name = "stage_mirror"
    Grabbable {}
    R.cube() {
        Mirror.quality(1440) {}
        Raycastable.enabled()
    }
}

T.position(0.0, 7.10, -7.75) {
    name = "stage_ceiling_truss"
    truss(26)
}
for platform_index in range(3) {
    T.position(11.5, 4.0, (platform_index - 1) * 15.0) {
        suspended_platform()
    }
}

let subject_light_target = [0.0, 1.75, 1.7]
tripod_light(
    "front_left_studio_light", [-10.5, 0.14, 4.4], subject_light_target,
    SL.color(1.0, 0.82, 0.70).intensity(10.0).distance(22.0).angle(0.58).penumbra(0.32),
)
tripod_light(
    "front_right_studio_light", [10.5, 0.14, 4.4], subject_light_target,
    SL.color(0.72, 0.84, 1.0).intensity(10.0).distance(22.0).angle(0.58).penumbra(0.32),
)
tripod_light(
    "rear_left_studio_light", [-10.5, 0.14, -5.7], subject_light_target,
    SL.color(0.72, 0.84, 1.0).intensity(8.0).distance(20.0).angle(0.62).penumbra(0.38),
)
tripod_light(
    "rear_right_studio_light", [10.5, 0.14, -5.7], subject_light_target,
    SL.color(1.0, 0.78, 0.68).intensity(8.0).distance(20.0).angle(0.62).penumbra(0.38),
)

T.position(-3.8, 0.15, 3.2).rotation(0.0, 2.65, 0.0).scale(0.45, 0.45, 0.45) {
    name = "agc_desktop_cat_left"
    Grabbable {}
    GLTF.new("assets/models/color-cat.2.glb") {}
}
T.position(0.0, 0.15, 4.0).rotation(0.0, 3.14159, 0.0).scale(0.38, 0.38, 0.38) {
    name = "agc_desktop_cat_center"
    Grabbable {}
    GLTF.new("assets/models/color-cat.2.glb") {}
}
T.position(3.8, 0.15, 3.2).rotation(0.0, 3.63, 0.0).scale(0.45, 0.45, 0.45) {
    name = "agc_desktop_cat_right"
    Grabbable {}
    GLTF.new("assets/models/color-cat.2.glb") {}
}

let bisket = GLTF.new("assets/models/bisket.glb") {
    bisket_anime_shading()
    bisket_humanoid_bone_map()
    // Direct pose child applies Bisket's captured relaxed stance as the
    // one-shot startup overlay after the model imports.
    MorphTargetMap.new().slot("viseme_aa", "Fcl_MTH_A")
    
    relaxed_pose_factory()
    bisket_shirt_physics(false)

    EM.on()
}

let avatar = AVC {
    name = "agc_desktop_avatar_control"
    mouth_open_from_amplitude(voice_level)
    // The performer-facing centre/range form is equivalent to the legacy
    // 0.003 floor and 0.060 ceiling, while retaining those builders as an
    // equally valid exact-calibration form.
    mouth_open_rms_center_range(mouth_tuning.center_rms, mouth_tuning.range_rms)
    mouth_open_amount(mouth_tuning.amount)
    mouth_open_smoothing(16.0)
    initial_yaw(3.14159)
    T { bisket }
    // AVC treats this one-transform Camera3D wrapper the same as a CameraXR
    // wrapper: it mounts the path beneath Bisket's camera anchor after the
    // humanoid map is ready.
    T.position(0.0, 0.08, 0.06).rotation(0.0, 3.14159, 0.0) {
        name = "agc_desktop_camera_rig"
        C3D { Pointer { Raycast.event_driven().min_distance(0.75) {} } }
    }
}

on(bisket, "GLTFInitialized", fn(event) {
    let left_eye = event.gltf.query("[name='J_Adj_L_FaceEye']")
    let right_eye = event.gltf.query("[name='J_Adj_R_FaceEye']")
    if left_eye && right_eye {
        avatar.attach(ambient_eye_saccades(left_eye, right_eye, 2.0))
    }
})

// Ordinary desktop pose driver: no XR input, HMD, or controller is required.
ED.active() {
    I.speed(2.2) {
        name = "agc_desktop_input"
        InputTransformMode.forward_z() { roll_axis_y() fps_rotation() }
        T.position(0.0, 1.6, 1.0) {
            name = "agc_desktop_avatar_driver"
            avatar
        }
    }
}

let settings_panel = mouth_response_panel({
    root_name = "agc_desktop_settings_panel"
    title = "Voice response"
    avatar_slot = { avatar = avatar }
    tuning = mouth_tuning
    description = "Map the AGC-adjusted microphone level to mouth movement. These controls do not alter AGC policy or audio input."
})
T.position(-1.30, 1.72, 1.35) {
    name = "agc_desktop_settings_anchor"
    settings_panel
}

let agc_level_graph = make_level_history_graph(raw_voice_level, voice_level, {
    sample_period_sec = 0.100
    max_samples = 12.0
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
    panel_x = 2.10
    panel_y = 1.72
    panel_z = 1.35
})
agc_level_graph

// Keep the desktop workspace deliberately narrow: only the editor Settings
// panel is available while tuning AGC, never the broader pose/scene panels.
T.position(1.25, 2.8, -1.5) {
    name = "agc_desktop_editor_ui"
    EditorUI { panels([{ panel = "settings" }]) }
}
