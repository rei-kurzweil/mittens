import { vroid_arm_ik } from "../assets/components/arm_ik/vroid.mms"

let arm_ik = vroid_arm_ik(-0.35, 1)

// XR-only button-driven linear Velocity test. Forward/back change the outer
// grounding root's world velocity using the active XR eye's horizontal heading.
// No gravity or floor contact is present yet.
// Run: cargo run --release -- load examples/mittens-corp-linear-velocity.mms

import { star_kawaii_background } from "../assets/components/backgrounds/star_kawaii_background.mms"
import { tripod_light } from "../assets/components/tripod_light.mms"
import { truss } from "../assets/components/truss.mms"
import { suspended_platform } from "../assets/components/platforms/suspended_platform.mms"
import { bisket_anime_shading } from "../assets/components/materials/bisket_anime_shading.mms"
import { bisket_shirt_physics } from "../assets/components/secondary_motion/bisket-shirt-physics.mms"
import { bisket_colliders } from "../assets/components/colliders/bisket.mms"
import { bisket_humanoid_bone_map } from "../assets/components/humanoid_bone_maps/bisket.mms"
import { ambient_eye_saccades } from "../assets/components/animations/ambient_eye_saccades.mms"
import { info_panel, info_panel_body } from "../assets/components/ui/info_panel.mms"
import { mouth_response_panel } from "../assets/components/ui/mouth_response_panel.mms"
import { button } from "../assets/components/button.mms"

BGC.rgba(0.0, 0.0, 0.0, 1.0)
AL.rgb(0.13, 0.13, 0.15)
Clock.bpm(120.0)
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

BG.occlusion_and_lighting() {
    star_kawaii_background([1.0, 0.72, 0.15, 1.0])
}
stage_box("studio_floor", [0.0, -0.92, 1.0], [54.0, 0.14, 32.0], [0.035, 0.037, 0.043])
stage_box("stage_deck", [0.0, 0.0, -1.5], [32.0, 0.24, 14.0], [0.18, 0.18, 0.20])
stage_box("stage_upper_step", [0.0, -0.24, 5.7], [32.0, 0.28, 0.8], [0.14, 0.14, 0.16])
stage_box("stage_lower_step", [0.0, -0.56, 6.3], [32.0, 0.36, 0.8], [0.10, 0.10, 0.12])
stage_box("stage_back_wall", [0.0, 4.0, -8.35], [32.0, 8.0, 0.35], [0.105, 0.105, 0.12])

T.position(0.0, 2.55, 8.10).scale(1.5, 1.5, 0.08).rotation(0.0, 3.1416, 0.0) {
    name = "stage_mirror"
    Grabbable {}
    R.cube() { Mirror.quality(1440) {} Raycastable.enabled() }
}
T.position(0.0, 7.10, -7.75) { name = "stage_ceiling_truss" truss(26) }
for platform_index in range(3) {
    T.position(11.5, 4.0, (platform_index - 1) * 15.0) { suspended_platform() }
}
let subject_light_target = [0.0, 1.75, 1.7]
tripod_light("front_left_studio_light", [-10.5, 0.14, 4.4], subject_light_target,
    SL.color(1.0, 0.82, 0.70).intensity(10.0).distance(22.0).angle(0.58).penumbra(0.32))
tripod_light("front_right_studio_light", [10.5, 0.14, 4.4], subject_light_target,
    SL.color(0.72, 0.84, 1.0).intensity(10.0).distance(22.0).angle(0.58).penumbra(0.32))
tripod_light("rear_left_studio_light", [-10.5, 0.14, -5.7], subject_light_target,
    SL.color(0.72, 0.84, 1.0).intensity(8.0).distance(20.0).angle(0.62).penumbra(0.38))
tripod_light("rear_right_studio_light", [10.5, 0.14, -5.7], subject_light_target,
    SL.color(1.0, 0.78, 0.68).intensity(8.0).distance(20.0).angle(0.62).penumbra(0.38))

let microphone = AudioInput {}
let voice_level = Amplitude.rolling_window(0.080).highpass(120.0).highpass_resonance(0.707).from(microphone) {}
let voice_panel_target = { avatar = null }
let voice_tuning = { center_rms = 0.0475 range_rms = 0.085 amount = 1.0 }
let xr_input = InputXR.on() {
    InputXRGamepad { locomotion() speed(1.5) }
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
            PoseCapture { label("Bisket XR source") asset_name("bisket") }
            bisket_colliders()
            bisket_shirt_physics(false)
        }
        let bisket_avatar_control = AVC {
            mouth_open_from_amplitude(voice_level)
            mouth_open_rms_center_range(voice_tuning.center_rms, voice_tuning.range_rms)
            mouth_open_amount(voice_tuning.amount)
            mouth_open_smoothing(16.0)
            voice_level
            initial_yaw(3.14159)
            left_two_bone_ik(arm_ik.left)
            right_two_bone_ik(arm_ik.right)
            hand_rotation_smoothing(220.0)
            T { bisket_avatar }
            T.position(0.0, 0.08, 0.12) {
                name = "bisket_xr_camera_anchor"
                CXR { Pointer {} }
            }
            HTCEyeTracking.on().enable_pupil_direction_tracking(false)
            XRHand.new(true, "Left", "GripAim").laser() {
                T { RestAttachment.new("[name='J_Bip_L_Hand']", "[name='J_Bip_L_Middle3']") { Pointer {} } }
            }
            XRHand.new(true, "Right", "GripAim").laser() {
                T { RestAttachment.new("[name='J_Bip_R_Hand']", "[name='J_Bip_R_Middle3']") { Pointer {} } }
            }
        }
        voice_panel_target.avatar = bisket_avatar_control
        bisket_avatar_control
        on(bisket_avatar, "GLTFInitialized", fn(event) {
            let left_eye = event.gltf.query("[name='J_Adj_L_FaceEye']")
            let right_eye = event.gltf.query("[name='J_Adj_R_FaceEye']")
            if left_eye && right_eye {
                bisket_avatar_control.attach(ambient_eye_saccades(left_eye, right_eye, 2.0))
            }
        })
    }
}

let vel = Velocity.rotation_basis(xr_input).horizontal() {
    name = "bisket_velocity"
    T {
        name = "bisket_grounding_root"
        T.position(-5.0, 0.0, 0.0) {
            name = "bisket_locomotion_root"
            xr_input
        }
    }
}
ED.active() {
    T {
        name = "bisket_grounding_frame"
        vel
    }
}

// Keep editor overhead out of this XR velocity smoke test. The authored
// forward/back info panel below remains visible independently of EditorUI.
T.position(1.25, 2.8, -1.5) {
    name = "linear_velocity_editor_ui"
    EditorUI {
        panels([{ panel = "settings" }])
    }
}

// Soft pearlescent palette: warm beige chrome, mint body, lilac toggle,
// lemon forward and blush-magenta back.
let velocity_body_color = [0.79, 0.92, 0.82, 0.98]
let velocity_body_text_color = [0.20, 0.30, 0.26, 1.0]

let voice_panel = mouth_response_panel({
    root_name = "linear_velocity_voice_response_panel"
    title = "Voice response"
    avatar_slot = voice_panel_target
    tuning = voice_tuning
    filter_slot = { amplitude = voice_level cutoff_hz = 120.0 resonance = 0.707 }
    description = "High-pass microphone analysis and mouth response. Audio output is unchanged."
})
T.position(1.4, 2.2, 1.4) {
    name = "linear_velocity_voice_panel_anchor"
    voice_panel
}

fn make_velocity_buttons(velocity_label) {
    let forward = button("forward: +0.25 m/s", {
        background_color = [1.0, 0.96, 0.70, 1.0]
        color = [0.29, 0.26, 0.19, 1.0]
        compact = true
    })
    let back = button("back: -0.25 m/s", {
        background_color = [0.98, 0.81, 0.93, 1.0]
        color = [0.34, 0.22, 0.32, 1.0]
        compact = true
    })
    on(forward, "Click", fn(event) {
        query("#bisket_velocity").translate([0.0, 0.0, -0.25])
    })
    on(back, "Click", fn(event) {
        query("#bisket_velocity").translate([0.0, 0.0, 0.25])
    })
    return T {
        Style { display("flex") flex_direction("column") row_gap(0.4) }
        forward
        back
        T {
            Style { font_size(0.65) color(velocity_body_text_color) }
            Text { name = "velocity_xyz_readout" velocity_label }
        }
    }
}

let panel = info_panel({
    root_name = "linear_velocity_panel"
    width_gu = 25.0
    unit_scale = 0.08
    title = "XR linear velocity"
    title_background_color = [0.94, 0.87, 0.79, 0.98]
    title_text_color = [0.28, 0.23, 0.27, 1.0]
    toggle_background_color = [0.85, 0.82, 0.95, 1.0]
    toggle_icon_color = [0.40, 0.32, 0.48, 1.0]
    toggle_icon_glow_intensity = 0.25
    body_background_color = velocity_body_color
    body_text_color = velocity_body_text_color
    content = make_velocity_buttons("velocity x/y/z: 0, 0, 0 m/s")
})
Selectable.off() {
    T.position(-2.5, 2.2, 1.4) {
        name = "linear_velocity_panel_anchor"
        Grabbable {}
        panel
    }
}
let body_mount = panel.query("#accordion_body_mount")
on(panel, "DataEvent", fn(event) {
    if event == "AccordionRestoreRequested" {
        let current = query("#bisket_velocity").linear()
        let label = "velocity x/y/z: " + current[0] + ", " + current[1] + ", " + current[2] + " m/s"
        body_mount.attach(info_panel_body({
            content = make_velocity_buttons(label)
            body_background_color = velocity_body_color
            body_text_color = velocity_body_text_color
        }))
    }
})

on(vel, "DataEvent", fn(event) {
    if event == "VelocityChanged" {
        let current = query("#bisket_velocity").linear()
        let readout = query("#velocity_xyz_readout")
        if readout {
            readout.set_text("velocity x/y/z: " + current[0] + ", " + current[1] + ", " + current[2] + " m/s")
        }
    }
})

XR.on()
