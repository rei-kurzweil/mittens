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
import { mountain_sunrise } from "../assets/components/backgrounds/mountain_sunrise.mms"
import { ship_input_controller } from "../assets/components/ship_input_controller.mms"
import { shared_sta_launcher } from "../assets/components/weapons/shared_sta_launcher.mms"
import { sta_ammo_cabinet } from "../assets/components/sta_ammo_cabinet.mms"
import { ship_machine_pistol } from "../assets/components/weapons/ship_machine_pistol.mms"
import { ship_environment_material } from "../assets/components/materials/ship_anime_materials.mms"
import { pool_tile_material } from "../assets/components/materials/pool_tiles.mms"
import { destroyable_ceramic_box } from "../assets/components/buildings/pool_building.mms"
import { fall_respawn } from "../assets/components/fall_respawn.mms"
import { pickup_truck } from "../assets/components/vehicles/pickup_truck.mms"
import { vehicle_patrol } from "../assets/components/vehicles/vehicle_patrol.mms"
import { display_car_desktop, display_car_xr } from "../assets/components/vehicles/display_car.mms"

// Truck bed demo: target practice on a range three times as long, with a
// road across it. A control script drives a pickup back and forth along the
// road (through the truck's common vehicle input, not its occupant), while
// Mitten rides in the driver's seat, shielded, aiming and firing at you the
// way the target practice NPC does. Climb the crates to the bridge over the
// road and jump down into the truck bed as it passes: you ride along,
// walking around the bed while it drives, turns and tilts, and step off with
// its speed. Items released inside the bed ride along too. The ground,
// road, crates and bridge are all destroyable. Or take the mech
// parked left of the spawn: grip it to climb in, press POWER, drive it to the
// road and shoot the truck with its nose laser (trigger) or its three-rocket
// burst (the red dashboard button).
// Pistols and rockets work as in target practice; the far wall is
// destroyable. All of it sits in a valley at sunrise (mountain_sunrise).
//
//   cargo run --release -- load examples/truck-bed-demo.mms
//
// Desktop: WASD + right-drag, Space fires a held weapon. XR: sticks move,
// triggers fire. Equip one STA rocket on each thigh to unlock flight.

RendererSettings { window_visible(use_desktop) window_size(1280, 800) }
Clock.bpm(120.0)
BGC.rgba(0.42, 0.24, 0.34, 1.0)
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
// Sunrise in the mountains: four rings of peaks 40 to 110 m beyond the range
// (inside the 250 m far plane) leave a winding valley along the road, with the sun coming
// up in its farthest pass, ahead and to the right.
let sunrise = mountain_sunrise(null)
sunrise.world
let pink = [0.88, 0.65, 0.73]
let crate = [0.80, 0.62, 0.44]

// Everything built from boxes is destroyable: crates, the bridge, the stands.
fn box(position, size, color) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        Destroyable.threshold(3.0)
        R.cube() { Raycastable.enabled() hardware C.rgba(color[0], color[1], color[2], 1.0) }
        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.85) {} }
    }
}

// A 96 x 30 m range of destroyable 8 m tiles: grass, with a 6 m road of
// tiles across it at z = -12. A truck that drops through a hole is caught
// and put back like anyone else.
// Cartoon grass, tinted a little pink to sit with the sunrise.
let grass = ship_environment_material("far")
fn grass_tile(x, z) {
    return T.position(x, -0.15, z).scale(8.0, 0.3, 8.0) {
        Destroyable.threshold(3.0)
        R.cube() {
            Raycastable.enabled()
            grass C.rgba(0.94, 0.80, 0.72, 1.0)
            Texture.with_uri("assets/textures/grass/cartoon-grass.png")
            TextureFiltering.linear()
        }
        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.85) {} }
    }
}
let road_z = -12.0
for ix in range(12) {
    for z in [-19.0, -5.0, 3.0] { grass_tile(-44.0 + ix * 8.0, z) }
}
T {
    name = "demo_road"
    for ix in range(12) {
        T.position(-44.0 + ix * 8.0, -0.15, road_z).scale(8.0, 0.3, 6.0) {
            Destroyable.threshold(3.0)
            R.cube() { Raycastable.enabled() hardware C.rgba(0.22, 0.21, 0.27, 1.0) }
            Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.85) {} }
        }
    }
}
fall_respawn("demo_fall_respawn", { min_x = -47.0 max_x = 47.0 min_z = -22.0 max_z = 8.0 }, 0.0, 5.0)
destroyable_ceramic_box([0.0, 1.0, -22.5], [96.0, 2.0, 0.4], [0.94, 0.96, 0.97], pool_tile_material("middle"))

// Crates stacked as stairs up to a bridge 4 m over the road, beside the
// middle of the range. The bridge's low rails are easy to hop over.
let stairs_x = 6.0
T {
    name = "demo_crate_stairs"
    for step in range(8) {
        let height = (step + 1) * 0.5
        box([stairs_x, height / 2.0, -1.5 - step * 0.6], [2.5, height, 0.6], crate)
    }
    box([stairs_x, 2.0, -6.6], [2.5, 4.0, 1.2], crate)
    box([stairs_x, 3.9, road_z], [2.5, 0.2, 9.6], pink)
    for side in [-1.0, 1.0] {
        box([stairs_x + side * 1.2, 4.2, road_z], [0.1, 0.4, 9.6], pink)
    }
}

// The pickup, driven by a control script, with Mitten in the driver's seat.
// Shoot the engine block (front of the truck) enough and it smokes, then
// burns, then the truck is wrecked: it explodes and stops. Shot-out wheels
// cost it traction. Twelve seconds after a wreck a new, undamaged truck
// replaces it at the start of the road, and Mitten moves to the new one.
let wreck_events = T { name = "demo_truck_wreck_events" }
wreck_events
let wreck_delay = Animation.paused().length(24.0) {
    Keyframe.at(0.0) {}
    Keyframe.at(24.0) { emit_data(wreck_events, "TruckRebuilt") }
}
wreck_events.attach(wreck_delay)
// The current truck's parts; Mitten's ride (chassis space) follows it.
let fleet = { place = null motion = null truck = null chassis = null patrol = null }
let truck_ride = { chassis = null seat = [-0.33, 0.20, 0.30] door = [-1.95, 0.0, -0.25] ground_y = -0.15 }
fn spawn_truck() {
    let motion = pickup_truck([0.55, 0.78, 0.90], null, true, { acceleration_drive = true vehicle_id = "demo_pickup" })
    let place = T.position(-12.0, 0.0, road_z).rotation(0.0, -Math.pi / 2.0, 0.0) {
        name = "demo_truck_place"
        motion
    }
    place
    let truck = motion.query("#pickup_truck")
    // Every other run is slower, to make jumping in easier.
    let patrol = vehicle_patrol(motion, truck, {
        name = "demo_truck" axis = "x" center = 0.0 drive_seconds = 4.0 slow_throttle = 0.4
    })
    patrol
    truck.set_on_wrecked(fn() { wreck_delay.play() })
    fleet.place = place fleet.motion = motion fleet.truck = truck fleet.patrol = patrol
    fleet.chassis = motion.query("#pickup_chassis_frame")
    truck_ride.chassis = fleet.chassis
}
spawn_truck()
let chassis = fleet.chassis

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
// Kills: yours count the NPC going down, the NPC's count you going down.
let kills = { player = 0 npc = 0 }
let player_kills = Text { name = "demo_player_kills" "PLAYER\n0" C.rgba(0.30, 0.85, 1.0, 1.0) Unlit {} }
let npc_kills = Text { name = "demo_npc_kills" "NPC\n0" C.rgba(1.0, 0.45, 0.62, 1.0) Unlit {} }
// Left of the spawn, raised and turned toward it.
T.position(-12.0, 3.2, -2.0).rotation(0.0, Math.pi / 4.0, 0.0) {
    name = "demo_kill_board"
    Raycastable.disabled()
    T.position(0.0, 0.0, -0.06).scale(6.4, 2.2, 0.1) {
        R.cube() { hardware C.rgba(0.10, 0.11, 0.16, 1.0) }
    }
    T.position(-2.6, 0.55, 0.02).scale(0.5, 0.5, 0.5) { player_kills }
    T.position(0.6, 0.55, 0.02).scale(0.5, 0.5, 0.5) { npc_kills }
}
let player = practice_health(local.health, local.shield_mount, {
    on_damage = sounds.damage
    on_death = fn() {
        kills.npc = kills.npc + 1
        npc_kills.set_text("NPC\n" + kills.npc)
    }
})
fn can_fire(user) { return player.alive() }

// Seated in the driver's seat (chassis space): the head lands near the
// driver's eye point. The script drives; Mitten only aims and fires, and
// climbs back in after going down.
let target = practice_target({
    prefix = "npc" avatar = "mitten" use_xr = use_xr use_desktop = use_desktop
    position = [-0.33, 0.20, 0.30] yaw = 0.0 strafe = 0.0 seated = true
    // Shot down, she's thrown out beside the driver's door; once back up she
    // walks to the door (chasing the truck if it's moving) and gets back in.
    vehicle = truck_ride
    health_view = local.health_view on_damage = sounds.damage
    on_down = fn() {
        kills.player = kills.player + 1
        player_kills.set_text("PLAYER\n" + kills.player)
    }
    player = local.shield_mount
    on_player_hit = fn(points) { player.damage(points, 1.0) }
})
chassis.attach(target.root)

// The new truck takes over: Mitten moves to it (into its seat if she's
// seated), and the wreck's smoke stops with it.
on(wreck_events, "DataEvent", fn(event) {
    if event != "TruckRebuilt" { return }
    let old = { place = fleet.place truck = fleet.truck patrol = fleet.patrol }
    spawn_truck()
    target.set_vehicle_chassis(fleet.chassis)
    old.truck.repair_damage()
    old.patrol.remove_subtree()
    old.place.remove_subtree()
})

fn weapon_options(id) {
    return { weapon_id = id opaque_bucket = "near" can_fire = can_fire
        left_pointer = "#local_left_pointer" right_pointer = "#local_right_pointer" user = "#local_avatar" }
}
fn make_pistol(index) {
    let options = weapon_options("demo.pistol." + index)
    options.ammunition = 240
    options.on_hit = target.pistol_hit
    return ship_machine_pistol("demo_machine_pistol_" + index, options)
}
let pistols = [make_pistol(1), make_pistol(2), make_pistol(3)]
T.position(-2.2, 0.0, -1.4) {
    name = "demo_pistol_rack"
    box([0.0, 0.48, 0.0], [2.5, 0.96, 0.75], pink)
    for slot in range(3) {
        T.position((slot - 1.0) * 0.84, 1.04, 0.0) { pistols[slot] }
    }
}
let launcher_options = weapon_options("demo.sta.1")
launcher_options.on_explosion = fn(point) {
    // Until rockets have their own explosion sound, use the damage cue.
    sounds.damage(1.0)
    target.rocket_splash(point)
    let blast = player.blast_at(point)
    if blast { player.damage(blast.health, 1.0) }
}
let launcher = shared_sta_launcher("demo_sta_launcher", launcher_options)
T.position(1.6, 0.0, -1.4) {
    T.position(0.0, 1.04, 0.0) { launcher.root }
    box([0.0, 0.48, 0.0], [0.55, 0.96, 0.55], pink)
}
T.position(2.7, 0.0, -1.6) { sta_ammo_cabinet("demo_sta_ammo_cabinet") }
ship_input_controller(false, [launcher.grip], pistols)

// The mech, parked left of the spawn and facing the road. Its laser hits
// count as pistol hits on Mitten and its rockets as rocket splash; both cut
// the truck like any weapon.
let mech = null
if use_xr {
    mech = display_car_xr("demo_mech", [-22.0, 0.1, 3.0], 0.0, "demo_mech_mount", query("#local_xr_locomotion"))
} else {
    mech = display_car_desktop("demo_mech", [-22.0, 0.1, 3.0], 0.0, "demo_mech_mount")
}
mech
let mech_body = mech.query("#demo_mech")
mech_body.on_laser_hit = target.pistol_hit
// Shot to pieces, the mech smokes, burns and is wrecked (its driver thrown
// out); twenty seconds later it comes back repaired at the next of three
// parking spots along the range, facing the road.
let mech_events = T { name = "demo_mech_events" }
mech_events
let mech_repair = Animation.paused().length(40.0) {
    Keyframe.at(0.0) {}
    Keyframe.at(40.0) { emit_data(mech_events, "MechRepaired") }
}
mech_events.attach(mech_repair)
let mech_spots = [[-22.0, 0.1, 3.0], [22.0, 0.1, 3.0], [-36.0, 0.1, 3.0]]
let mech_respawns = { count = 0 }
on(mech_events, "DataEvent", fn(event) {
    if event != "MechRepaired" { return }
    mech_body.repair_damage()
    mech_respawns.count = mech_respawns.count + 1
    let spot = mech_spots[mech_respawns.count % 3]
    mech.reset()
    mech_body.update_transform(spot, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
})
mech_body.on_wrecked = fn() { mech_repair.play() }
mech_body.on_rocket_hit = fn(hit) {
    sounds.damage(1.0)
    target.rocket_splash(hit.hit_point)
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

let dock_panels = T { name = "demo_dock_panels" }
dock_panels
let dock_items = {
    avatar = { id = "avatar" label = "Avatar" root_name = "demo_avatar_picker"
        position = [0.0, 2.0, -2.2]
        build = fn() {
            let picker = avatar_picker_panel({
                root_name = "demo_avatar_picker"
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
    input = { id = "input" label = "Input" root_name = "demo_input_settings"
        position = [1.2, 2.3, -2.2]
        build = fn() { return input_settings_panel({ root_name = "demo_input_settings" }) } }
    humanoid = { id = "humanoid" label = "Humanoid" root_name = "demo_avatar_settings"
        position = [-4.65, 2.3, -2.2]
        build = fn() {
            return avatar_settings_panel({
                root_name = "demo_avatar_settings"
                show_xr = use_xr
                calibration_file = xr_calibration_file
                on_height_recommitted = apply_avatar_height
                camera_view = local.camera_view
            })
        } }
    graphics = { id = "graphics" label = "Graphics" root_name = "demo_graphics_settings"
        position = [-3.0, 2.3, -4.0]
        build = fn() { return graphics_settings_panel({ root_name = "demo_graphics_settings" }) } }
}
T.position(-0.8, 1.6, -1.65) {
    dock({
        root_name = "demo_dock"
        mount = dock_panels
        groups = [[dock_items.avatar], [dock_items.input, dock_items.humanoid, dock_items.graphics]]
    })
}
let dock_slot = T.position(0.35, 1.25, -0.45).rotation(0.0, -0.66, 0.0) {
    dock_dropzone({ name = "demo_dock_slot" })
}
local.root.attach(dock_slot)
if use_desktop { local.watch_pointer(query("[name='local_desktop_pointer']"), "desktop") }
if use_xr {
    local.watch_pointer(query("[name='local_left_pointer']"), "left")
    local.watch_pointer(query("[name='local_right_pointer']"), "right")
}
