import { vroid_arm_ik } from "../assets/components/arm_ik/vroid.mms"

let arm_ik = vroid_arm_ik(-0.35, -1)

// XR VTuber slide-deck prototype.
//
// Derived from vtuber-mirror-example, with a deliberately lean EditorUI.
// The world and asset panels are omitted because their current draw/list work
// materially affects the VR performance this example is intended to preserve.
//
// Manual slide controls:
//   B = next slide
//   A = previous slide
//
// Run:
//   cargo run --release --example vtuber-slidedeck

import { bisket_shirt_physics } from "../assets/components/secondary_motion/bisket-shirt-physics.mms"
import { bisket_colliders } from "../assets/components/colliders/bisket.mms"
import { bisket_anime_shading } from "../assets/components/materials/bisket_anime_shading.mms"

// Default capture is optional: unavailable input leaves AVC's mouth driver neutral.
let microphone = AudioInput {}
let voice_level = Amplitude.rolling_window(0.080).from(microphone) {}

RendererSettings {
    window_size(640, 480)
}

BGC { C.rgba(0.90, 0.93, 0.98, 1.0) }
AL.rgb(0.20, 0.20, 0.24)
Clock.bpm(60) {}

RenderGraph {
    EmissivePass {
        BlurPass {
            radius_ndc(0.06)
            half_res(true)
        }
    }
    Bloom {
        intensity(0.90)
        radius_ndc(0.06)
        emissive_scale(1.2)
        half_res(true)
    }
}

T.position(0.15, -0.45, 1.0) {
    DL { intensity(1.0) color(1.0, 0.98, 0.95) }
}
T.position(-0.85, 0.55, 0.35) {
    DL { intensity(0.75) color(0.90, 0.94, 1.0) }
}
T.position(0.75, 0.35, -0.75) {
    DL { intensity(0.65) color(1.0, 0.82, 0.90) }
}

// One lightweight editor workspace. An explicit panel list prevents EditorUI
// from materializing its default assets and world panels.
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

// Temple floor, frame, and mirror retain the useful workload and visual context
// of vtuber-mirror-example without reproducing every decorative primitive.
ED.active() {
    T.position(0.0, -1.75, 0.0).scale(120.0, 0.12, 120.0) {
        Collision.static() {
            CollisionShape.cube([60.0, 0.06, 60.0])
        }
        R.cube() { C.rgba(0.73, 0.71, 0.70, 1.0) Raycastable.enabled() }
    }

    T.position(0.0, -1.55, -0.2).scale(8.0, 0.18, 10.5) {
        R.cube() { C.rgba(0.86, 0.84, 0.82, 1.0) }
    }

    T.position(0.0, 0.55, -3.95).scale(2.30, 2.60, 0.08) {
        R.cube() {
            C.rgba(0.82, 0.88, 0.94, 1.0)
            Mirror.quality(1024) {}
        }
    }

    T.position(-1.1, -0.95, -1.7).scale(0.45, 0.9, 0.45) {
        R.cube() { C.rgba(0.75, 0.28, 0.26, 1.0) EM.on() Raycastable.enabled() }
    }
    T.position(0.0, -0.75, -1.25).scale(0.40, 1.3, 0.40) {
        R.cube() { C.rgba(0.15, 0.70, 0.98, 1.0) EM.on() Raycastable.enabled() }
    }
    T.position(1.1, -0.95, -1.7).scale(0.45, 0.9, 0.45) {
        R.cube() { C.rgba(1.0, 0.84, 0.18, 1.0) EM.on() Raycastable.enabled() }
    }
}

// The slide content keeps its authored presentation offset beneath a detached
// world-space placement root. Button presses copy the current XR camera-wrapper
// pose into that root once; later headset/avatar motion does not drag it along.
//
// LayoutRoot flow begins at its top-left. Keep the camera-relative presentation
// offset centered at X/Y = 0, then move the layout origin left by half its width
// and up by half its height. The authored slide rectangle is therefore centered
// on the presentation basis before the whole subtree is scaled into world space.
let SLIDE_LAYOUT_WIDTH_GU = 30.0
let SLIDE_LAYOUT_HEIGHT_GU = 9.0
let slide_text = Text {
    name = "slide_text"
    "press B to reveal one weird rendering trick"
    font_size(0.72)
    TextureFiltering.linear()
    Raycastable.enabled()
}
let slide_color = C.rgba(1.0, 0.35, 0.78, 1.0) {
    name = "slide_color"
    EM.on()
    slide_text
}
let slide_subtitle_box = T {
    name = "slide_subtitle_box"
    Style {
        display("block")
        width(SLIDE_LAYOUT_WIDTH_GU)
        height(SLIDE_LAYOUT_HEIGHT_GU)
        word_wrap("normal")
        text_align("center")
        vertical_align("middle")
    }
    slide_color
}
let slide_layout = LayoutRoot {
    name = "slide_layout_root"
    available_width(SLIDE_LAYOUT_WIDTH_GU)
    available_height(SLIDE_LAYOUT_HEIGHT_GU)
    slide_subtitle_box
}
let slide_layout_origin_offset = T.position(
    -SLIDE_LAYOUT_WIDTH_GU / 2.0,
    SLIDE_LAYOUT_HEIGHT_GU / 2.0,
    0.0,
) {
    name = "slide_layout_origin_offset"
    slide_layout
}
let slide_offset = T.position(0.0, 0.0, 1.0).rotation(0.0, 3.14159, 0.0).scale(0.055, 0.055, 1.0) {
    name = "slide_presentation_offset"
    slide_layout_origin_offset
}
let slide_root = T {
    name = "detached_slide_root"
    Grabbable {}
    Draggable.plane("camera")
    slide_offset
}

// Materialize the placement root as an independent world root.
slide_root

// Each slide is state-complete so previous() can reapply an earlier state.
// Placement is deliberately not part of slide state. Slides only change their
// presentation; the button handler snapshots a fresh placement independently.
let slides = Animation.paused() {
    name = "short_form_slide_deck"

    Keyframe.at(0) {
        slide_text.set_font_size(0.72)
        slide_text.set_text("short form video creators\n\nhate it\n\nwhen you\n\nuse this\none simple trick!")
        slide_color.set_color([1.0, 0.35, 0.78, 1.0])
    }

    Keyframe.at(1) {
        slide_text.set_font_size(0.66)
        slide_text.set_text("i swear guys, this is not just a powerpoint\nThis changes everythng\n (again)")
        slide_color.set_color([0.10, 0.95, 1.0, 1.0])
    }

    Keyframe.at(2) {
        slide_text.set_font_size(0.82)
        slide_text.set_text("next we're gonna animate this text\ncause tbh i don't expect\nppl to read more\nthan 4 words at\nonce")
        slide_color.set_color([1.0, 0.84, 0.18, 1.0])
    }

    Keyframe.at(3) {
        slide_text.set_font_size(0.76)
        slide_text.set_text("im not even an influencer.\n im just a dog,\n but ur watching so w/e")
        slide_color.set_color([0.42, 1.0, 0.55, 1.0])
    }

    Keyframe.at(4) {
        slide_text.set_font_size(0.70)
        slide_text.set_text("like, follow, and subscribe\nfor more suspiciously fast cats")
        slide_color.set_color([0.72, 0.48, 1.0, 1.0])
    }
}

let xr_gamepad = InputXRGamepad {
    locomotion()
    speed(1.5)
}

let presentation_anchor = T.position(0.0, 0.08, 0.12) {
    name = "xr_camera_wrapper"
    CXR { Pointer {} }
}

T {
    name = "avatar_locomotion_root"
    InputXR.on() {
        xr_gamepad
        T {
            name = "xr_pose"
            AVC {
                mouth_open_from_amplitude(voice_level)
                mouth_open_rms_floor(0.005)
                mouth_open_rms_ceiling(0.09)
                mouth_open_smoothing(16.0)
                voice_level

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
                        EM.on()
                        PoseCapture { label("Bisket") asset_name("bisket") }
                        bisket_colliders()
                        bisket_shirt_physics(false)
                    }
                }

                presentation_anchor
                XREyeTracking.on()

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
        }
    }
}

slides

on(xr_gamepad, "XrButtonDown", fn(event) {
    if event.control == "ButtonB" {
        print("vtuber-slidedeck: received ButtonB; requesting next slide")
        let pose = presentation_anchor.world.trs()
        slide_root.world.trs(pose)
        slides.next()
    } else if event.control == "ButtonA" {
        print("vtuber-slidedeck: received ButtonA; requesting previous slide")
        let pose = presentation_anchor.world.trs()
        slide_root.world.trs(pose)
        slides.previous()
    }
})

// The XR runtime supplies both headset eyes. The window remains a companion
// surface rather than adding a separate desktop Camera3D scene view.
XR.on()
