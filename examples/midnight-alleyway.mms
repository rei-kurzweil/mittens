// View flags: XR-only by default. Set use_desktop = true (and use_xr = false)
// for a desktop window.
let use_xr = true
let use_desktop = false

import { make_rooftop_player } from "../assets/components/characters/rooftop_player.mms"
import { xr_settings_panel } from "../assets/components/panels/xr_settings_panel.mms"
import { pool_tile_material } from "../assets/components/materials/pool_tiles.mms"
import { pool_building_with_stairs, ceramic_box } from "../assets/components/buildings/pool_building.mms"
import { panorama_cylinder } from "../assets/components/backgrounds/panorama_cylinder.mms"
import { tripod_light } from "../assets/components/tripod_light.mms"

// One stairwell building at night, standing in front of a panorama photo of
// an alleyway. Mitten spawns on the top storey, one floor below the roof;
// the tripod lights there can be grabbed (XR grip or desktop left mouse).
//
//   cargo run --release -- load examples/midnight-alleyway.mms
//
// XR: sticks move, B jumps, double-tap B to toggle flight.

// Local voice drives Mitten's mouth. Stays neutral without a microphone.
let microphone = AudioInput {}
let voice_level = Amplitude.rolling_window(0.080).highpass(120.0).highpass_resonance(0.707).from(microphone) {}
let mouth_tuning = { center_rms = 0.0475 range_rms = 0.085 amount = 1.0 }

RendererSettings { window_visible(use_desktop) window_size(1280, 800) }
// Eye/blink clip timestamps in the player rig use seconds converted to beats.
Clock.bpm(120.0)
BGC.rgba(0.010, 0.014, 0.040, 1.0)
// The photo fills the horizon; above and below it the clear color shows.
panorama_cylinder("assets/images/panoramas/PXL_20261120_070951558.PANO.1920p.jpg", { brightness = 0.85 })
// Low ambient with a faint cool moonlight; the tripods do the real work.
AL.rgb(0.07, 0.08, 0.12)
T.position(-0.4, 0.8, 0.3) { DL.color(0.55, 0.62, 1.0).intensity(0.25) }

let storey_height = 3.2
let floors = 5
// The top storey's floor slab, one level below the roof.
let top_floor_y = (floors - 1) * storey_height
let tiles = pool_tile_material()
pool_building_with_stairs("midnight_alleyway_building", [0.0, 0.0, 0.0], floors, 1.0, tiles, [0.30, 0.34, 0.52])
T { name = "midnight_alleyway_ground" ceramic_box([0.0, -0.48, 0.0], [48.0, 0.5, 48.0], [0.16, 0.16, 0.20], tiles) }

// Kept clear of the stairwell opening (x = [-5.8, -1.4], z = [1.6, 5.6]).
let room_center = [1.0, top_floor_y + 1.2, -0.5]
tripod_light("midnight_alleyway_tripod_warm", [4.6, top_floor_y, 3.8], room_center,
    SL.color(1.0, 0.74, 0.50).intensity(5.0).distance(10.0).angle(0.62).penumbra(0.35))
tripod_light("midnight_alleyway_tripod_cool", [-4.8, top_floor_y, -4.0], room_center,
    SL.color(0.52, 0.66, 1.0).intensity(4.0).distance(10.0).angle(0.62).penumbra(0.35))
tripod_light("midnight_alleyway_tripod_pink", [5.0, top_floor_y, -4.2], room_center,
    PL.color(1.0, 0.45, 0.72).intensity(3.0).distance(6.0))

let local = make_rooftop_player({ prefix = "local" local_player = true avatar = "mitten" use_xr = use_xr use_desktop = use_desktop voice_level = voice_level mouth_tuning = mouth_tuning })
local.label.set_text("")
local.root.update_transform([2.0, top_floor_y, 0.5])
if use_desktop && !use_xr {
    I.speed(2.0) {
        name = "local_desktop_locomotion"
        InputTransformMode.forward_z() { fps_rotation() roll_axis_y() }
        local.root
    }
} else { local.locomotion }
if use_xr { XR.on() } else { XR.off() }
if use_xr {
    let xr_calibration_file = XrSettings.floor.default_file()
    XrSettings.floor.set_correction_meters(XrSettings.floor.correction_meters({file = xr_calibration_file}))
    T.position(-0.5, top_floor_y + 1.4, -4.6) {
        xr_settings_panel({ root_name = "midnight_alleyway_xr_settings", calibration_file = xr_calibration_file })
    }
}
if !use_xr {
    // Scripted hand targets keep the desktop arms posed without tracking hardware.
    local.left_driver.set_pose_valid(true)
    local.right_driver.set_pose_valid(true)
}
