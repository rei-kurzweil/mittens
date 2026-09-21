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
import { info_panel, info_panel_body } from "../assets/components/ui/info_panel.mms"

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

let PANEL_WIDTH = 40.0
let ROW_HEIGHT = 3.0
let LABEL_WIDTH = 11.0
let SLIDER_SLOT_WIDTH = 18.0
let READOUT_WIDTH = 4.0
let CONTROL_FONT_SIZE = 0.8

fn mouth_response_slider(slider_name, initial, minimum, maximum, step) {
    // Match the compact, explicit slider visuals used by shading-models: the
    // Slider's world-space width remains four units even though its layout slot
    // is wider, so no track can escape the info panel horizontally.
    let track = T.scale(2.0, 0.025, 0.10) {
        R.cube() {
            C.rgba(0.24, 0.45, 0.65, 1.0)
            Raycastable.enabled() { interaction_priority(120.0) }
        }
    }
    let thumb = T.scale(0.1125, 0.225, 0.30) {
        R.sphere() {
            C.rgba(1.0, 0.46, 0.64, 1.0)
            Raycastable.enabled() { interaction_priority(120.0) }
        }
    }
    return Slider.range(minimum, maximum).step(step).value(initial).width(4.0)
        .track(track)
        .thumb(thumb) { name = slider_name }
}

fn mouth_response_row(text, slider, readout) {
    return T {
        Style {
            display("flex") flex_direction("row") width(100%) height(ROW_HEIGHT)
            align_items("center") gap(0.5)
            background_color([0.055, 0.075, 0.10, 0.94]) background_z(-0.02)
        }
        T {
            Style {
                display("flex") width(LABEL_WIDTH) height(ROW_HEIGHT)
                align_items("center") padding_xy(0.25, 0.0) font_size(CONTROL_FONT_SIZE)
            }
            T.position(0.0, 0.0, 0.03) { Text { text } }
        }
        T {
            Style {
                display("flex") width(SLIDER_SLOT_WIDTH) height(ROW_HEIGHT)
                flex_grow(1.0) align_items("center") justify_content("center")
            }
            slider
        }
        T {
            Style {
                display("flex") width(READOUT_WIDTH) height(ROW_HEIGHT)
                align_items("center") justify_content("center") font_size(CONTROL_FONT_SIZE)
                color([0.76, 0.92, 1.0, 1.0])
            }
            T.position(0.0, 0.0, 0.03) { readout }
        }
    }
}

// This is a factory because info_panel removes its body while minimized. A
// restored panel gets fresh sliders and handlers while retaining the same AVC
// and its live mouth-response settings.
fn make_mouth_response_content(avatar, mouth_tuning) {
    let center_slider = mouth_response_slider(
        "mouth_rms_center_slider", mouth_tuning.center_rms, 0.001, 0.120, 0.001,
    )
    let center_readout = Text { name = "mouth_rms_center_readout" "0.032 RMS" }
    let range_slider = mouth_response_slider(
        "mouth_rms_range_slider", mouth_tuning.range_rms, 0.001, 0.120, 0.001,
    )
    let range_readout = Text { name = "mouth_rms_range_readout" "0.057 RMS" }
    let amount_slider = mouth_response_slider(
        "mouth_amount_slider", mouth_tuning.amount, 0.0, 1.0, 0.01,
    )
    let amount_readout = Text { name = "mouth_amount_readout" "1.000" }
    let live_readout = Text { name = "agc_live_readout" "waiting for microphone…" }

    on(center_slider, "SliderChanged", fn(event) {
        mouth_tuning.center_rms = event.value
        let half_range = mouth_tuning.range_rms * 0.5
        if half_range > mouth_tuning.center_rms {
            mouth_tuning.range_rms = mouth_tuning.center_rms * 2.0
            range_slider.sync_value(mouth_tuning.range_rms)
            range_readout.set_text(fixed_3(mouth_tuning.range_rms) + " RMS")
        }
        avatar.set_mouth_open_rms_center_range(mouth_tuning.center_rms, mouth_tuning.range_rms)
        center_readout.set_text(fixed_3(mouth_tuning.center_rms) + " RMS")
    })
    on(range_slider, "SliderChanged", fn(event) {
        mouth_tuning.range_rms = event.value
        if mouth_tuning.range_rms * 0.5 > mouth_tuning.center_rms {
            mouth_tuning.range_rms = mouth_tuning.center_rms * 2.0
            range_slider.sync_value(mouth_tuning.range_rms)
        }
        avatar.set_mouth_open_rms_center_range(mouth_tuning.center_rms, mouth_tuning.range_rms)
        range_readout.set_text(fixed_3(mouth_tuning.range_rms) + " RMS")
    })
    on(amount_slider, "SliderChanged", fn(event) {
        mouth_tuning.amount = event.value
        avatar.set_mouth_open_amount(mouth_tuning.amount)
        amount_readout.set_text(fixed_3(mouth_tuning.amount))
    })

    return T {
        name = "mouth_response_settings_content"
        Style { display("flex") flex_direction("column") width(100%) row_gap(0.20) }
        T {
            Style { display("block") width(100%) font_size(0.60) color([0.78, 0.84, 0.92, 1.0]) }
            Text { "Map the AGC-adjusted microphone level to Bisket's mouth. These controls do not alter the AGC policy or audio device." }
        }
        mouth_response_row("RMS centre", center_slider, center_readout)
        mouth_response_row("RMS range", range_slider, range_readout)
        mouth_response_row("mouth amount", amount_slider, amount_readout)
        T {
            Style { display("block") width(100%) margin_top(0.25) font_size(0.60) color([0.88, 0.96, 1.0, 1.0]) }
            live_readout
        }
    }
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

let settings_content = make_mouth_response_content(avatar, mouth_tuning)
let settings_panel = info_panel({
    root_name = "agc_desktop_settings_panel"
    width_gu = PANEL_WIDTH
    unit_scale = 0.08
    title = "Bisket mouth response"
    background_color = [0.10, 0.20, 0.28, 0.98]
    toggle_background_color = [0.16, 0.38, 0.54, 1.0]
    content = settings_content
})

T.position(-1.30, 1.72, 1.35) {
    name = "agc_desktop_settings_anchor"
    settings_panel
}

let settings_body_mount = settings_panel.query("#accordion_body_mount")
on(settings_panel, "DataEvent", fn(event) {
    if event == "AccordionRestoreRequested" {
        settings_body_mount.attach(info_panel_body({
            content = make_mouth_response_content(avatar, mouth_tuning)
        }))
    }
})

let readout_clock = { elapsed = 0.0 }
on_global("FrameTick", fn(event) {
    readout_clock.elapsed = readout_clock.elapsed + event.dt_sec
    if readout_clock.elapsed >= 0.10 {
        readout_clock.elapsed = readout_clock.elapsed - 0.10
        let live_readout = settings_panel.query("#agc_live_readout")
        if live_readout {
            live_readout.set_text(
                "raw " + fixed_3(raw_voice_level.value())
                    + " RMS  ·  normalized " + fixed_3(voice_level.value())
                    + " RMS  ·  gain " + fixed_3(voice_level.gain_db()) + " dB"
            )
        }
    }
})

// Keep the desktop workspace deliberately narrow: only the editor Settings
// panel is available while tuning AGC, never the broader pose/scene panels.
T.position(1.25, 2.8, -1.5) {
    name = "agc_desktop_editor_ui"
    EditorUI { panels([{ panel = "settings" }]) }
}
