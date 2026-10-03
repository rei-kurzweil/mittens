import { vroid_arm_ik } from "../assets/components/arm_ik/vroid.mms"

let arm_ik = vroid_arm_ik(-0.35, -1)

// VTuber microphone-speaking animation + switchable XR eye-tracking mirror
//
// A focused XR acceptance scene for automatic eye-bone tracking and the
// amplitude-driven mouth-open fallback. This first panel slice deliberately
// uses a static body; the next slice reloads AudioInput.devices() rows when
// the panel emits AccordionRestoreRequested.

import { star_kawaii_background } from "../assets/components/backgrounds/star_kawaii_background.mms"
import { tripod_light } from "../assets/components/tripod_light.mms"
import { bisket_colliders } from "../assets/components/colliders/bisket.mms"
import { bisket_shirt_physics } from "../assets/components/secondary_motion/bisket-shirt-physics.mms"
import { bisket_anime_shading } from "../assets/components/materials/bisket_anime_shading.mms"
import { info_panel } from "../assets/components/ui/info_panel.mms"

RendererSettings { window_size(960, 720) }
BGC.rgba(0.0, 0.0, 0.0, 1.0)
AL.rgb(0.18, 0.18, 0.22)

RenderGraph {
    EmissivePass { BlurPass { radius_ndc(0.06) half_res(true) } }
    Bloom { intensity(0.8) emissive_scale(1.1) }
}

BG.occlusion_and_lighting() {
    star_kawaii_background([1.0, 0.72, 0.15, 1.0])
}

// Three physical studio fixtures make the mirror's lighting easy to inspect.
tripod_light("eye_tracking_key", [-4.2, 0.0, 2.8], [0.0, 1.25, -1.5], SL.color(1.0, 0.78, 0.62).intensity(6.0).distance(11.0).angle(0.62).penumbra(0.35))
tripod_light("eye_tracking_fill", [4.0, 0.0, 1.4], [0.0, 1.25, -1.5], SL.color(0.48, 0.68, 1.0).intensity(4.5).distance(11.0).angle(0.62).penumbra(0.35))
tripod_light("eye_tracking_rim", [1.8, 0.0, -4.2], [0.0, 1.25, -1.5], SL.color(1.0, 0.42, 0.78).intensity(5.0).distance(11.0).angle(0.62).penumbra(0.35))

// Start on the known-good host default. Clicking a row below switches this
// live source to that row's session-local AudioInput.devices() index.
let microphone = AudioInput {}
let voice_level = Amplitude.rolling_window(0.080).from(microphone) {}
let active_eye_tracking = VRChatOSCEyeTracking.on().rotation_limits(0.35, 0.35, 0.25, 0.25)
let eye_tracking_state = { tracker = active_eye_tracking }

let audio_input_devices = AudioInput.devices()
let audio_input_status = Text { "default audio input selected — click a device to switch" }

fn audio_input_option(input_source_name, input_source_index, microphone, status_text) {
    let label = "[" + input_source_index + "] " + input_source_name
    let row = T {
        name = "audio_input_option_" + input_source_index
        Option {}
        Raycastable.enabled()
        Style {
            display("block")
            width(100%)
            margin_xy(0.1, 0.12)
            padding_xy(0.45, 0.35)
            background_color([0.15, 0.15, 0.17, 0.98])
            background_z(-0.01)
            color([0.96, 0.96, 0.98, 1.0])
        }
        T.position(0.0, 0.0, 0.02) { Text { label } }
    }

    // This first slice uses direct row handlers; a generic MMS
    // SelectionChanged payload API can be introduced separately.
    on(row, "Click", fn(event) {
        microphone.select_device_number(input_source_index)
        status_text.set_text("selected audio input " + label)
    })
    return row
}

fn audio_input_options(devices, microphone, status_text) {
    let next_index = 0.0
    return T {
        name = "audio_input_selection_content"
        Selection { name = "audio_input_selection" }
        Style {
            display("flex")
            flex_direction("column")
            width(100%)
        }
        for input_source_name in devices {
            let input_source_index = next_index
            next_index = next_index + 1.0
            audio_input_option(input_source_name, input_source_index, microphone, status_text)
        }
    }
}

let microphone_info_panel = info_panel({
    root_name = "microphone_inputs_panel"
    width_gu = 30.0
    unit_scale = 0.075
    title = "audio input devices"
    content = T {
        name = "microphone_inputs_content"
        Style {
            display("flex")
            flex_direction("column")
            width(100%)
        }
        T.position(0.0, 0.0, 0.02) {
            Style {
                display("block")
                width(100%)
                padding_xy(0.35, 0.25)
                color([0.96, 0.96, 0.98, 1.0])
            }
            audio_input_status
        }
        audio_input_options(audio_input_devices, microphone, audio_input_status)
    }
})

let avatar_control = AVC {
        name = "avatar_control"
        mouth_open_from_amplitude(voice_level)
        mouth_open_rms_floor(0.005)
        mouth_open_rms_ceiling(0.09)
        mouth_open_smoothing(16.0)

        // Direct placement also makes the retained measurement available to
        // AVC diagnostics. Capture is not monitored.
        voice_level

        // OpenXR's rest-forward is -Z.
        initial_yaw(3.14159)
        left_two_bone_ik(arm_ik.left)
        right_two_bone_ik(arm_ik.right)
        hand_rotation_smoothing(220.0)

        T {
            GLTF.new("assets/models/bisket.glb") {
                bisket_anime_shading()
                MorphTargetMap.new()
                    .slot("left_eye_blink", "Fcl_EYE_Close_L")
                    .slot("right_eye_blink", "Fcl_EYE_Close_R")
                    .slot("viseme_aa", "Fcl_MTH_A")
                    .slot("viseme_ih", "Fcl_MTH_I")
                    .slot("viseme_ou", "Fcl_MTH_U")
                    .slot("viseme_e", "Fcl_MTH_E")
                    .slot("viseme_oh", "Fcl_MTH_O")
                EM.on()
                bisket_colliders()
                bisket_shirt_physics(false)
            }
        }

        // AVC reparents this camera path to the mapped head.
        T.position(0.0, 0.08, 0.12) {
            CXR { Pointer {} }
        }

        // The active transport is always a direct AVC child.
        eye_tracking_state.tracker

        // Direct children supply the left/right hand targets used by AVC's
        // mapped TwoBoneIK chains.
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

// An explicit list keeps the editor useful without materializing the default
// world and assets panels in this focused capture scene.
T.position(-2.75, 2.8, -1.5) {
    EditorUI {
        panels([{
            panel = "settings"
            config = {
                show_armature = false
                show_bounds = false
                show_cameras = false
                show_colliders = false
                show_gltf_colliders = false
                show_spring_bones = false
            }
        }])
    }
}

let scene_root = ED {
    // Low presentation stage, with colored block piles marking its edges.
    T.position(0.0, -0.85, -1.5).scale(9.8, 0.16, 8.4) {
        R.cube() { C.rgba(0.20, 0.22, 0.27, 1.0) }
    }

    // Left pile.
    T.position(-7.70, -0.56, -0.7).scale(0.34, 0.34, 0.34) { Grabbable {} R.cube() { C.rgba(1.0, 0.38, 0.56, 1.0) EM.on() } }
    T.position(-7.70, -0.22, -0.7).scale(0.34, 0.34, 0.34) { Grabbable {} R.cube() { C.rgba(1.0, 0.72, 0.20, 1.0) EM.on() } }
    T.position(-7.35, -0.56, -0.7).scale(0.34, 0.34, 0.34) { Grabbable {} R.cube() { C.rgba(0.40, 0.76, 1.0, 1.0) EM.on() } }

    // Right pile.
    T.position(7.70, -0.56, -0.7).scale(0.34, 0.34, 0.34) { Grabbable {} R.cube() { C.rgba(0.44, 0.92, 0.66, 1.0) EM.on() } }
    T.position(7.70, -0.22, -0.7).scale(0.34, 0.34, 0.34) { Grabbable {} R.cube() { C.rgba(0.54, 0.48, 1.0, 1.0) EM.on() } }
    T.position(7.35, -0.56, -0.7).scale(0.34, 0.34, 0.34) { Grabbable {} R.cube() { C.rgba(1.0, 0.48, 0.82, 1.0) EM.on() } }

    // Rear pile.
    T.position(0.0, -0.56, 5.85).scale(0.34, 0.34, 0.34) { Grabbable {} R.cube() { C.rgba(0.25, 0.78, 1.0, 1.0) EM.on() } }
    T.position(0.0, -0.22, 5.85).scale(0.34, 0.34, 0.34) { Grabbable {} R.cube() { C.rgba(0.98, 0.90, 0.30, 1.0) EM.on() } }
    T.position(-0.35, -0.56, 5.85).scale(0.34, 0.34, 0.34) { Grabbable {} R.cube() { C.rgba(1.0, 0.48, 0.42, 1.0) EM.on() } }

    // Deliberate versions of the friendly fallback-scene accents: a color cat
    // and a rainbow row of planes behind the stage.
    T.position(0.0, -0.10, -4.0).scale(0.50, 0.50, 0.50) {
        Grabbable {}
        GLTF.new("assets/models/color-cat.2.glb") {}
    }
    T.position(-1.40, 1.95, 1.75).scale(0.28, 0.28, 1.0) { R.plane() { C.rgba(1.0, 0.20, 0.20, 1.0) EM.on() } }
    T.position(-0.84, 1.95, 1.75).scale(0.28, 0.28, 1.0) { R.plane() { C.rgba(1.0, 0.60, 0.20, 1.0) EM.on() } }
    T.position(-0.28, 1.95, 1.75).scale(0.28, 0.28, 1.0) { R.plane() { C.rgba(1.0, 1.0, 0.20, 1.0) EM.on() } }
    T.position(0.28, 1.95, 1.75).scale(0.28, 0.28, 1.0) { R.plane() { C.rgba(0.20, 0.80, 0.35, 1.0) EM.on() } }
    T.position(0.84, 1.95, 1.75).scale(0.28, 0.28, 1.0) { R.plane() { C.rgba(0.20, 0.60, 1.0, 1.0) EM.on() } }
    T.position(1.40, 1.95, 1.75).scale(0.28, 0.28, 1.0) { R.plane() { C.rgba(0.80, 0.20, 1.0, 1.0) EM.on() } }

    // Full-body mirror facing the avatar.
    T.position(0.0, 1.25, -4.5).scale(2.4, 2.4, 0.08) {
        R.cube() { Mirror.quality(2048) {} }
    }

    T {
        InputXR.on() {
            InputXRGamepad { locomotion() speed(1.5) }
            T { avatar_control }
        }
    }
}

// Emit the scene before creating the panel.
scene_root

let eye_tracking_status = Text { "selected VRChat OSC" }

let vrchat_eye_tracking_option = T {
    name = "eye_tracking_option_vrchat_osc"
    Option {}
    Raycastable.enabled()
    Style {
        display("block")
        width(100%)
        margin_xy(0.1, 0.12)
        padding_xy(0.45, 0.35)
        background_color([0.15, 0.15, 0.17, 0.98])
        background_z(-0.01)
        color([0.96, 0.96, 0.98, 1.0])
    }
    T.position(0.0, 0.0, 0.02) { Text { "VRChat OSC" } }
}

let htc_eye_tracking_option = T {
    name = "eye_tracking_option_htc_wave"
    Option {}
    Raycastable.enabled()
    Style {
        display("block")
        width(100%)
        margin_xy(0.1, 0.12)
        padding_xy(0.45, 0.35)
        background_color([0.15, 0.15, 0.17, 0.98])
        background_z(-0.01)
        color([0.96, 0.96, 0.98, 1.0])
    }
    T.position(0.0, 0.0, 0.02) { Text { "HTC Wave Sdk\n(experimental)" } }
}

on(vrchat_eye_tracking_option, "Click", fn(event) {
    let current_tracker = eye_tracking_state.tracker
    current_tracker.remove_subtree()
    let replacement = VRChatOSCEyeTracking.on().rotation_limits(0.35, 0.35, 0.25, 0.25)
    avatar_control.attach(replacement)
    eye_tracking_state.tracker = replacement
    eye_tracking_status.set_text("selected VRChat OSC")
})

on(htc_eye_tracking_option, "Click", fn(event) {
    let current_tracker = eye_tracking_state.tracker
    current_tracker.remove_subtree()
    let replacement = HTCEyeTracking.on().rotation_limits(0.35, 0.35, 0.25, 0.25)
    avatar_control.attach(replacement)
    eye_tracking_state.tracker = replacement
    eye_tracking_status.set_text("selected HTC Wave Sdk (experimental)")
})

let eye_tracking_options = T {
    name = "eye_tracking_selection_content"
    Selection { name = "eye_tracking_selection" }
    Style {
        display("flex")
        flex_direction("column")
        width(100%)
    }
    vrchat_eye_tracking_option
    htc_eye_tracking_option
}

let eye_tracking_info_panel = info_panel({
    root_name = "eye_tracking_panel"
    width_gu = 24.0
    unit_scale = 0.075
    title = "eye tracking"
    content = T {
        name = "eye_tracking_content"
        Style {
            display("flex")
            flex_direction("column")
            width(100%)
        }
        T.position(0.0, 0.0, 0.02) {
            Style {
                display("block")
                width(100%)
                padding_xy(0.35, 0.25)
                color([1.0, 0.84, 0.0, 1.0])
            }
            eye_tracking_status
        }
        eye_tracking_options
    }
})

// Ordinary scene UI. The selection system supplies its standard yellow
// highlight to the clicked Option rows; title bars remain draggable and both
// bodies can be collapsed independently in XR.
T.position(-5.85, 2.25, -3.8) {
    eye_tracking_info_panel
}
T.position(-3.6, 2.25, -3.8) {
    microphone_info_panel
}

XR.on()
