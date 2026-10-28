// Rooftop ceramic city / Sakura drone conversion workbench.
// cargo run --release -- load examples/sakura-drone-conversion.mms
// XR: sticks move, B jumps, falling to street level returns you to this roof.
// Desktop preview: WASD + mouse, without a second avatar or physics capsule.
let desktop_view_enabled = false
let xr_view_enabled = true
let initial_lumae_count = 0 // opt in with the rooftop pedestal; useful for profiling
let initial_full_buildings = 8 // surrounding buildings, 0..8; player tower stays detailed
let ground_color = [0.40, 0.31, 0.32]

// Editable sky stops: direction Y is -1 at the nadir, 0 at the horizon, +1 overhead.
let sky_low_color = [1.0, 0.72, 0.66]
let sky_high_color = [0.23, 0.32, 0.62]
let sky_low_point = -0.12
let sky_high_point = 0.82
let sun_direction = [-0.35, 0.30, -0.88]
let sun_color = [1.0, 0.84, 0.60]
let sun_angular_radius = 0.055
let sun_intensity = 5.0

let roof_height = 25.6
let spawn_position = [-18.0, roof_height + 0.03, -1.4]
// teleport_pit destinations describe the capsule centre, not the locomotion feet.
let reset_position = [spawn_position[0], roof_height + 1.2, spawn_position[2]]
let pistol_scale = 0.085
let pistol_hand_offset = 0.05 // metres back toward the wrist, in the held grip frame

import { info_panel } from "../assets/components/ui/info_panel.mms"
import { ui_unlit_material } from "../assets/components/materials/ui_unlit.mms"
import { graphics_settings_panel } from "../assets/components/panels/graphics_settings_panel.mms"
import { xr_settings_panel } from "../assets/components/panels/xr_settings_panel.mms"
import { mini_explosion } from "../assets/components/mini-explosion.mms"
import { metal_can } from "../assets/components/targets/metal-can.mms"
import { matcap_metal } from "../assets/components/materials/matcap_metal.mms"
import { laser_machine_pistol } from "../assets/components/laser-machine-pistol.mms"
import { gradient_skybox, background_sun } from "../assets/components/backgrounds/gradient_skybox.mms"
import { sakura_tree } from "../assets/components/plants/sakura_tree.mms"
import { pool_tile_material } from "../assets/components/materials/pool_tiles.mms"
import { pool_building, ceramic_box } from "../assets/components/buildings/pool_building.mms"
import { teleport_pit } from "../assets/components/teleport_pit.mms"
import { display_car } from "../assets/components/vehicles/display_car.mms"
import { bisket_colliders } from "../assets/components/colliders/bisket.mms"
import { bisket_shirt_physics } from "../assets/components/secondary_motion/bisket-shirt-physics.mms"
import { rei_mu_bow_secondary_motion } from "../assets/components/secondary_motion/rei-mu-bow.mms"
import { lumae_secondary_motion } from "../assets/components/secondary_motion/lumae.mms"
import { lumae_colliders } from "../assets/components/colliders/lumae.mms"
import { pose as lumae_relaxed_pose } from "../assets/components/poses/lumae/000-relaxed.pose.mms"
import { bisket_humanoid_bone_map } from "../assets/components/humanoid_bone_maps/bisket.mms"
import { ambient_eye_saccades } from "../assets/components/animations/ambient_eye_saccades.mms"
import { vroid_arm_ik_bend_plane_prototype } from "../assets/components/arm_ik/vroid.mms"

RendererSettings { window_size(1440, 960) }
Clock.bpm(120.0)
BGC.rgba(0.23, 0.32, 0.62, 1.0)
AL.rgb(0.16, 0.17, 0.23)
RenderGraph {
    EmissivePass {}
    Bloom { intensity(0.65) radius_ndc(0.035) emissive_scale(1.0) half_res(true) }
}
// Shared Graphics settings controls bloom and the runtime pyramid image count.
gradient_skybox(sky_low_color, sky_high_color, sky_low_point, sky_high_point)
background_sun(sun_direction, sun_angular_radius, sun_color, sun_intensity)
T.position(sun_direction[0], sun_direction[1], sun_direction[2]) {
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
// Target response is ordinary MMS for now. This is the place to experiment
// with hit policy before committing to Damageable or a general damage API.
let range_state = { hits = 0 last_hit = null }
fn can_hit(hit, target_state) {
    range_state.hits = range_state.hits + 1
    range_state.last_hit = hit
    let readout = query("[name='sakura_can_hit_readout']")
    if readout != null { readout.set_text("Can hits: " + range_state.hits) }
}
let can_metal = matcap_metal()
let shooting_cans = [
    metal_can({ name = "sakura_bench_can_1" position = [-19.2, roof_height + 1.023, -4.28] material = can_metal on_hit = can_hit }),
    metal_can({ name = "sakura_bench_can_2" position = [-18.75, roof_height + 1.023, -4.28] material = can_metal on_hit = can_hit }),
    metal_can({ name = "sakura_bench_can_3" position = [-16.7, roof_height + 1.023, -4.28] material = can_metal on_hit = can_hit }),
    metal_can({ name = "sakura_range_can_1" position = [-15.0, roof_height + 0.866, -4.2] material = can_metal on_hit = can_hit }),
    metal_can({ name = "sakura_range_can_2" position = [-14.2, roof_height + 0.866, -4.2] material = can_metal on_hit = can_hit }),
    metal_can({ name = "sakura_range_can_3" position = [-13.4, roof_height + 0.866, -4.2] material = can_metal on_hit = can_hit }),
]
for can in shooting_cans { can }
fn handle_laser_hit(hit) {
    let impact = mini_explosion(hit.hit_point, 2.0)
    impact.root
    impact.animation.play()
    let on_hit = hit.renderable.on_hit
    if on_hit != null { on_hit(hit) }
    for can in shooting_cans { can.on_blast(hit) }
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

// The player tower never belongs to the regenerated neighborhood subtree.
// Only this building gets separate storey/roof chunks for local CSG rebuilds.
pool_building("sakura_rooftop_building", [-18.0, 0.0, 0.0], 8, 1.0, true, tiles, pink, true, true)
let neighborhood = T { name = "sakura_neighborhood" }
neighborhood
let neighborhood_specs = [
    { label = "sakura_west_tower_-1" position = [-18.0, 0.0, -25.0] floors = 8 side = 1.0 color = white },
    { label = "sakura_east_tower_1" position = [18.0, 0.0, 0.0] floors = 8 side = -1.0 color = blue },
    { label = "sakura_west_tower_1" position = [-18.0, 0.0, 25.0] floors = 8 side = 1.0 color = white },
    { label = "sakura_east_tower_0" position = [18.0, 0.0, -25.0] floors = 8 side = -1.0 color = blue },
    { label = "sakura_east_tower_2" position = [18.0, 0.0, 25.0] floors = 8 side = -1.0 color = blue },
    { label = "sakura_low_annex_west" position = [-39.0, 0.0, -10.0] floors = 2 side = 1.0 color = blue },
    { label = "sakura_low_annex_east" position = [39.0, 0.0, 13.0] floors = 3 side = -1.0 color = pink },
    { label = "sakura_low_annex_south" position = [-18.0, 0.0, 47.0] floors = 2 side = 1.0 color = white },
]
let neighborhood_state = { full_count = initial_full_buildings generation = null }
fn regenerate_buildings(value) {
    let count = Math.round(value)
    if count < 0 { count = 0 }
    if count > 8 { count = 8 }
    if count == neighborhood_state.full_count && neighborhood_state.generation != null { return }
    if neighborhood_state.generation != null { neighborhood_state.generation.remove_subtree() }
    neighborhood_state.full_count = count
    let generation = T {
        name = "sakura_neighbor_generation"
        for index in range(8) {
            let spec = neighborhood_specs[index]
            pool_building(spec.label, spec.position, spec.floors, spec.side, false, tiles, spec.color, index < count, false)
        }
    }
    neighborhood.attach(generation)
    neighborhood_state.generation = generation
}
regenerate_buildings(initial_full_buildings)

// Muted, greyish pink-brown ground fills the spaces around the road and towers.
T { name = "sakura_ground" box([0.0, -0.48, 0.0], [120.0, 0.5, 140.0], ground_color) }

// Scene-specific detail and hit readout; rendering controls live in Graphics settings.
fn scene_controls() {
    let track = T.scale(0.75, 0.025, 0.08) {
        R.cube() { C.rgba(0.50, 0.32, 0.41, 1.0) Raycastable.enabled() { interaction_priority(120.0) } }
    }
    let thumb = T.scale(0.06, 0.10, 0.10) {
        R.sphere() { C.rgba(1.0, 0.73, 0.85, 1.0) Raycastable.enabled() { interaction_priority(120.0) } }
    }
    let slider = Slider.range(0.0, 8.0).step(1.0).value(neighborhood_state.full_count).width(0.75)
        .track(track).thumb(thumb) { name = "sakura_building_detail_slider" }
    let readout = Text { name = "sakura_building_detail_readout" "" + neighborhood_state.full_count + " / 8 fully generated" }
    let can_readout = Text { name = "sakura_can_hit_readout" "Can hits: " + range_state.hits }
    on(slider, "SliderChanged", fn(event) {
        regenerate_buildings(event.value)
        readout.set_text("" + neighborhood_state.full_count + " / 8 fully generated")
    })
    return T {
        Unlit {}
        Style { display("flex") flex_direction("column") width(100%) row_gap(0.5) font_size(0.75) }
        readout
        T {
            Style { display("block") width(100%) font_size(0.75) }
            can_readout
        }
        T {
            Grabbable.off()
            Style { display("flex") width(100%) height(3.0) align_items("center") justify_content("center") }
            slider
        }
    }
}
T.position(-21.1, roof_height + 2.2, -3.6) {
    info_panel({ root_name = "sakura_scene_controls" title = "Sakura / scene detail"
        width_gu = 32.0 unit_scale = 0.05 content_factory = scene_controls })
}
T.position(-24.2, roof_height + 2.2, -1.3) {
    graphics_settings_panel({ root_name = "sakura_graphics_settings" })
}
// Beside the spawn-facing workbench, with the roof as a calibration reference.
T.position(-16.2, roof_height + 2.2, -3.6) {
    Grabbable {}
    xr_settings_panel({ root_name = "sakura_xr_settings" })
}

// Full-height reflection facing the rooftop spawn; grip to reposition.
T.position(-14.8, roof_height + 1.35, -1.0).rotation(0.0, -1.57079633, 0.0).scale(2.6, 2.6, 0.06) {
    name = "sakura_mirror"
    Grabbable {}
    R.cube() { Mirror.quality(1440) {} Raycastable.enabled() }
}

// Long road, tiled pavements, lane markings and two existing display-car assets.
box([0.0, -0.12, 0.0], [14.0, 0.24, 112.0], [0.12, 0.14, 0.19])
for side in [-1.0, 1.0] {
    ceramic_box([side * 8.7, -0.02, 0.0], [3.4, 0.20, 112.0], white, tiles)
    box([side * 6.7, 0.008, 0.0], [0.10, 0.012, 110.0], [0.94, 0.90, 0.79])
}
for dash in range(28) {
    box([0.0, 0.008, -53.0 + dash * 4.0], [0.13, 0.012, 1.8], [1.0, 0.82, 0.55])
}
display_car("sakura_road_car_north", [-3.4, -0.04, -17.0], 0.0, "sakura_car_north_mount", [])
display_car("sakura_road_car_south", [3.4, -0.04, 19.0], 3.14159265, "sakura_car_south_mount", [])

// Rooftop workbench bolted to a ceramic backing wall, with two cantilever brackets.
T.position(-18.0, roof_height, 0.0) {
    name = "sakura_workbench"
    ceramic_box([0.0, 1.35, -4.85], [6.4, 2.7, 0.24], white, tiles)
    box([0.0, 0.94, -4.12], [3.2, 0.10, 1.15], [0.30, 0.32, 0.40])
    box([0.0, 1.004, -4.05], [2.85, 0.025, 0.90], [0.82, 0.56, 0.67])
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
        Text { "SAKURA / DRONE CONVERSION" C.rgba(0.42, 0.22, 0.34, 1.0) Unlit {} }
    }
}

// Small rooftop target stand, with three more cans beyond the workbench.
box([-14.2, roof_height + 0.80, -4.2], [2.4, 0.12, 0.70], [0.30, 0.32, 0.40])
for x in [-15.1, -13.3] {
    box([x, roof_height + 0.37, -4.2], [0.10, 0.74, 0.50], [0.30, 0.32, 0.40])
}

// Lumae is opt-in. Safe anchors keep arrivals out of the firing lane,
// stairwell, pistol table and planter; a small deterministic jitter varies each placement.
let lumae_spawn_state = { count = 0 }
let lumae_group = T { name = "sakura_lumae_group" }
lumae_group
let lumae_count = Text { name = "sakura_lumae_count" "Lumae: 0" }
let lumae_spawn_anchors = [
    [-23.6, -4.1], [-22.7, -2.6], [-23.8, -0.1],
    [-12.0, -4.0], [-12.0, -2.0], [-12.0, 0.0],
    [-20.0, 4.7], [-18.0, 4.7], [-16.0, 4.7],
]
fn spawn_fraction(seed) {
    let value = Math.sin(seed * 12.9898) * 43758.5453
    return value - Math.floor(value)
}
fn spawn_lumae() {
    let count = lumae_spawn_state.count
    let index = count - Math.floor(count / 9.0) * 9.0
    let anchor = lumae_spawn_anchors[index]
    let x = anchor[0] + (spawn_fraction(count + 1.0) - 0.5) * 0.4
    let z = anchor[1] + (spawn_fraction(count + 17.0) - 0.5) * 0.4
    lumae_spawn_state.count = count + 1
    let label = "sakura_lumae_" + lumae_spawn_state.count
    let avatar = T.position(x, roof_height, z) {
        name = label + "_placement"
        GLTF.new("assets/models/lumae.glb") {
            name = label
            pistol_material
            lumae_relaxed_pose()
            lumae_colliders()
            lumae_secondary_motion()
        }
    }
    lumae_group.attach(avatar)
    lumae_count.set_text("Lumae: " + lumae_spawn_state.count)
}
let add_lumae = T {
    name = "sakura_add_lumae_button"
    Grabbable.off()
    Raycastable.click_only() { interaction_priority(150.0) }
    Style {
        display("flex") width(12.0) height(3.5) align_items("center") justify_content("center")
        font_size(1.0) background_color([1.0, 0.86, 0.90, 1.0]) color([0.0, 0.0, 0.0, 1.0]) background_z(-0.04)
    }
    T.position(0.0, 0.0, 0.03) { Text { "Add Lumae" } }
}
on(add_lumae, "Click", fn(event) { spawn_lumae() })
T.position(-17.0, roof_height, 1.2) {
    name = "sakura_lumae_pedestal"
    ceramic_box([0.0, 0.425, 0.0], [0.6, 0.85, 0.6], white, tiles)
    ceramic_box([0.0, 0.89, 0.0], [0.64, 0.08, 0.64], pink, tiles)
    T.position(-0.24, 0.94, -0.12).rotation(-1.57079633, 0.0, 0.0).scale(0.04, 0.04, 0.04) {
        ui_unlit_material()
        LayoutRoot {
            available_width(12.0)
            T {
                Style { display("flex") flex_direction("column") width(12.0) row_gap(0.4) color([0.0, 0.0, 0.0, 1.0]) }
                add_lumae
                T { Style { display("block") height(2.0) font_size(0.9) } lumae_count }
            }
        }
    }
}
for index in range(initial_lumae_count) { spawn_lumae() }

// The upgraded Rei(mu) tree: one rooftop specimen and two street planters.
fn sakura_planter(label, position) {
    return T.position(position[0], position[1], position[2]) {
        name = label
        ceramic_box([0.0, 0.18, 0.0], [2.2, 0.36, 2.2], white, tiles)
        box([0.0, 0.37, 0.0], [1.94, 0.025, 1.94], [0.24, 0.15, 0.19])
        T.position(0.0, 0.385, 0.0) { sakura_tree(3, 1.15, 0.24) }
    }
}
sakura_planter("sakura_rooftop_tree", [-14.1, roof_height, 2.4])
sakura_planter("sakura_street_tree_west", [-8.7, 0.08, -9.0])
sakura_planter("sakura_street_tree_east", [8.7, 0.08, 7.0])

// teleport_pit has a -10 m sensor offset. This places its invisible trigger at
// y=2 +/-2 m, so falling players reset before they land on the road. No spikes.
teleport_pit("sakura_roof_reset", [0.0, 12.0, 0.0], [150.0, 4.0, 160.0], reset_position, "none")

if desktop_view_enabled {
    I.speed(4.0) {
        InputTransformMode.forward_z() { fps_rotation() roll_axis_y() }
        T.position(spawn_position[0], roof_height + 1.65, spawn_position[2]) {
            name = "sakura_desktop_camera"
            C3D { fov(74.0) Pointer { name = "sakura_desktop_pointer" } }
        }
    }
}

if xr_view_enabled {
    let microphone = AudioInput { name = "sakura_reimu_microphone" }
    let voice_level = Amplitude.rolling_window(0.080).highpass(120.0).highpass_resonance(0.707).from(microphone) {
        name = "sakura_reimu_voice_level"
    }
    let player_motion = Velocity {
        name = "sakura_reimu_velocity"
        T.position(spawn_position[0], spawn_position[1], spawn_position[2]) {
            name = "sakura_reimu_locomotion_root"
            InputXR.on() {
                InputXRGamepad { locomotion() speed(1.4) }
                T {
                    name = "sakura_reimu_xr_driver"
                    let xr_reimu = GLTF.new("assets/models/rei(mu).glb") {
                        name = "sakura_xr_reimu"
                        HandTracking.new().source("openxr")
                        pistol_material
                        bisket_humanoid_bone_map()
                        MorphTargetMap.new()
                            .slot("left_eye_blink", "Fcl_EYE_Close_L")
                            .slot("right_eye_blink", "Fcl_EYE_Close_R")
                            .slot("viseme_aa", "Fcl_MTH_A")
                        bisket_colliders()
                        bisket_shirt_physics(false)
                        rei_mu_bow_secondary_motion()
                    }
                    let xr_avatar_control = AVC.movement_target("[name='sakura_reimu_locomotion_root']") {
                        name = "sakura_reimu_avatar_control"
                        crouch_enabled(true)
                        crouch_sole_offsets([0.0, 0.0530325, 0.0874534], [0.0, 0.0530325, 0.0874534])
                        mouth_open_from_amplitude(voice_level)
                        mouth_open_rms_center_range(0.0475, 0.085)
                        mouth_open_amount(1.0)
                        mouth_open_smoothing(16.0)
                        voice_level
                        initial_yaw(3.14159)
                        arm_ik(vroid_arm_ik_bend_plane_prototype())
                        hand_rotation_smoothing(220.0)
                        T { xr_reimu }
                        T.position(0.0, 0.08, 0.12) {
                            name = "sakura_reimu_xr_camera"
                            CXR { Pointer {} }
                        }
                        // Live tracking supplies eyelid closure; idle animation
                        // owns pupil direction, as in the main Rei(mu) example.
                        XREyeTracking.on().priority(["htc", "vrchat_osc"]).enable_pupil_direction_tracking(false)
                        XRHand.new(true, "Left", "GripAim").laser() {
                            T {
                                RestAttachment.new("[name='J_Bip_L_Hand']", "[name='J_Bip_L_Middle3']") { Pointer { name = "sakura_left_hand_pointer" } }
                            }
                        }
                        XRHand.new(true, "Right", "GripAim").laser() {
                            T {
                                RestAttachment.new("[name='J_Bip_R_Hand']", "[name='J_Bip_R_Middle3']") { Pointer { name = "sakura_right_hand_pointer" } }
                            }
                        }
                    }
                    xr_avatar_control
                    on(xr_reimu, "GLTFInitialized", fn(event) {
                        let left_eye = event.gltf.query("[name='J_Adj_L_FaceEye']")
                        let right_eye = event.gltf.query("[name='J_Adj_R_FaceEye']")
                        if left_eye && right_eye {
                            xr_avatar_control.attach(ambient_eye_saccades(left_eye, right_eye, 2.0))
                        } else {
                            print("XR Rei(mu): eye joints were not found for idle gaze")
                        }
                    })
                }
            }
        }
    }
    Gravity.enabled(false) { name = "sakura_reimu_gravity" player_motion }
    let gravity_avatar = query("[name='sakura_reimu_avatar_control']")
    on(gravity_avatar, "DataEvent", fn(event) {
        if event == "CapsuleReady" {
            query("[name='sakura_reimu_gravity']").set_enabled(true)
        }
    })
    if gravity_avatar.capsule_ready() {
        query("[name='sakura_reimu_gravity']").set_enabled(true)
    }

    on_global("XrButtonDown", fn(event) {
        if event.control == "ButtonB" && player_motion.grounded() {
            player_motion.translate_world([0.0, 4.5, 0.0])
        }
    })
    XR.on()
} else {
    XR.off()
}

// Weapon table near the spawn end of the roof, to the player's left.
T.position(-20.3, roof_height, 0.2) {
    name = "sakura_pistol_table"
    ceramic_box([0.0, 0.90, 0.0], [2.8, 0.12, 1.15], white, tiles)
    box([0.0, 0.974, 0.0], [2.64, 0.025, 1.0], [0.91, 0.72, 0.78])
    for x in [-1.18, 1.18] {
        for z in [-0.43, 0.43] { box([x, 0.42, z], [0.10, 0.84, 0.10], [0.30, 0.32, 0.40]) }
    }
}
fn rooftop_pistol(label, effect_label, x) {
    let tool = laser_machine_pistol({
        name = effect_label length = 40.0 flash_size = 0.28 beam_radius = 0.025
        ray_origin_offset = 0.06 on_hit = handle_laser_hit
    })
    let held = { active = false hand = "" pointer = null }
    // Preserve the held grip frame and the original rolled tabletop orientation.
    let grip = T.position(x + pistol_hand_offset, roof_height + 1.015, 0.2).rotation_quat(0.5, -0.5, -0.5, 0.5) {
        name = label + "_placement"
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
            held.hand = "Desktop"
            if event.pointer == query("[name='sakura_left_hand_pointer']") { held.hand = "Left" }
            if event.pointer == query("[name='sakura_right_hand_pointer']") { held.hand = "Right" }
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
            if event.control == "LeftTrigger" || event.control == "RightTrigger" { tool.use_start({ user = query("[name='sakura_reimu_avatar_control']") }) }
        }
    })
    on_global("XrButtonUp", fn(event) {
        if held.active && event.hand == held.hand {
            if event.control == "LeftTrigger" || event.control == "RightTrigger" { tool.use_end() }
        }
    })
    on_global("KeyDown", fn(event) {
        if held.active && held.hand == "Desktop" && event.code == "Space" { tool.use_start({ user = held.pointer }) }
    })
    on_global("KeyUp", fn(event) {
        if held.active && held.hand == "Desktop" && event.code == "Space" { tool.use_end() }
    })
    return grip
}
rooftop_pistol("sakura_machine_pistol", "sakura_pistol_laser", -21.15)
rooftop_pistol("sakura_machine_pistol_2", "sakura_pistol_laser_2", -20.30)
rooftop_pistol("sakura_machine_pistol_3", "sakura_pistol_laser_3", -19.45)
