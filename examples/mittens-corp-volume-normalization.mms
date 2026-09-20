// mittens-corp-volume-normalization — XR car/controller regression scene and Bisket pose-authoring tool.
//
// Run with:
//   cargo run --release -- load examples/mittens-corp-volume-normalization.mms
//
// The vehicle uses the generic inverse-local anchor operator rather than AVC:
// the rigid car has no humanoid specialization for AVC to perform.

import { tripod_light } from "../assets/components/tripod_light.mms"
import { truss } from "../assets/components/truss.mms"
import { bisket_anime_shading } from "../assets/components/materials/bisket_anime_shading.mms"
import { bisket_shirt_physics } from "../assets/components/secondary_motion/bisket-shirt-physics.mms"
import { bisket_colliders } from "../assets/components/colliders/bisket.mms"
import { bisket_humanoid_bone_map } from "../assets/components/humanoid_bone_maps/bisket.mms"
import { ambient_eye_saccades } from "../assets/components/animations/ambient_eye_saccades.mms"
import { suspended_platform } from "../assets/components/platforms/suspended_platform.mms"
import { display_car_xr } from "../assets/components/vehicles/display_car.mms"
import { star_kawaii_background } from "../assets/components/backgrounds/star_kawaii_background.mms"
import { info_panel, info_panel_body } from "../assets/components/ui/info_panel.mms"

// Optional sources stay neutral when the runtime or hardware is unavailable.
let microphone = AudioInput {}
let raw_voice_level = Amplitude.rolling_window(0.080).from(microphone) {}

// Keep the raw observer alive for diagnostics; AVC consumes only this adaptive
// analysis view. It changes no audible microphone samples.
let voice_level = VolumeNormalization.from(raw_voice_level) {}
let agc_mode = { enabled = true }

// The response panel samples retained main-thread diagnostics rather than
// audio callback data. The graph is rebuilt from twelve scalar snapshots at
// 10 Hz, so the columns are always left-to-right chronological: oldest on the
// left, newest on the right. It continues to show the normalizer's decision
// even while B routes AVC to the raw meter.
fn make_gain_history_bar(gain_db, index, visible, config) {
    let magnitude = Math.abs(gain_db)
    if magnitude > config.db_extent {
        magnitude = config.db_extent
    }
    let bar_height = magnitude * config.units_per_db
    if bar_height < config.min_bar_height {
        bar_height = config.min_bar_height
    }
    let zero_y = -config.chart_height / 2.0
    let is_boost = gain_db > 0.0
    let is_cut = gain_db < 0.0
    let y = zero_y
    let colour = [0.95, 0.62, 0.16, 0.0]
    if is_boost {
        y = zero_y + bar_height / 2.0
        colour = [0.20, 1.00, 0.48, 1.0]
    } else if is_cut {
        y = zero_y - bar_height / 2.0
        colour = [1.00, 0.30, 0.18, 1.0]
    } else if visible {
        colour = [0.95, 0.62, 0.16, 1.0]
    }

    // Layout dimensions above are glyph/layout units, while authored child
    // transforms are world units. The LayoutRoot does not scale these manual
    // cube transforms for us, so convert both position and size explicitly.
    let x_gu = config.column_width / 2.0 + index * (config.column_width + config.column_gap)
    return T.position(x_gu * config.unit_scale, y * config.unit_scale, 0.03).scale(
        config.column_width * config.unit_scale,
        bar_height * config.unit_scale,
        config.column_depth * config.unit_scale,
    ) {
        R.cube() { C.rgba(colour[0], colour[1], colour[2], colour[3]) }
    }
}

fn make_gain_history_view(history, config) {
    let first_visible = config.max_samples - history.sample_count
    return T {
        name = "agc_gain_history_view"
        make_gain_history_bar(history.s0, 0.0, 0.0 >= first_visible, config)
        make_gain_history_bar(history.s1, 1.0, 1.0 >= first_visible, config)
        make_gain_history_bar(history.s2, 2.0, 2.0 >= first_visible, config)
        make_gain_history_bar(history.s3, 3.0, 3.0 >= first_visible, config)
        make_gain_history_bar(history.s4, 4.0, 4.0 >= first_visible, config)
        make_gain_history_bar(history.s5, 5.0, 5.0 >= first_visible, config)
        make_gain_history_bar(history.s6, 6.0, 6.0 >= first_visible, config)
        make_gain_history_bar(history.s7, 7.0, 7.0 >= first_visible, config)
        make_gain_history_bar(history.s8, 8.0, 8.0 >= first_visible, config)
        make_gain_history_bar(history.s9, 9.0, 9.0 >= first_visible, config)
        make_gain_history_bar(history.s10, 10.0, 10.0 >= first_visible, config)
        make_gain_history_bar(history.s11, 11.0, 11.0 >= first_visible, config)
    }
}

fn make_gain_history_content(history, config) {
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
            "current applied gain: +0.0 dB"
        }
        T {
            Style {
                display("block")
                width(100%)
                color([0.75, 0.80, 0.90, 1.0])
            }
            Text { "12 samples / 1.2 s  ·  boost green  ·  cut red  ·  zero amber" }
        }
        T {
            name = "agc_gain_history_plot"
            Style {
                display("block")
                width(config.plot_width)
                height(config.chart_height)
                margin_top(0.20)
                background_color([0.025, 0.030, 0.040, 0.96])
                background_z(-0.01)
            }
            // All graph children use explicit local coordinates. They are not
            // inline or inline-block layout items, so layout cannot move a
            // historical column outside the plot.
            T.position(
                config.plot_width / 2.0 * config.unit_scale,
                -config.chart_height / 2.0 * config.unit_scale,
                0.01,
            ).scale(
                config.plot_width * config.unit_scale,
                0.05 * config.unit_scale,
                0.04 * config.unit_scale,
            ) {
                R.cube() { C.rgba(0.92, 0.70, 0.25, 0.72) }
            }
            T {
                name = "agc_gain_history_layers"
                make_gain_history_view(history, config)
            }
        }
        T {
            Style {
                display("block")
                width(100%)
                color([0.70, 0.74, 0.82, 1.0])
            }
            Text { "oldest ←                         → newest" }
        }
    }
}

fn gain_db_label(gain_db) {
    let rounded = Math.round(gain_db * 10.0) / 10.0
    let sign = ""
    if rounded >= 0.0 { sign = "+" }
    return sign + rounded + " dB"
}

fn make_gain_history_graph(level, config) {
    let history = {
        elapsed_sec = 0.0
        sample_count = 0.0
        s0 = 0.0 s1 = 0.0 s2 = 0.0 s3 = 0.0 s4 = 0.0 s5 = 0.0
        s6 = 0.0 s7 = 0.0 s8 = 0.0 s9 = 0.0 s10 = 0.0 s11 = 0.0
    }
    let initial_content = make_gain_history_content(history, config)
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
            let gain_db = level.gain_db()
            history.s0 = history.s1
            history.s1 = history.s2
            history.s2 = history.s3
            history.s3 = history.s4
            history.s4 = history.s5
            history.s5 = history.s6
            history.s6 = history.s7
            history.s7 = history.s8
            history.s8 = history.s9
            history.s9 = history.s10
            history.s10 = history.s11
            history.s11 = gain_db
            if history.sample_count < config.max_samples {
                history.sample_count = history.sample_count + 1.0
            }
            let history_layers = graph.query("#agc_gain_history_layers")
            if history_layers {
                // Replace one contained view rather than adding individual
                // layout-flow children. This preserves a true, fixed-width
                // twelve-snapshot history even after it fills.
                history_layers.remove_child(0)
                history_layers.attach(make_gain_history_view(history, config))
            }
            let current_gain_text = graph.query("#agc_current_gain_text")
            if current_gain_text {
                current_gain_text.set_text("current applied gain: " + gain_db_label(gain_db))
            }
        }
    })

    on(graph, "DataEvent", fn(event) {
        if event == "AccordionRestoreRequested" {
            // The info-panel asset intentionally removes its body while
            // minimized. Recreate this dynamic body and resume sampling on
            // its next normal 100 ms update.
            body_mount.attach(info_panel_body({
                content = make_gain_history_content(history, config)
            }))
        }
    })

    return graph
}

let agc_gain_graph = make_gain_history_graph(voice_level, {
    sample_period_sec = 0.100
    max_samples = 12.0
    db_extent = 24.0
    column_width = 2.05
    column_gap = 0.34
    column_depth = 0.22
    // ±24 dB must fit within the 10-unit plot height around its zero line.
    units_per_db = 0.20
    min_bar_height = 0.12
    chart_height = 10.0
    plot_width = 28.7
    panel_width = 32.0
    unit_scale = 0.08
    panel_x = -1.30
    panel_y = 2.05
    panel_z = 1.35
})
agc_gain_graph

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

fn stage_box(box_name, position, size, color) {
    return T.position(position[0], position[1], position[2])
        .scale(size[0], size[1], size[2]) {
        name = box_name
        R.cube() { C.rgba(color[0], color[1], color[2], 1.0) }
    }
}

// Keep the car example's studio stage so this tool also retains a stable
// lighting and mirror reference while XR transforms and gizmos are exercised.
stage_box(
    "studio_floor",
    [0.0, -0.92, 1.0],
    [54.0, 0.14, 32.0],
    [0.035, 0.037, 0.043],
)

stage_box("stage_deck",       [0.0,  0.00, -1.5], [32.0, 0.24, 14.0], [0.18, 0.18, 0.20])
stage_box("stage_upper_step", [0.0, -0.24,  5.7], [32.0, 0.28,  0.8], [0.14, 0.14, 0.16])
stage_box("stage_lower_step", [0.0, -0.56,  6.3], [32.0, 0.36,  0.8], [0.10, 0.10, 0.12])
stage_box("stage_back_wall",  [0.0,  4.00, -8.35], [32.0, 8.00, 0.35], [0.105, 0.105, 0.12])

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

// Bisket is the player rig. InputXR continues to own tracked head translation
// and rotation; only the gamepad's built-in locomotion mapping is handed off
// when a vehicle layer takes movement authority.
ED.active() {
    let vehicle_controls = InputXRGamepad {
        name = "bisket_pedestrian_locomotion"
        locomotion()
        speed(1.5)
    }

    T.position(-5.0, 0.0, 0.0) {
        name = "bisket_locomotion_root"
        Rider
            .anchor("[name='bisket_rider_cxr_anchor']")
            .movement_root("[name='bisket_locomotion_root']")
            .input("[name='bisket_pedestrian_locomotion']") {}
        InputXR.on() {
            vehicle_controls

            T {
                name = "bisket_xr_driver"
                let bisket_avatar = GLTF.new("assets/models/bisket.glb") {
                    bisket_anime_shading()
                    bisket_humanoid_bone_map()
                    MorphTargetMap.new()
                        .slot("left_eye_blink", "Fcl_EYE_Close_L")
                        .slot("right_eye_blink", "Fcl_EYE_Close_R")
                        .slot("viseme_aa", "Fcl_MTH_A")
                    EM.on()
                    PoseCapture { label("Bisket") asset_name("bisket") }
                    bisket_colliders()
                    bisket_shirt_physics(false)
                }
                let bisket_avatar_control = AVC {
                    mouth_open_from_amplitude(voice_level)
                    mouth_open_rms_floor(0.005)
                    mouth_open_rms_ceiling(0.09)
                    mouth_open_smoothing(16.0)
                    voice_level
                    
                    initial_yaw(3.14159)
                    left_arm_pole_direction([1, -0.35, 1])
                    right_arm_pole_direction([-1, -0.35, 1])
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
                    HTCEyeTracking.on().enable_pupil_direction_tracking(false)

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

                // The explicit Bisket humanoid map above declares these two
                // skin-joint targets. Query only this GLTF instance after it
                // finishes importing, so another avatar cannot be animated.
                on(bisket_avatar, "GLTFInitialized", fn(event) {
                    let left_eye = event.gltf.query("[name='J_Adj_L_FaceEye']")
                    let right_eye = event.gltf.query("[name='J_Adj_R_FaceEye']")
                    if left_eye && right_eye {
                        bisket_avatar_control.attach(ambient_eye_saccades(left_eye, right_eye, 2.0))
                    } else {
                        print("GLTFInitialized: Bisket mapped eye bones were not found; ambient eye animation was not attached")
                    }
                })
            }
        }
    }

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
