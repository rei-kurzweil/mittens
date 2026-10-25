// First named-f32 custom-material scene.
// cargo run --release -- load examples/youtuber-room.mms
// Desktop: WASD + mouse. XR: controller locomotion and avatar-free pointers.
// Facing-gated Anime lighting with rim light, including on the seated Rei(mu) NPC.
// Four CombineMesh outputs share one retained material. LED cubes are baked
// once; only time_seconds changes per frame for their hue and brightness wave.

// View switches: desktop only = true/false; XR only = false/true; both = true/true.
// Omitting the desktop camera skips desktop rendering and presentation.
let desktop_view_enabled = true
let xr_view_enabled = false

import { f32 } from "types"
import { pose as floor_seated_pose } from "../assets/components/poses/rei-mu/floor-seated.pose.mms"
import { bisket_colliders } from "../assets/components/colliders/bisket.mms"
import { bisket_shirt_physics } from "../assets/components/secondary_motion/bisket-shirt-physics.mms"
import { rei_mu_bow_secondary_motion } from "../assets/components/secondary_motion/rei-mu-bow.mms"
import { ambient_eye_saccades } from "../assets/components/animations/ambient_eye_saccades.mms"

RendererSettings { window_size(1440, 960) }
Clock.bpm(120.0)
// One explicit switch restores the LED wave and Bloom. Set false for isolated
// lighting comparisons again; this does not change the Anime shading controls.
let led_effects_enabled = true
let led_intensity = 0.0
if led_effects_enabled {
    led_intensity = 1.0
    RenderGraph {
        EmissivePass { BlurPass { radius_ndc(0.04) half_res(true) } }
        Bloom { intensity(2.25) radius_ndc(0.086) emissive_scale(1.0) half_res(true) }
    }
} else {
    RenderGraph.off()
}
BGC.rgba(0.025, 0.032, 0.052, 1.0)
AL.rgb(0.0, 0.0, 0.0)
// Directional fill and monitor spotlight are omitted during the diagnostic.

// These resources deliberately have no scene-tree attachment. Completing the
// definition queues asynchronous backend preparation immediately.
let led_base = MaterialBase.extracted_emissive()
let led_definition = MaterialDefinition(led_base)
    .fragment("assets/shaders/youtuber-led-wave.frag")
    .inputs({
        time_seconds: f32
        hue_speed: f32
        wave_speed: f32
        wave_count: f32
        intensity: f32
    })
let ceiling_leds = Material(led_definition).inputs({
    time_seconds = 0.0
    // A brightness crest takes 6 / 0.5 = 12 seconds to lap the room.
    // The entire strip completes one hue cycle every four laps (48 seconds).
    hue_speed = 0.020833333333
    wave_speed = 0.5
    wave_count = 6.0
    intensity = led_intensity
})

let animation = { seconds = 0.0 }
on_global("FrameTick", fn(event) {
    // Every phase repeats after 48 seconds; wrapping this scene's shared
    // phase bounds f32 precision without a visible discontinuity.
    animation.seconds = (animation.seconds + event.dt_sec) % 48.0
    ceiling_leds.set_input("time_seconds", animation.seconds)
})

fn box(position, size, color) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        R.cube() { C.rgba(color[0], color[1], color[2], 1.0) }
    }
}

fn label(position, scale, text, color) {
    return T.position(position[0], position[1], position[2]).scale(scale, scale, scale) {
        Text { text C.rgba(color[0], color[1], color[2], 1.0) Unlit {} }
    }
}

fn award_line(text, label_size, row_height) {
    return T {
        Style {
            display("flex") width(100%) height(row_height)
            justify_content("center") align_items("center")
        }
        T {
            Style { display("inline-block") font_size(label_size) }
            Text { text Unlit {} }
        }
    }
}

let charcoal = [0.075, 0.085, 0.11]
let metal = [0.18, 0.21, 0.27]
let walnut = [0.37, 0.24, 0.16]
let fabric = [0.15, 0.17, 0.23]
let accent = [0.62, 0.12, 0.24]
let wall_lilac = [0.72, 0.60, 0.82]
let baseboard_lilac = [0.54, 0.42, 0.65]
let ceiling_white = [0.96, 0.95, 0.98]
let floor_wood = [0.39, 0.23, 0.12]
let floor_underlay = [0.16, 0.085, 0.035]
let room_shading = Shading.anime().shade_color([0.35, 0.35, 0.35]).shade_strength(0.8)
    .shade_threshold(0.35).lit_threshold(0.55)
    .rim_color([1.0, 1.0, 1.0]).rim_strength(0.18).rim_power(4.0) { name = "room_shading" }

// Flat desktop graphics use ordinary emissive materials; only the ceiling LEDs
// use the animated custom material. Non-emissive icon fills keep the glow selective.
let screen_gold = [1.0, 0.67, 0.10]
let screen_amber = [1.0, 0.40, 0.025]
let screen_pale_gold = [1.0, 0.91, 0.62]
let screen_white = [0.97, 0.98, 1.0]

fn screen_rect(position, size, color, intensity) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], 0.002) {
        R.cube() {
            C.rgba(color[0], color[1], color[2], 1.0)
            if intensity > 0.0 { EM.on().intensity(intensity) } else { Unlit {} }
        }
    }
}

fn screen_text(position, scale, text, color, intensity) {
    return T.position(position[0], position[1], position[2]).scale(scale, scale, scale) {
        Text { text C.rgba(color[0], color[1], color[2], 1.0) EM.on().intensity(intensity) }
    }
}

fn screen_icon(y, kind) {
    return T.position(0.44, y, 0.008) {
        if kind == 0 {
            // Folder: a quiet gold body with a glowing white document and gold lip.
            R.polygon("youtuber-room/monitor-folder/v1", [[-0.065, -0.04], [0.065, -0.04], [0.065, 0.035], [-0.005, 0.035], [-0.02, 0.055], [-0.065, 0.055]]) {
                C.rgba(0.56, 0.31, 0.055, 1.0) Unlit {}
            }
            screen_rect([0.012, 0.016, 0.003], [0.052, 0.07], screen_white, 0.7)
            screen_rect([0.0, -0.012, 0.006], [0.13, 0.048], screen_gold, 1.1)
        } else if kind == 1 {
            // Cat: warm amber face, bright ears/eyes and a white muzzle.
            R.polygon("youtuber-room/monitor-cat/v1", [[-0.03, -0.047], [0.03, -0.047], [0.055, -0.03], [0.06, 0.057], [0.025, 0.032], [-0.025, 0.032], [-0.06, 0.057], [-0.055, -0.03]]) {
                C.rgba(1.0, 0.40, 0.025, 1.0) EM.on().intensity(0.6)
            }
            screen_rect([-0.024, 0.008, 0.004], [0.014, 0.021], screen_pale_gold, 1.8)
            screen_rect([0.024, 0.008, 0.004], [0.014, 0.021], screen_white, 1.3)
            screen_rect([0.0, -0.021, 0.004], [0.021, 0.008], screen_white, 0.9)
        } else {
            // Terminal icon: dark inset, glowing gold frame and white prompt.
            screen_rect([0.0, 0.0, 0.0], [0.13, 0.095], screen_amber, 0.65)
            screen_rect([0.0, -0.003, 0.003], [0.116, 0.075], [0.045, 0.026, 0.01], 0.0)
            screen_text([-0.047, -0.015, 0.007], 0.031, ">_", screen_white, 1.3)
        }
        screen_text([-0.036, -0.086, 0.009], 0.018, "meow", screen_pale_gold, 0.8)
    }
}

let monitor_screen_content = T.position(-0.30, 1.34, -1.94) {
    name = "monitor_screen_content"
    Shading.toon()
    screen_rect([-0.16, -0.005, 0.0], [0.82, 0.53], [0.025, 0.016, 0.009], 0.0)
    screen_rect([-0.16, 0.231, 0.003], [0.82, 0.056], [0.12, 0.065, 0.015], 0.25)
    screen_text([-0.54, 0.218, 0.008], 0.018, "terminal", screen_pale_gold, 0.65)
    screen_rect([-0.16, 0.196, 0.005], [0.79, 0.003], screen_amber, 0.8)
    screen_text([-0.54, 0.105, 0.009], 0.035, "[rei@system ~]#", screen_gold, 1.1)
    screen_rect([0.008, 0.105, 0.009], [0.018, 0.032], screen_pale_gold, 1.2)
    screen_icon(0.19, 0)
    screen_icon(0.0, 1)
    screen_icon(-0.19, 2)
}

T {
    name = "youtuber_room"
    // Lilac walls, white ceiling and stained wood keep the shared Anime rim.
    room_shading
    // Architectural shell, 5 m wide and 4.6 m deep, with a 2.9 m ceiling.
    // A darker wood substrate sits 2 cm below the finished floor. Real gaps
    // between the planks expose it and the separately lit vertical plank edges.
    box([0.0, -0.08, 0.0], [5.1, 0.12, 4.7], floor_underlay)
    for plank in range(12) {
        box([-2.3375 + plank * 0.425, -0.01, 0.0], [0.413, 0.02, 4.7], floor_wood)
    }
    box([0.0, 1.45, -2.35], [5.1, 2.9, 0.12], wall_lilac)
    box([-2.55, 1.45, 0.0], [0.12, 2.9, 4.7], wall_lilac)
    box([2.55, 1.45, 0.0], [0.12, 2.9, 4.7], wall_lilac)
    box([0.0, 2.96, 0.0], [5.1, 0.12, 4.7], ceiling_white)
    // Front doorway leaves a generous entrance for desktop and room-scale XR.
    box([-1.90, 1.45, 2.35], [1.3, 2.9, 0.12], wall_lilac)
    box([1.90, 1.45, 2.35], [1.3, 2.9, 0.12], wall_lilac)
    box([0.0, 2.65, 2.35], [2.5, 0.5, 0.12], wall_lilac)
    box([0.0, 0.075, -2.27], [5.0, 0.15, 0.035], baseboard_lilac)
    box([-2.47, 0.075, 0.0], [0.035, 0.15, 4.6], baseboard_lilac)
    box([2.47, 0.075, 0.0], [0.035, 0.15, 4.6], baseboard_lilac)

    // Soft acoustic panels and a rug make the blockout read as a recording room.
    for panel in range(3) {
        box([-1.95 + panel * 0.30, 1.70, -2.245], [0.22, 0.95, 0.075], fabric)
    }
    box([0.35, 0.012, -0.1], [2.5, 0.022, 2.0], [0.13, 0.15, 0.21])
    box([0.35, 0.025, -0.1], [2.34, 0.005, 1.84], [0.20, 0.22, 0.29])

    // Desktop, black steel legs and a three-drawer pedestal.
    box([0.0, 0.79, -1.75], [2.8, 0.085, 0.90], walnut)
    for side in [-1.0, 1.0] {
        for depth in [-1.0, 1.0] {
            box([side * 1.22, 0.38, -1.75 + depth * 0.31], [0.065, 0.76, 0.065], charcoal)
        }
    }
    box([-1.02, 0.39, -1.76], [0.46, 0.70, 0.65], charcoal)
    for drawer in range(3) {
        box([-1.02, 0.19 + drawer * 0.22, -1.421], [0.41, 0.19, 0.025], metal)
        box([-1.02, 0.21 + drawer * 0.22, -1.398], [0.15, 0.018, 0.025], [0.55, 0.57, 0.61])
    }

    // Use the resized model at its authored scale. Recenter its 85 cm length
    // and rest its 3 cm diameter on the desktop.
    T.position(0.0, 0.8475, -1.75).rotation_euler(0.0, 1.31, 0.0) {
        name = "desk_straw"
        T.position(0.0, 0.0, 0.42907333) {
            GLTF.new("assets/models/straw.glb") {
                Shading.anime().shade_color([0.5, 0.5, 0.5])
                                .shade_strength(0.7)
                                .shade_threshold(0.35)
                                .lit_threshold(0.55)
                                .rim_color([1.0, 1.0, 1.0])
                                .rim_strength(0.18)
                                .rim_power(4.0)
            }
        }
    }

    // Monitor body and stand, with a gold terminal and emissive desktop icons.
    box([-0.30, 0.86, -2.02], [0.40, 0.025, 0.30], charcoal)
    box([-0.30, 1.05, -2.045], [0.075, 0.38, 0.07], metal)
    box([-0.30, 1.34, -1.99], [1.28, 0.72, 0.06], charcoal)
    T.position(-0.30, 1.34, -1.90) {
        name = "monitor_lights"
        PL.color(1.0, 1.0, 1.0).intensity(1.5).distance(5.0)
        // Spotlights aim along local +Z: out of the screen into the room.
        // A 60-degree half-angle gives a broad, soft 120-degree cone.
        // Previous spotlight: intensity 0.65, distance 5, angle 1.0471975512,
        // penumbra 0.45; disabled to leave exactly one point-light source.
    }
    T.position(-0.30, 1.34, -1.953).scale(1.21, 0.65, 1.0) {
        R.plane() { C.rgba(0.018, 0.012, 0.008, 1.0) Unlit {} }
    }
    monitor_screen_content

    // Keyboard: one small box per key, a wider space bar, and a separate mouse.
    box([-0.36, 0.853, -1.39], [0.70, 0.028, 0.235], charcoal)
    for row in range(4) {
        for key in range(14) {
            box([-0.67 + key * 0.047, 0.873, -1.47 + row * 0.048], [0.038, 0.014, 0.037], [0.48, 0.50, 0.55])
        }
    }
    box([-0.36, 0.875, -1.28], [0.29, 0.015, 0.035], metal)
    box([0.52, 0.841, -1.40], [0.44, 0.014, 0.35], [0.10, 0.24, 0.29])
    box([0.52, 0.867, -1.40], [0.085, 0.04, 0.13], [0.55, 0.57, 0.62])

    // PC tower with three inset, front-facing cooling fans.
    box([1.82, 0.50, -1.82], [0.40, 0.96, 0.64], charcoal)
    box([1.82, 0.51, -1.488], [0.32, 0.83, 0.015], [0.035, 0.045, 0.065])
    for fan in range(3) {
        T.position(1.82, 0.25 + fan * 0.25, -1.47).scale(0.22, 0.22, 1.0) {
            R.annulus_2d() { C.rgba(0.30, 0.61, 0.68, 1.0) Unlit {} }
        }
    }

    // Fictional 100k creator award, not an actual account or channel claim.
    box([1.35, 1.89, -2.25], [0.73, 0.91, 0.045], charcoal)
    box([1.35, 1.89, -2.217], [0.66, 0.84, 0.025], [0.68, 0.71, 0.76])
    box([1.35, 2.03, -2.197], [0.30, 0.22, 0.012], [0.22, 0.25, 0.31])
    T.position(1.35, 2.03, -2.185) {
        R.polygon("youtuber-room/award-play/v1", [[-0.06, -0.075], [0.075, 0.0], [-0.06, 0.075]]) {
            C.rgba(0.84, 0.87, 0.92, 1.0) Unlit {}
        }
    }
    // The layout's 60 cm width is centered on the plaque; each styled row
    // centers its measured text rather than guessing a left edge per string.
    T.position(1.05, 1.87, -2.18) {
        LayoutRoot {
            name = "award_text_layout"
            available_width(30.0) available_height(15.0) unit_scale(0.02)
            T {
                Style {
                    display("flex") flex_direction("column")
                    width(100%) height(15.0) gap(0.6)
                    justify_content("center") align_items("center")
                    color([0.12, 0.15, 0.21, 1.0])
                }
                award_line("100K", 3.3, 6.0)
                award_line("SUBSCRIBERS", 1.25, 3.0)
                award_line("MITTENS", 1.0, 3.0)
            }
        }
    }

    // Racing chair: contoured polygon shells plus box-built seat, arms and base.
    T.position(0.78, 0.0, -0.28).rotation_euler(0.0, -0.28, 0.0) {
        Draggable.plane([[1, 0, 0], [0, 0, 1]])
        
        name = "racing_chair"
        let outline = [[-0.30, 0.0], [0.30, 0.0], [0.32, 0.22], [0.27, 0.52], [0.37, 0.72], [0.25, 1.00], [0.15, 1.12], [-0.15, 1.12], [-0.25, 1.00], [-0.37, 0.72], [-0.27, 0.52], [-0.32, 0.22]]
        T.position(0,0,0.25) {
            for side in [-1.0, 1.0] {
                // The seat extends toward local -Z (the desk). Face each shell
                // outward and upholster both the sitter's side and the rear.
                T.position(0.0, 0.48, side * 0.055).rotation_euler(0.0, (1.0 - side) * 1.5707963268, 0.0) {
                    R.polygon("youtuber-room/chair-shell/v1", outline) { C.rgba(0.055, 0.065, 0.09, 1.0) }
                }
                T.position(0.0, 0.55, side * 0.065).rotation_euler(0.0, (1.0 - side) * 1.5707963268, 0.0) {
                    R.polygon("youtuber-room/chair-insert/v1", [[-0.20, 0.0], [0.20, 0.0], [0.18, 0.35], [0.28, 0.65], [0.15, 0.94], [-0.15, 0.94], [-0.28, 0.65], [-0.18, 0.35]]) { C.rgba(0.62, 0.12, 0.24, 1.0) }
                }
                box([0.0, 1.18, side * 0.08], [0.26, 0.39, 0.025], fabric)
                for slot in [-1.0, 1.0] {
                    box([slot * 0.095, 1.43, side * 0.083], [0.085, 0.065, 0.025], charcoal)
                }
            }

            for side in [-1.0, 1.0] {
                box([side * 0.38, 0.67, -0.18], [0.10, 0.06, 0.34], charcoal)
                box([side * 0.35, 0.54, -0.10], [0.045, 0.24, 0.045], metal)
            }

            box([0.0, 0.49, -0.23], [0.64, 0.14, 0.59], charcoal)
            box([0.0, 0.57, -0.23], [0.48, 0.045, 0.53], accent)
            
        }

        box([0.0, 0.26, 0.0], [0.09, 0.40, 0.09], metal)
        
        for spoke in range(5) {
            T.rotation_euler(0.0, spoke * 1.2566370614, 0.0) {
                box([0.0, 0.09, -0.22], [0.06, 0.05, 0.43], metal)
                box([0.0, 0.055, -0.41], [0.10, 0.095, 0.09], charcoal)
            }
        }
    }
}

// A separate NPC, with no avatar-control, XR camera or tracked-hand ownership.
// Her back leans toward the left wall; low, open legs and bent elbows keep the
// wrists above the lap. The authored pose is applied once after GLTF import.
let room_reimu = GLTF.new("assets/models/rei(mu).glb") {
    name = "room_reimu"
    room_shading
    floor_seated_pose()
    bisket_colliders()
    bisket_shirt_physics(false)
    rei_mu_bow_secondary_motion()
}
T.position(-2.22, 0.0, 0.0).rotation_euler(0.0, 1.15, 0.0) {
    name = "room_reimu_anchor"
    room_reimu
}
on(room_reimu, "GLTFInitialized", fn(event) {
    let left_eye = event.gltf.query("[name='J_Adj_L_FaceEye']")
    let right_eye = event.gltf.query("[name='J_Adj_R_FaceEye']")
    if left_eye && right_eye {
        room_reimu.attach(ambient_eye_saccades(left_eye, right_eye, 2.0))
    } else {
        print("Room Rei(mu): eye joints were not found for idle gaze")
    }
})

let strip_backing = Shading.anime().shade_color([0.5, 0.5, 0.5]).shade_strength(0.7)
    .shade_threshold(0.35).lit_threshold(0.55)
    .rim_color([1.0, 1.0, 1.0]).rim_strength(0.18).rim_power(4.0)

// One combined mesh per wall. Each cube carries its perimeter position in UV X,
// shared by all 24 vertices so the whole LED changes brightness together.
// The white plastic backing has a separate, non-emissive Anime material.
fn ceiling_strip(position, length, yaw, start_u, end_u, count) {
    return T.position(position[0], position[1], position[2]).rotation_euler(0.0, yaw, 0.0) {
        T {
            strip_backing
            box([0.0, 0.0, -0.018], [length, 0.075, 0.028], [0.92, 0.92, 0.92])
        }
        CombineMesh {
            name = "ceiling_led_surface"
            ceiling_leds
            for chip in range(count) {
                let fraction = (chip + 0.5) / count
                let u = start_u + fraction * (end_u - start_u)
                T.position((fraction - 0.5) * length, 0.0, 0.0).scale(0.035, 0.035, 0.008) {
                    R.cube() {
                        UV.uv(u, 0.5).uv(u, 0.5).uv(u, 0.5).uv(u, 0.5)
                            .uv(u, 0.5).uv(u, 0.5).uv(u, 0.5).uv(u, 0.5)
                            .uv(u, 0.5).uv(u, 0.5).uv(u, 0.5).uv(u, 0.5)
                            .uv(u, 0.5).uv(u, 0.5).uv(u, 0.5).uv(u, 0.5)
                            .uv(u, 0.5).uv(u, 0.5).uv(u, 0.5).uv(u, 0.5)
                            .uv(u, 0.5).uv(u, 0.5).uv(u, 0.5).uv(u, 0.5)
                    }
                }
            }
        }
    }
}
ceiling_strip([0.0, 2.77, -2.275], 4.94, 0.0, 0.0, 0.26, 80)
ceiling_strip([2.475, 2.77, 0.0], 4.55, -1.5707963268, 0.26, 0.5, 74)
ceiling_strip([0.0, 2.77, 2.275], 4.94, 3.1415926536, 0.5, 0.76, 80)
ceiling_strip([-2.475, 2.77, 0.0], 4.55, 1.5707963268, 0.76, 1.0, 74)

if desktop_view_enabled {
    I.speed(1.8) {
        InputTransformMode.forward_z() { fps_rotation() roll_axis_y() }
        T.position(0.0, 1.65, 2.1) { C3D { fov(72.0) Pointer {} } }
    }
}
if xr_view_enabled {
    T {
        InputXR.on() {
            InputXRGamepad { locomotion() speed(1.4) }
            T { CXR { Pointer {} } }
            XRHand.new(true, "Left", "GripAim").laser() { T { Pointer {} } }
            XRHand.new(true, "Right", "GripAim").laser() { T { Pointer {} } }
        }
    }
    XR.on()
} else {
    XR.off()
}
