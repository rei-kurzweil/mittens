// View flags: XR-only by default. Set use_desktop = true (and use_xr = false)
// for a desktop window.
let use_xr = true
let use_desktop = false
let start_in_third_person = false

import { make_rooftop_player } from "../assets/components/characters/rooftop_player.mms"
import { require_leg_rockets_for_flight } from "../assets/components/characters/rocket_flight_equipment.mms"
import { practice_health, practice_target } from "../assets/components/target_practice.mms"
import { arena_audio } from "../assets/components/arena_audio.mms"
import { avatar_picker_panel } from "../assets/components/panels/avatar_picker_panel.mms"
import { avatar_settings_panel } from "../assets/components/panels/avatar_settings_panel.mms"
import { input_settings_panel } from "../assets/components/panels/input_settings_panel.mms"
import { graphics_settings_panel } from "../assets/components/panels/graphics_settings_panel.mms"
import { dock, dock_dropzone } from "../assets/components/ui/dock.mms"
import { sakura_twilight_lighting } from "../assets/components/buildings/sakura_twilight_lighting.mms"
import { ship_input_controller } from "../assets/components/ship_input_controller.mms"
import { shared_sta_launcher } from "../assets/components/weapons/shared_sta_launcher.mms"
import { sta_ammo_cabinet } from "../assets/components/sta_ammo_cabinet.mms"
import { ship_machine_pistol } from "../assets/components/weapons/ship_machine_pistol.mms"
import { ship_environment_material } from "../assets/components/materials/ship_anime_materials.mms"
import { pool_tile_material } from "../assets/components/materials/pool_tiles.mms"
import { destroyable_ceramic_box } from "../assets/components/buildings/pool_building.mms"

// Target practice: Sakura Twilight Simpler's weapons and combat rules
// against a Mitten NPC, with no multiplayer. The NPC strafes left and right
// in front of the spawn under gravity, wearing an energy shield. Every five
// seconds it turns to you and fires its machine pistol (10% if it lands;
// sidestep while it turns). The grass floor and back wall are destroyable. Pistol hits break the
// shield first (10 per hit), then take 10% health. Rocket splash within 2 m
// takes 49% health and 50 shield; out to 6 m it takes 25 shield only. At
// zero the NPC bursts, drops its shield and comes back full after six
// seconds. Your own rockets can splash you too.
//
//   cargo run --release -- load examples/target-practice.mms
//
// Desktop: WASD + right-drag, Space fires a held weapon. XR: sticks move,
// triggers fire. Equip one STA rocket on each thigh to unlock flight.
// The Panels dock beside the spawn switches avatars (Mitten, Lumae, Miku)
// and opens the humanoid, input and graphics settings.

RendererSettings { window_visible(use_desktop) window_size(1280, 800) }
Clock.bpm(120.0)
BGC.rgba(0.68, 0.56, 0.84, 1.0)
RenderGraph {
    EmissivePass {}
    Bloom { intensity(0.65) radius_ndc(0.035) emissive_scale(1.0) half_res(true) }
}

let anime = MaterialDefinition.anime()
let hardware = Material(anime).inputs({
    shade_color = [0.42, 0.43, 0.52]
    shade_strength = 0.6
    shade_threshold = 0.1
    lit_threshold = 1.6
    rim_color = [1.0, 0.84, 0.92]
    rim_strength = 0.22
})
let lighting = sakura_twilight_lighting(1, hardware)
lighting.world
let pink = [0.88, 0.65, 0.73]

fn box(position, size, color) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        R.cube() { hardware C.rgba(color[0], color[1], color[2], 1.0) }
        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.85) {} }
    }
}

// A 32 m grass range of destroyable 4 m tiles (so each repeats the grass
// texture), with a destroyable pool-tile wall behind the target lane.
let grass = ship_environment_material("far")
fn grass_tile(x, z) {
    return T.position(x, -0.15, z).scale(4.0, 0.3, 4.0) {
        Destroyable.threshold(3.0)
        R.cube() {
            Raycastable.enabled()
            grass C.rgba(1.0, 1.0, 1.0, 1.0)
            Texture.with_uri("assets/textures/grass/ship-park-grass.png")
            TextureFiltering.linear()
        }
        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.85) {} }
    }
}
for ix in range(8) {
    for iz in range(8) { grass_tile(-14.0 + ix * 4.0, -22.0 + iz * 4.0) }
}
destroyable_ceramic_box([0.0, 1.0, -12.0], [14.0, 2.0, 0.4], [0.94, 0.96, 0.97], pool_tile_material("middle"))
for x in [-4.5, 4.5] { box([x, 0.4, -5.0], [1.2, 0.8, 0.6], pink) }

let sounds = arena_audio()
sounds.root

// Local voice drives the avatar's mouth. Stays neutral without a microphone.
let microphone = AudioInput {}
let voice_level = Amplitude.rolling_window(0.080).highpass(120.0).highpass_resonance(0.707).from(microphone) {}
let mouth_tuning = { center_rms = 0.0475 range_rms = 0.085 amount = 1.0 }

let local = make_rooftop_player({
    prefix = "local" local_player = true avatar = "mitten" use_xr = use_xr use_desktop = use_desktop
    camera_far = 250.0 third_person = start_in_third_person voice_level = voice_level mouth_tuning = mouth_tuning
    opaque_bucket = "near" show_health = true shield_mount = true
})
let player = practice_health(local.health, local.shield_mount, { on_damage = sounds.damage })
fn can_fire(user) { return player.alive() }

let target = practice_target({
    prefix = "npc" avatar = "mitten" use_xr = use_xr use_desktop = use_desktop
    position = [0.0, 0.0, -8.0] yaw = Math.pi health_view = local.health_view
    on_damage = sounds.damage physics = true
    // It turns to you every five seconds and fires; step aside to dodge.
    player = local.shield_mount
    on_player_hit = fn() { player.damage(1.0, 1.0) }
})
target.root

fn weapon_options(id) {
    return { weapon_id = id opaque_bucket = "near" can_fire = can_fire
        left_pointer = "#local_left_pointer" right_pointer = "#local_right_pointer" user = "#local_avatar" }
}
fn make_pistol(index) {
    let options = weapon_options("practice.pistol." + index)
    options.ammunition = 240
    options.on_hit = target.pistol_hit
    return ship_machine_pistol("practice_machine_pistol_" + index, options)
}
let pistols = [make_pistol(1), make_pistol(2), make_pistol(3)]
T.position(-2.2, 0.0, -1.4) {
    name = "practice_pistol_rack"
    box([0.0, 0.48, 0.0], [2.5, 0.96, 0.75], pink)
    for slot in range(3) {
        T.position((slot - 1.0) * 0.84, 1.04, 0.0) { pistols[slot] }
    }
}
let launcher_options = weapon_options("practice.sta.1")
launcher_options.on_explosion = fn(point) {
    // Until rockets have their own explosion sound, use the damage cue.
    sounds.damage(1.0)
    target.rocket_splash(point)
    let blast = player.blast_at(point)
    if blast { player.damage(blast.health, 1.0) }
}
let launcher = shared_sta_launcher("practice_sta_launcher", launcher_options)
T.position(1.6, 0.0, -1.4) {
    T.position(0.0, 1.04, 0.0) { launcher.root }
    box([0.0, 0.48, 0.0], [0.55, 0.96, 0.55], pink)
}
T.position(2.7, 0.0, -1.6) { sta_ammo_cabinet("practice_sta_ammo_cabinet") }
ship_input_controller(false, [launcher.grip], pistols)

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
local.label.set_text("You")
if use_desktop && !use_xr {
    I.speed(2.0) {
        name = "local_desktop_locomotion"
        InputTransformMode.forward_z() { fps_rotation() roll_axis_y() }
        local.root
    }
} else { local.locomotion }
if use_xr { XR.on() } else { XR.off() }
let xr_gamepad = query("#local_xr_locomotion")
if xr_gamepad { require_leg_rockets_for_flight(local.root, xr_gamepad) }
let xr_calibration_file = null
if use_xr {
    xr_calibration_file = XrSettings.floor.default_file()
    XrSettings.floor.set_correction_meters(XrSettings.floor.correction_meters({file = xr_calibration_file}))
}
fn apply_avatar_height() { local.apply_desired_height(report_avatar_status) }
on_global("DataEvent", fn(event) {
    if event == "AvatarSettingsChanged" { apply_avatar_height() }
})

let dock_panels = T { name = "practice_dock_panels" }
dock_panels
let dock_items = {
    avatar = { id = "avatar" label = "Avatar" root_name = "practice_avatar_picker"
        position = [0.0, 2.0, -2.2]
        build = fn() {
            let picker = avatar_picker_panel({
                root_name = "practice_avatar_picker"
                status = picker_status.text
                choices = [
                    { id = "mitten" label = "Mitten" },
                    { id = "lumae" label = "Lumae" },
                    { id = "miku" label = "Miku" },
                ]
                on_select = fn(choice) { local.select_avatar(choice, report_avatar_status) }
            })
            return T.position(-picker.width / 2.0, 0.0, 0.0) { picker.root }
        } }
    input = { id = "input" label = "Input" root_name = "practice_input_settings"
        position = [1.2, 2.3, -2.2]
        build = fn() { return input_settings_panel({ root_name = "practice_input_settings" }) } }
    humanoid = { id = "humanoid" label = "Humanoid" root_name = "practice_avatar_settings"
        position = [-4.65, 2.3, -2.2]
        build = fn() {
            return avatar_settings_panel({
                root_name = "practice_avatar_settings"
                show_xr = use_xr
                calibration_file = xr_calibration_file
                on_height_recommitted = apply_avatar_height
                camera_view = local.camera_view
            })
        } }
    graphics = { id = "graphics" label = "Graphics" root_name = "practice_graphics_settings"
        position = [-3.0, 2.3, -4.0]
        build = fn() { return graphics_settings_panel({ root_name = "practice_graphics_settings" }) } }
}
T.position(-0.8, 1.6, -1.65) {
    dock({
        root_name = "practice_dock"
        mount = dock_panels
        groups = [[dock_items.avatar], [dock_items.input, dock_items.humanoid, dock_items.graphics]]
    })
}
let dock_slot = T.position(0.35, 1.25, -0.45).rotation(0.0, -0.66, 0.0) {
    dock_dropzone({ name = "practice_dock_slot" })
}
local.root.attach(dock_slot)
if use_desktop { local.watch_pointer(query("[name='local_desktop_pointer']"), "desktop") }
if use_xr {
    local.watch_pointer(query("[name='local_left_pointer']"), "left")
    local.watch_pointer(query("[name='local_right_pointer']"), "right")
}
