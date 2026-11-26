// View flags: XR by default. Set use_desktop = true (and use_xr = false)
// for a desktop window.
let use_xr = true
let use_desktop = false
let start_in_third_person = false

// Bliss: a reference scene of soft green rolling hills under a clear blue
// sky. Implicit-surface hills wear the cartoon grass in a pastel green, a
// few implicit clouds drift overhead, and the sun stands 60 degrees up with
// its glow drawn by the sky shader. Drive the light yellow pickup over the
// hills (grip the driver's door, press POWER), visit the cherry blossom tree
// in its planter, and find a refractive cube and a frosted (rough
// transmission) cube on the hilltop ahead and to the right. The dock opens
// the avatar, mirror, input, humanoid, graphics and network panels.
//
//   cargo run --release -- load examples/bliss.mms
//
// Desktop: WASD + right-drag, V toggles the camera view. XR: sticks move,
// Y toggles the camera view.

import { f32, vec3 } from "types"
import { make_rooftop_player } from "../assets/components/characters/rooftop_player.mms"
import { cartoon_grass } from "../assets/components/materials/cartoon_grass.mms"
import { anime_surface } from "../assets/components/materials/anime_surface.mms"
import { sky_cloud_banks } from "../assets/components/backgrounds/clouds.mms"
import { sakura_tree } from "../assets/components/plants/sakura_tree.mms"
import { pickup_truck } from "../assets/components/vehicles/pickup_truck.mms"
import { ship_input_controller } from "../assets/components/ship_input_controller.mms"
import { fall_respawn } from "../assets/components/fall_respawn.mms"
import { avatar_picker_panel } from "../assets/components/panels/avatar_picker_panel.mms"
import { avatar_settings_panel } from "../assets/components/panels/avatar_settings_panel.mms"
import { input_settings_panel } from "../assets/components/panels/input_settings_panel.mms"
import { graphics_settings_panel } from "../assets/components/panels/graphics_settings_panel.mms"
import { network_settings_panel } from "../assets/components/panels/network_settings_panel.mms"
import { mirror_panel } from "../assets/components/panels/mirror_panel.mms"
import { dock, dock_dropzone } from "../assets/components/ui/dock.mms"

RendererSettings { window_visible(use_desktop) window_size(1280, 800) }
Clock.bpm(120.0)
BGC.rgba(0.36, 0.58, 0.88, 1.0)

// The sun, 60 degrees above the horizon, ahead and to the right of the spawn.
let SUN = [0.354, 0.866, -0.354]

// Sky. All colours are linear (the swapchain is sRGB).
let sky_definition = MaterialDefinition(MaterialBase()).fragment("assets/shaders/bliss-sky.frag")
    .inputs({
        zenith_color: vec3, zenith_power: f32, horizon_color: vec3, haze_width: f32,
        ground_color: vec3, sun_radius: f32, sun_direction: vec3, sun_strength: f32,
        glow_color: vec3, glow_strength: f32, sun_color: vec3, glow_width: f32
    })
let sky = Material(sky_definition).inputs({
    zenith_color = [0.045, 0.20, 0.62], zenith_power = 1.6,
    horizon_color = [0.52, 0.72, 0.92], haze_width = 0.08,
    ground_color = [0.30, 0.46, 0.30], sun_radius = 0.022,
    sun_direction = SUN, sun_strength = 1.0,
    glow_color = [1.0, 0.92, 0.72], glow_strength = 0.55,
    sun_color = [1.0, 0.98, 0.92], glow_width = 1.0
})
Raycastable.disabled() {
    Selectable.off()
    BG {
        name = "bliss_sky"
        // Radius 80 m, well inside the far plane.
        T.scale(160.0, 160.0, 160.0) { R.sphere() { name = "bliss_sky_surface" sky } }
    }
    // Implicit cloud banks, each [x, y, z, scale, yaw, puffs]. Implicit
    // surfaces draw with the foreground, so they stay inside the far plane.
    BG.occlusion_and_lighting() {
        name = "bliss_clouds"
        sky_cloud_banks([
            [-46.0, 26.0, -70.0, 4.6, 0.40, 5],
            [38.0, 30.0, -82.0, 4.0, -0.30, 5],
            [86.0, 24.0, 10.0, 3.6, 1.10, 3],
            [-64.0, 28.0, 48.0, 4.2, -0.90, 5],
        ], [0.97, 0.98, 1.0]).root
    }
}

AL.rgb(0.42, 0.50, 0.62)
T.position(SUN[0], SUN[1], SUN[2]) { DL.color(1.0, 0.96, 0.86).intensity(1.6) }
T.position(-0.5, 0.4, 0.6) { DL.color(0.62, 0.74, 1.0).intensity(0.45) }

// Ground and hills: a flat grass slab (its top at y = 0) and, for each hill,
// one large sphere sunk into it, so the ground cuts the sphere into a gentle
// cap. Each hill is its own small implicit surface bounded around its cap,
// one sphere per field.
// [x, z, radius, depth]: the cap rises radius - depth above the ground.
let HILLS = [
    [20.0, -24.0, 38.0, 33.0], [-26.0, -18.0, 40.0, 34.0], [30.0, 18.0, 36.0, 32.0],
    [-22.0, 26.0, 42.0, 36.5], [2.0, -34.0, 44.0, 37.0], [-38.0, 2.0, 36.0, 32.0],
    [38.0, -2.0, 38.0, 34.0], [8.0, 38.0, 40.0, 35.0],
]
let HILL_VOXEL = 0.5

// The ground's height at (x, z): the highest hill cap there, or 0.
fn hill_height(x, z) {
    let height = 0.0
    for hill in HILLS {
        let dx = x - hill[0]
        let dz = z - hill[1]
        let inside = hill[2] * hill[2] - dx * dx - dz * dz
        if inside > 0.0 {
            let cap = Math.sqrt(inside) - hill[3]
            if cap > height { height = cap }
        }
    }
    return height
}

// Pastel green that stays green: linear [0.40, 0.89, 0.29] shows as #99db85
// where lit, and the green-tinted shade keeps slopes away from the sun at
// #69a461 instead of grey. The lower lit threshold keeps sunny slopes lit.
let grass = cartoon_grass("far", {
    albedo = [0.40, 0.89, 0.29] shade_color = [0.32, 0.42, 0.40] lit_threshold = 1.2
    fade_near = 10.0 fade_far = 18.0
})
T {
    name = "bliss_ground"
    T.position(0.0, -0.5, 0.0).scale(600.0, 1.0, 600.0) {
        R.cube() {
            grass C.rgba(1.0, 1.0, 1.0, 1.0)
            Texture.with_uri("assets/textures/grass/cartoon-grass-tufts.png")
            TextureFiltering.linear()
        }
        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.8) {} }
    }
    for index in range(len(HILLS)) {
        let hill = HILLS[index]
        let reach = Math.sqrt(hill[2] * hill[2] - hill[3] * hill[3]) + 1.0
        let top = hill[2] - hill[3] + 0.5
        T.position(hill[0], 0.0, hill[1]) {
            ImplicitSurface.bounds(-reach, -0.3, -reach, reach, top, reach)
                .voxel_size(HILL_VOXEL).clip_floor(-0.08).uv_scale(0.25) {
                name = "bliss_hill_" + index
                grass C.rgba(1.0, 1.0, 1.0, 1.0)
                Texture.with_uri("assets/textures/grass/cartoon-grass-tufts.png")
                TextureFiltering.linear()
                // This zone receives the baked triangles, so feet and wheels
                // touch the same surface that is drawn.
                Zone.cube([reach, top, reach]) { Collidable.static().friction(0.8) {} }
                T.position(0.0, -hill[3], 0.0) { ImplicitSphere.radius(hill[2]) }
            }
        }
    }
}
fall_respawn("bliss_fall_respawn", { min_x = -40.0 max_x = 40.0 min_z = -40.0 max_z = 40.0 }, 0.0, 6.0)

// A cherry blossom tree in a raised soil planter, blossoms lined along the
// soil inside its border.
let solid = anime_surface()
let blossom_definition = MaterialDefinition(MaterialBase()).fragment("assets/shaders/sakura-blossom.frag")
    .inputs({ brightness: f32 })
let blossoms = Material(blossom_definition).inputs({ brightness = 1.0 })
let PLANTER = 2.6
fn planter_part(position, size, color) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        R.cube() { solid C.rgba(color[0], color[1], color[2], 1.0) }
        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.8) {} }
    }
}
T.position(-5.0, 0.0, -3.0) {
    name = "bliss_cherry_planter"
    Raycastable.enabled()
    let edge = PLANTER / 2.0
    let rim = [0.93, 0.84, 0.78]
    for side in [-1.0, 1.0] {
        planter_part([side * (edge - 0.08), 0.23, 0.0], [0.16, 0.46, PLANTER], rim)
        planter_part([0.0, 0.23, side * (edge - 0.08)], [PLANTER, 0.46, 0.16], rim)
    }
    T.position(0.0, 0.36, 0.0).scale(PLANTER - 0.3, 0.12, PLANTER - 0.3) {
        R.cube() { solid C.rgba(0.20, 0.135, 0.105, 1.0) }
    }
    T.position(0.0, 0.42, 0.0) { sakura_tree(3, 1.15, 0.24) }
    CombineMesh {
        name = "bliss_planter_blossoms"
        blossoms
        Texture.with_uri("assets/textures/cherry-blossoms/sakura-blossom-256.png")
        TextureFiltering.linear()
        for index in range(7) {
            let along = -edge + 0.32 + index * (PLANTER - 0.64) / 6.0
            for side in [-1.0, 1.0] {
                T.position(side * (edge - 0.3), 0.5, along).rotation(-0.25, side * 0.6, 0.0).scale(0.2, 0.2, 0.2) { R.square() {} }
                T.position(along, 0.5, side * (edge - 0.3)).rotation(-0.25, along, 0.0).scale(0.2, 0.2, 0.2) { R.square() {} }
            }
        }
    }
}

// A refractive cube and a frosted one on the hilltop ahead and to the right.
let hilltop = [20.0, hill_height(20.0, -24.0), -24.0]
T.position(hilltop[0], hilltop[1], hilltop[2]) {
    name = "bliss_glass_cubes"
    Raycastable.enabled()
    T.position(-1.1, 0.75, 0.0).rotation(0.0, 0.35, 0.0).scale(1.2, 1.2, 1.2) {
        R.cube() {
            name = "bliss_refraction_cube"
            C.rgba(0.82, 0.94, 1.0, 0.75)
            Refraction.ior(1.45).thickness(0.12).strength(1.0).edge_fade(0.025)
        }
        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static() {} }
    }
    T.position(1.1, 0.75, 0.0).rotation(0.0, -0.4, 0.0).scale(1.2, 1.2, 1.2) {
        R.cube() {
            name = "bliss_rough_transmission_cube"
            C.rgba(0.96, 0.88, 1.0, 0.75)
            RoughTransmission.ior(1.45).thickness(0.12).strength(1.0).edge_fade(0.025).roughness(0.45)
        }
        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static() {} }
    }
}

// A light yellow pickup (it bakes its own ambient occlusion) to drive over
// the hills. Colours are linear: this shows as a soft butter yellow.
let truck_motion = pickup_truck([1.0, 0.86, 0.34], null, true, { acceleration_drive = true vehicle_id = "bliss_pickup" })
T.position(7.0, 0.0, -3.0).rotation(0.0, -2.2, 0.0) {
    name = "bliss_truck_place"
    truck_motion
}
truck_motion.query("#pickup_truck").set_control_input({ brake = true })
ship_input_controller(false, [], [], null)

// The player: XR or desktop by the flags above, with voice driving the
// avatar's mouth, eye tracking when available, and the avatar's spring bones.
let microphone = AudioInput {}
let voice_level = Amplitude.rolling_window(0.080).highpass(120.0).highpass_resonance(0.707).from(microphone) {}
let mouth_tuning = { center_rms = 0.0475 range_rms = 0.085 amount = 1.0 }
let local = make_rooftop_player({
    prefix = "local" local_player = true avatar = "mitten" use_xr = use_xr use_desktop = use_desktop
    camera_far = 420.0 third_person = start_in_third_person voice_level = voice_level mouth_tuning = mouth_tuning
    opaque_bucket = "near"
})
local.label.set_text("You")
local.root.update_transform([0.0, 0.0, 4.0])
if use_desktop && !use_xr {
    I.speed(2.0) {
        name = "local_desktop_locomotion"
        InputTransformMode.forward_z() { fps_rotation() roll_axis_y() }
        local.root
    }
} else { local.locomotion }
if use_xr { XR.on() } else { XR.off() }
let xr_calibration_file = null
if use_xr {
    xr_calibration_file = XrSettings.floor.default_file()
    XrSettings.floor.set_correction_meters(XrSettings.floor.correction_meters({ file = xr_calibration_file }))
}
if use_desktop {
    on_global("KeyUp", fn(event) { if event.code == "KeyV" { local.toggle_camera_view() } })
}
if use_xr {
    on_global("XrButtonDown", fn(event) { if event.control == "ButtonY" { local.toggle_camera_view() } })
}

let picker_status = { text = "mitten is active" }
fn report_avatar_status(message) {
    picker_status.text = message
    let line = query("#avatar_picker_status")
    if line { line.set_text(message) }
}
fn apply_avatar_height() { local.apply_desired_height(report_avatar_status) }
on_global("DataEvent", fn(event) {
    if event == "AvatarSettingsChanged" { apply_avatar_height() }
})

// The dock, in front of the spawn, opens every panel.
let dock_panels = T { name = "bliss_dock_panels" }
dock_panels
let dock_items = {
    avatar = { id = "avatar" label = "Avatar" root_name = "bliss_avatar_picker"
        position = [0.0, 2.0, 1.8]
        build = fn() {
            let picker = avatar_picker_panel({
                root_name = "bliss_avatar_picker"
                status = picker_status.text
                choices = [{ id = "mitten" label = "Mitten" }, { id = "lumae" label = "Lumae" },
                    { id = "miku" label = "Miku" }, { id = "succubus" label = "Succubus" }]
                on_select = fn(choice) { local.select_avatar(choice, report_avatar_status) }
            })
            return T.position(-picker.width / 2.0, 0.0, 0.0) { picker.root }
        } }
    mirror = { id = "mirror" label = "Mirror" root_name = "bliss_mirror_frame"
        position = [3.4, 1.4, 2.6] rotation = [0.0, -Math.pi / 4.0, 0.0]
        build = fn() { return mirror_panel({ name = "bliss_mirror", width = 2.8, height = 1.6, quality = 1024 }) } }
    input = { id = "input" label = "Input" root_name = "bliss_input_settings"
        position = [1.2, 2.3, 1.8]
        build = fn() { return input_settings_panel({ root_name = "bliss_input_settings" }) } }
    humanoid = { id = "humanoid" label = "Humanoid" root_name = "bliss_avatar_settings"
        position = [-4.65, 2.3, 1.8]
        build = fn() {
            return avatar_settings_panel({
                root_name = "bliss_avatar_settings"
                show_xr = use_xr
                calibration_file = xr_calibration_file
                on_height_recommitted = apply_avatar_height
                camera_view = local.camera_view
            })
        } }
    graphics = { id = "graphics" label = "Graphics" root_name = "bliss_graphics_settings"
        position = [-3.0, 2.3, 0.6]
        build = fn() { return graphics_settings_panel({ root_name = "bliss_graphics_settings" }) } }
    network = { id = "network" label = "Network" root_name = "bliss_network_settings"
        position = [2.8, 2.3, 0.6]
        build = fn() { return network_settings_panel({ root_name = "bliss_network_settings" }) } }
}
T.position(-0.8, 1.6, 2.35) {
    dock({
        root_name = "bliss_dock"
        mount = dock_panels
        groups = [[dock_items.avatar, dock_items.mirror],
            [dock_items.input, dock_items.humanoid, dock_items.graphics, dock_items.network]]
    })
}
let dock_slot = T.position(0.35, 1.25, -0.45).rotation(0.0, -0.66, 0.0) {
    dock_dropzone({ name = "bliss_dock_slot" })
}
local.root.attach(dock_slot)
if use_desktop { local.watch_pointer(query("[name='local_desktop_pointer']"), "desktop") }
if use_xr {
    local.watch_pointer(query("[name='local_left_pointer']"), "left")
    local.watch_pointer(query("[name='local_right_pointer']"), "right")
}
