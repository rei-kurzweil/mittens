// Paper craft XR workshop: reduced floating Sakura city and full-height stairs.
// cargo run --release -- load examples/paper-craft.mms
// XR: sticks move, B jumps; double-tap B to fly (2x speed), hold B/A to rise/descend.
// XR only. Walk the stairs to the street and back; flight remains available.
let ground_color = [0.40, 0.31, 0.32]

// Editable sky stops: direction Y is -1 at the nadir, 0 at the horizon, +1 overhead.
let sky_low_color = [1.0, 0.67, 0.78]
let sky_middle_color = [0.67, 0.53, 0.82]
let sky_high_color = [0.35, 0.40, 0.70]
let sky_low_point = -0.12
let sky_middle_point = 0.28
let sky_high_point = 0.90
let moon_direction = [-0.35, 0.30, -0.88]

let main_floors = 6
let storey_height = 3.2
let roof_height = main_floors * storey_height
let spawn_position = [-18.0, roof_height + 0.03, -1.4]
let pistol_scale = 0.085
let pistol_hand_offset = 0.05 // metres back toward the wrist, in the held grip frame

import { graphics_settings_panel } from "../assets/components/panels/graphics_settings_panel.mms"
import { xr_settings_panel } from "../assets/components/panels/xr_settings_panel.mms"
import { mini_explosion } from "../assets/components/mini-explosion.mms"
import { laser_machine_pistol } from "../assets/components/laser-machine-pistol.mms"
import { kawaii_twilight_sky } from "../assets/components/backgrounds/kawaii-twilight-sky.mms"
import { sakura_tree } from "../assets/components/plants/sakura_tree.mms"
import { pool_tile_material } from "../assets/components/materials/pool_tiles.mms"
import { pool_building, pool_building_with_stairs, ceramic_box, destroyable_ceramic_box } from "../assets/components/buildings/pool_building.mms"
import { display_car } from "../assets/components/vehicles/display_car.mms"
import { bisket_colliders } from "../assets/components/colliders/bisket.mms"
import { bisket_shirt_physics } from "../assets/components/secondary_motion/bisket-shirt-physics.mms"
import { mitten_bow_secondary_motion } from "../assets/components/secondary_motion/mitten-bow.mms"
import { bisket_humanoid_bone_map } from "../assets/components/humanoid_bone_maps/bisket.mms"
import { ambient_eye_saccades } from "../assets/components/animations/ambient_eye_saccades.mms"
import { vroid_arm_ik_bend_plane_prototype } from "../assets/components/arm_ik/vroid.mms"

// Share floor calibration with other XR demos and future app sessions.
let xr_calibration_file = XrSettings.floor.default_file()
XrSettings.floor.set_correction_meters(XrSettings.floor.correction_meters({file = xr_calibration_file}))

// Scope scene geometry for editor diagnostics without adding other editor panels.
Editor.active() {
name = "paper_craft_scene_editor"

RendererSettings { window_size(1440, 960) }
Clock.bpm(120.0)
BGC.rgba(0.35, 0.40, 0.70, 1.0)
AL.rgb(0.16, 0.17, 0.23)
RenderGraph {
    EmissivePass {}
    Bloom { intensity(0.65) radius_ndc(0.035) emissive_scale(1.0) half_res(true) }
}
// Shared Graphics settings controls bloom and the runtime pyramid image count.
Raycastable.disabled() {
    Selectable.off()
    kawaii_twilight_sky({
        low_color = sky_low_color, low_point = sky_low_point,
        middle_color = sky_middle_color, middle_point = sky_middle_point,
        high_color = sky_high_color, high_point = sky_high_point,
        moon_direction = moon_direction
    })
}
// Surface lighting stays independent of the visible sky and its baked moon.
T.position(moon_direction[0], moon_direction[1], moon_direction[2]) {
    DL.color(1.0, 0.84, 0.72).intensity(1.5)
}
T.position(0.5, 0.8, 0.3) { DL.color(0.62, 0.74, 1.0).intensity(0.35) }

let tiles = pool_tile_material()
let anime = MaterialDefinition.anime()
let hardware = Material(anime).inputs({
    shade_color = [0.42, 0.43, 0.52]
    shade_strength = 0.6
    shade_threshold = 0.1
    lit_threshold = 1.6
    rim_color = [1.0, 0.84, 0.92]
    rim_strength = 0.22
})
let pistol_material = Material(anime).inputs({
    shade_color = [0.55, 0.40, 0.51]
    shade_strength = 0.45
    shade_threshold = 0.1
    lit_threshold = 1.6
    rim_color = [1.0, 0.86, 0.94]
    rim_strength = 0.24
})
fn handle_laser_hit(hit) {
    Damage.at(hit.hit_point[0], hit.hit_point[1], hit.hit_point[2], 0.28, 1.0)
    let impact = mini_explosion(hit.hit_point, 2.0)
    impact.root
    impact.animation.play()
    let on_hit = hit.renderable.on_hit
    if on_hit != null { on_hit(hit) }
}
let white = [0.94, 0.96, 0.97]
let pink = [0.88, 0.65, 0.73]
let blue = [0.64, 0.78, 0.86]

fn box(position, size, color) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        R.cube() { hardware C.rgba(color[0], color[1], color[2], 1.0) }
        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.85) {} }
    }
}

fn destroyable_box(position, size, color) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        Destroyable.threshold(3.0)
        R.cube() { hardware C.rgba(color[0], color[1], color[2], 1.0) Raycastable.enabled() }
        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.85) {} }
    }
}

// Grounded player tower; four shorter neighbors float above the retained city ground.
pool_building_with_stairs("paper_craft_rooftop_building", [-18.0, 0.0, 0.0], main_floors, 1.0, tiles, pink)
let neighborhood_specs = [
    { label = "paper_craft_west_tower_north" position = [-18.0, 0.0, -25.0] floors = 5 lift = 0.25 side = 1.0 color = white },
    { label = "paper_craft_east_tower_center" position = [18.0, 0.0, 0.0] floors = 4 lift = 0.75 side = -1.0 color = blue },
    { label = "paper_craft_west_tower_south" position = [-18.0, 0.0, 25.0] floors = 5 lift = 1.0 side = 1.0 color = white },
    { label = "paper_craft_low_annex_west" position = [-39.0, 0.0, -10.0] floors = 2 lift = 0.5 side = 1.0 color = blue },
]
T {
    name = "paper_craft_neighborhood"
    for spec in neighborhood_specs {
        let base_height = spec.floors * storey_height * spec.lift
        pool_building(spec.label, [spec.position[0], base_height, spec.position[2]], spec.floors, spec.side, false, tiles, spec.color, true, true)
    }
}

// Muted, greyish pink-brown ground fills the spaces around the road and towers.
T { name = "paper_craft_ground" box([0.0, -0.48, 0.0], [120.0, 0.5, 140.0], ground_color) }

// Only the editor settings panel is materialized, including BVH snapshots.
T.position(-20.4, roof_height + 3.3, -3.8) {
    name = "paper_craft_editor_settings_mount"
    EditorUI { panels([{ panel = "settings" }]) }
}
T.position(-24.2, roof_height + 2.2, -1.3) {
    graphics_settings_panel({ root_name = "paper_craft_graphics_settings" })
}
// Beside the spawn-facing workbench, with the roof as a calibration reference.
T.position(-16.2, roof_height + 2.2, -3.6) {
    Grabbable {}
    xr_settings_panel({ calibration_file = xr_calibration_file, root_name = "paper_craft_xr_settings" })
}

// Full-height reflection facing the rooftop spawn; grip to reposition.
T.position(-14.8, roof_height + 1.35, -1.0).rotation(0.0, -1.57079633, 0.0).scale(2.6, 2.6, 0.06) {
    name = "paper_craft_mirror"
    Grabbable {}
    R.plane() { Mirror.quality(1440) {} Raycastable.enabled() }
}

// Long road, tiled pavements, lane markings and two existing display-car assets.
box([0.0, -0.12, 0.0], [14.0, 0.24, 112.0], [0.12, 0.14, 0.19])
for side in [-1.0, 1.0] {
    ceramic_box([side * 8.7, -0.02, 0.0], [3.4, 0.20, 112.0], white, tiles)
    box([side * 6.7, 0.008, 0.0], [0.10, 0.012, 110.0], [0.94, 0.90, 0.79])
}
// Bridge the short ground gap between the main building's door and pavement.
ceramic_box([-10.65, -0.07, 0.0], [1.3, 0.14, 2.4], white, tiles)
for dash in range(28) {
    box([0.0, 0.008, -53.0 + dash * 4.0], [0.13, 0.012, 1.8], [1.0, 0.82, 0.55])
}
display_car("paper_craft_road_car_north", [-3.4, -0.04, -17.0], 0.0, "paper_craft_car_north_mount", [])
display_car("paper_craft_road_car_south", [3.4, -0.04, 19.0], 3.14159265, "paper_craft_car_south_mount", [])

// Rooftop workbench bolted to a ceramic backing wall, with two cantilever brackets.
T.position(-18.0, roof_height, 0.0) {
    name = "paper_craft_workbench"
    destroyable_ceramic_box([0.0, 1.35, -4.85], [6.4, 2.7, 0.24], white, tiles)
    destroyable_box([0.0, 0.94, -4.12], [3.2, 0.10, 1.15], [0.30, 0.32, 0.40])
    destroyable_box([0.0, 1.004, -4.05], [2.85, 0.025, 0.90], [0.82, 0.56, 0.67])
    for side in [-1.0, 1.0] {
        box([side * 1.15, 0.57, -4.60], [0.10, 0.68, 0.12], [0.22, 0.24, 0.31])
        T.position(side * 1.15, 0.62, -4.24).rotation(0.65, 0.0, 0.0) {
            box([0.0, 0.0, 0.0], [0.09, 0.09, 1.0], [0.22, 0.24, 0.31])
        }
        box([side * 1.15, 0.89, -4.15], [0.12, 0.10, 1.12], [0.22, 0.24, 0.31])
    }
    // A small parts tray and a bench lamp leave the centre clear for the model.
    ceramic_box([1.12, 1.035, -4.12], [0.40, 0.045, 0.54], white, tiles)
    box([-1.30, 1.04, -4.46], [0.20, 0.06, 0.22], [0.22, 0.24, 0.31])
    box([-1.30, 1.42, -4.46], [0.045, 0.72, 0.045], [0.22, 0.24, 0.31])
    T.position(-1.30, 1.82, -4.33) {
        PL.color(1.0, 0.88, 0.92).intensity(0.8).distance(3.5)
        T.scale(0.30, 0.035, 0.20) {
            R.cube() { Material(MaterialDefinition.toon()) C.rgba(1.0, 0.90, 0.94, 1.0) EM.on().intensity(1.5) }
        }
    }
    T.position(-1.75, 2.14, -4.71).scale(0.085, 0.085, 0.085) {
        Text { "PAPER CRAFT / XR WORKSHOP" C.rgba(0.42, 0.22, 0.34, 1.0) Unlit {} }
    }
}

// The upgraded mitten tree: one rooftop specimen and two street planters.
fn paper_craft_planter(label, position) {
    return T.position(position[0], position[1], position[2]) {
        name = label
        ceramic_box([0.0, 0.18, 0.0], [2.2, 0.36, 2.2], white, tiles)
        box([0.0, 0.37, 0.0], [1.94, 0.025, 1.94], [0.24, 0.15, 0.19])
        T.position(0.0, 0.385, 0.0) { sakura_tree(3, 1.15, 0.24) }
    }
}
paper_craft_planter("paper_craft_rooftop_tree", [-14.1, roof_height, 2.4])
paper_craft_planter("paper_craft_street_tree_west", [-8.7, 0.08, -9.0])
paper_craft_planter("paper_craft_street_tree_east", [8.7, 0.08, 7.0])

// One tracked XR player; no desktop player or rooftop reset trigger.
let microphone = AudioInput { name = "paper_craft_mitten_microphone" }
let voice_level = Amplitude.rolling_window(0.080).highpass(120.0).highpass_resonance(0.707).from(microphone) {
    name = "paper_craft_mitten_voice_level"
}
let player_motion = Velocity {
    name = "paper_craft_mitten_velocity"
    T.position(spawn_position[0], spawn_position[1], spawn_position[2]) {
        name = "paper_craft_mitten_locomotion_root"
        InputXR.on() {
            InputXRGamepad { locomotion() speed(1.4) enable_flying(true) }
            T {
                name = "paper_craft_mitten_xr_driver"
                let xr_mitten = GLTF.new("assets/models/mitten.glb") {
                    name = "paper_craft_xr_mitten"
                    Raycastable.disabled()
                    Selectable.off()
                    HandTracking.new().source("openxr")
                    pistol_material
                    bisket_humanoid_bone_map()
                    MorphTargetMap.new()
                        .slot("left_eye_blink", "Fcl_EYE_Close_L")
                        .slot("right_eye_blink", "Fcl_EYE_Close_R")
                        .slot("viseme_aa", "Fcl_MTH_A")
                    bisket_colliders()
                    bisket_shirt_physics(false)
                    mitten_bow_secondary_motion()
                }
                let xr_avatar_control = AVC.movement_target("[name='paper_craft_mitten_locomotion_root']") {
                    name = "paper_craft_mitten_avatar_control"
                    crouch_enabled(true)
                    crouch_sole_offsets([0.0, 0.0530325, 0.0874534], [0.0, 0.0530325, 0.0874534])
                    WalkingIK { step_scale(1.3) }
                    mouth_open_from_amplitude(voice_level)
                    mouth_open_rms_center_range(0.0475, 0.085)
                    mouth_open_amount(1.0)
                    mouth_open_smoothing(16.0)
                    voice_level
                    initial_yaw(3.14159)
                    arm_ik(vroid_arm_ik_bend_plane_prototype())
                    hand_rotation_smoothing(220.0)
                    T { name = "paper_craft_xr_model_root" Raycastable.disabled() Selectable.off() xr_mitten }
                    T.position(0.0, 0.08, 0.12) {
                        name = "paper_craft_mitten_xr_camera"
                        CXR { Pointer {} }
                    }
                    // Live tracking supplies eyelid closure; idle animation
                    // owns pupil direction, as in the main mitten example.
                    XREyeTracking.on().priority(["htc", "vrchat_osc"]).enable_pupil_direction_tracking(false)
                    XRHand.new(true, "Left", "GripAim").laser() {
                        T {
                            RestAttachment.new("[name='J_Bip_L_Hand']", "[name='J_Bip_L_Middle3']") { Pointer { name = "paper_craft_left_hand_pointer" } }
                        }
                    }
                    XRHand.new(true, "Right", "GripAim").laser() {
                        T {
                            RestAttachment.new("[name='J_Bip_R_Hand']", "[name='J_Bip_R_Middle3']") { Pointer { name = "paper_craft_right_hand_pointer" } }
                        }
                    }
                }
                xr_avatar_control
                on(xr_mitten, "GLTFInitialized", fn(event) {
                    let left_eye = event.gltf.query("[name='J_Adj_L_FaceEye']")
                    let right_eye = event.gltf.query("[name='J_Adj_R_FaceEye']")
                    if left_eye && right_eye {
                        xr_avatar_control.attach(ambient_eye_saccades(left_eye, right_eye, 2.0))
                    } else {
                        print("XR mitten: eye joints were not found for idle gaze")
                    }
                })
            }
        }
    }
}
Gravity.enabled(false) { name = "paper_craft_mitten_gravity" player_motion }
let gravity_avatar = query("[name='paper_craft_mitten_avatar_control']")
on(gravity_avatar, "DataEvent", fn(event) {
    if event == "CapsuleReady" {
        query("[name='paper_craft_mitten_gravity']").set_enabled(true)
    }
})
if gravity_avatar.capsule_ready() {
    query("[name='paper_craft_mitten_gravity']").set_enabled(true)
}

XR.on()

// Weapon table near the spawn end of the roof, to the player's left.
T.position(-20.3, roof_height, 0.2) {
    name = "paper_craft_pistol_table"
    destroyable_ceramic_box([0.0, 0.90, 0.0], [2.8, 0.12, 1.15], white, tiles)
    destroyable_box([0.0, 0.974, 0.0], [2.64, 0.025, 1.0], [0.91, 0.72, 0.78])
    for x in [-1.18, 1.18] {
        for z in [-0.43, 0.43] { box([x, 0.42, z], [0.10, 0.84, 0.10], [0.30, 0.32, 0.40]) }
    }
}
fn rooftop_pistol(label, effect_label, x) {
    let tool = laser_machine_pistol({
        name = effect_label length = 40.0 flash_size = 0.28 beam_radius = 0.025
        ray_origin_offset = 0.18 on_hit = handle_laser_hit
    })
    let held = { active = false hand = "" pointer = null }
    // Preserve the held grip frame and the original rolled tabletop orientation.
    let grip = T.position(x + pistol_hand_offset, roof_height + 1.015, 0.2).rotation_quat(0.5, -0.5, -0.5, 0.5) {
        name = label + "_placement"
        Selectable.off()
        Grabbable.to_hand(true).hand_offset([0.0, 0.0, -0.105]) {}
        T.position(0.028, 0.085, 0.0402).rotation(0.0, 1.57079633, 0.0) { tool.mode_selector }
        T.position(0.0, 1.048425 * pistol_scale, pistol_hand_offset - 3.342659 * pistol_scale - 0.015) { tool.root }
        T.position(0.0, 0.0, pistol_hand_offset).rotation(0.0, 3.14159265, 0.0).scale(pistol_scale, pistol_scale, pistol_scale) {
            GLTF.new("assets/models/sakura-machine-pistol.glb") { name = label pistol_material }
        }
    }
    on(grip, "GrabStart", fn(event) {
        if event.target == grip {
            held.active = true
            held.pointer = event.pointer
            held.hand = ""
            if event.pointer == query("[name='paper_craft_left_hand_pointer']") { held.hand = "Left" }
            if event.pointer == query("[name='paper_craft_right_hand_pointer']") { held.hand = "Right" }
        }
    })
    on(grip, "GrabEnd", fn(event) {
        if event.target == grip && event.pointer == held.pointer {
            held.active = false
            held.pointer = null
            tool.use_end()
        }
    })
    on_global("XrButtonDown", fn(event) {
        if held.active && event.hand == held.hand {
            if event.control == "LeftTrigger" || event.control == "RightTrigger" { tool.use_start({ user = query("[name='paper_craft_mitten_avatar_control']") }) }
        }
    })
    on_global("XrButtonUp", fn(event) {
        if held.active && event.hand == held.hand {
            if event.control == "LeftTrigger" || event.control == "RightTrigger" { tool.use_end() }
        }
    })
    return grip
}
rooftop_pistol("paper_craft_machine_pistol", "paper_craft_pistol_laser", -20.30)

}
