// View flags: XR-only by default. Set use_desktop = true (and use_xr = false)
// for a desktop window. Network role is separate (commands below).
let use_xr = true
let use_desktop = false
// Starting avatars also serve as the peer's fallback if an optional model is unavailable.
// Supported: "mitten", "lumae".
let host_avatar = "mitten"
let client_avatar = "mitten"

import { f32, str, vec3, vec4 } from "types"

import { gohei_black_spring } from "../assets/components/paper_craft/gohei_black_spring.mms"
import { shared_grabbable } from "../assets/components/network/shared_grabbable.mms"
import { shared_mounts } from "../assets/components/network/shared_mounts.mms"
import { controller_hand_curls } from "../assets/components/network/controller_hand_curls.mms"
import { make_rooftop_player } from "../assets/components/characters/rooftop_player.mms"
import { avatar_picker_panel } from "../assets/components/panels/avatar_picker_panel.mms"
import { graphics_settings_panel } from "../assets/components/panels/graphics_settings_panel.mms"
import { avatar_settings_panel } from "../assets/components/panels/avatar_settings_panel.mms"
import { input_settings_panel } from "../assets/components/panels/input_settings_panel.mms"
import { mirror_panel } from "../assets/components/panels/mirror_panel.mms"
import { dock, dock_dropzone } from "../assets/components/ui/dock.mms"
import { pool_tile_material } from "../assets/components/materials/pool_tiles.mms"
import { pool_building_with_openings, pool_building_with_stairs } from "../assets/components/buildings/pool_building.mms"
import { round_aperture_wall } from "../assets/components/buildings/round_aperture.mms"
import { train_station } from "../assets/components/trains/train_station.mms"
import { railroad_arc, railroad_walkway } from "../assets/components/trains/railroad_track.mms"
import { boxcar, boxcar_sensor, passenger_mount } from "../assets/components/trains/boxcar.mms"
import { pickup_truck } from "../assets/components/vehicles/pickup_truck.mms"
import { ship_input_controller } from "../assets/components/ship_input_controller.mms"
import { shared_sta_launcher } from "../assets/components/weapons/shared_sta_launcher.mms"
import { sta_ammo_cabinet } from "../assets/components/sta_ammo_cabinet.mms"
import { ship_machine_pistol } from "../assets/components/weapons/ship_machine_pistol.mms"
import { rooftop_dusk_sky } from "../assets/components/backgrounds/rooftop_dusk_sky.mms"

// Two-player LAN/loopback rooftop: the multiplayer-basic baseline (typed pose
// channels, shared cube and gohei) on the main building's roof. Below it, a
// train runs a crescent of track through all three buildings and reverses on
// ZoneContact (see the train section). Without a second
// player the session waits as host and the scene runs normally.
//
//   MITTENS_MULTIPLAYER_ROLE=host   MITTENS_MULTIPLAYER_NAME=rei cargo run --release -- load examples/twilight_rooftop.mms
//   MITTENS_MULTIPLAYER_ROLE=client MITTENS_MULTIPLAYER_NAME=dan cargo run --release -- load examples/twilight_rooftop.mms
//
// Across machines: the host binds 0.0.0.0:7010 by default; the client sets
// MITTENS_MULTIPLAYER_ADDR=<host-ip>:7010 (numeric address, UDP port open on the host).
// Desktop: WASD + right-drag to move/look. XR: sticks move, B jumps,
// double-tap B to toggle flight, and hold B/A to rise/descend in flight.
// The Panels dock beside the spawn opens the avatar picker and the mirror,
// input, humanoid (avatar settings) and graphics panels; Input switches the
// left stick between strafing and turning. Collapse it from its title bar,
// or grab the title bar into the slot in front of you to carry it along.
// XR-only mode creates no winit window or Camera3D.

// Local voice drives the local avatar's mouth. Stays neutral without a
// microphone. Client side only: mouth and fingers are not sent to peers yet.
let microphone = AudioInput {}
let voice_level = Amplitude.rolling_window(0.080).highpass(120.0).highpass_resonance(0.707).from(microphone) {}
let mouth_tuning = { center_rms = 0.0475 range_rms = 0.085 amount = 1.0 }

let net = MultiplayerSession.from_env() {}
net

RendererSettings { window_visible(use_desktop) window_size(1280, 800) }
// Eye/blink clip timestamps below use seconds converted to beats.
Clock.bpm(120.0)
BGC.rgba(0.68, 0.56, 0.84, 1.0)
RenderGraph {
    EmissivePass {}
    Bloom { intensity(0.65) radius_ndc(0.035) emissive_scale(1.0) half_res(true) }
}
Raycastable.disabled() {
    Selectable.off()
    rooftop_dusk_sky(false)
}
// Warm low light from the beige horizon, with a cool lilac fill from above.
AL.rgb(0.26, 0.24, 0.32)
T.position(-0.70, 0.35, -0.60) { DL.color(1.0, 0.86, 0.70).intensity(1.3) }
T.position(0.5, 0.8, 0.3) { DL.color(0.70, 0.66, 1.0).intensity(0.4) }

// City: paper-craft's three buildings, grounded, each one floor shorter.
let storey_height = 3.2
let train_storey_height = 3.8
// Keep the main rooftop at 16 m and raise the balcony above the train.
let main_floors = [storey_height, storey_height, train_storey_height, 2.6, storey_height]
let roof_height = 0.0
for height in main_floors { roof_height = roof_height + height }
let roof_x = -18.0
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
let white = [0.94, 0.96, 0.97]
let pink = [0.88, 0.65, 0.73]
let blue = [0.64, 0.78, 0.86]

fn box(position, size, color) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        R.cube() { hardware C.rgba(color[0], color[1], color[2], 1.0) }
        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.85) {} }
    }
}

pool_building_with_stairs("twilight_rooftop_main", [roof_x, 0.0, 0.0], main_floors, 1.0, tiles, pink)
T { name = "twilight_rooftop_ground" box([0.0, -0.48, -13.0], [120.0, 0.5, 120.0], [0.40, 0.31, 0.32]) }

// Train: a crescent of track on the third storey (floor slab y = 6.4), a
// semicircle of radius 13 m around (-11, -13). It starts at the main
// building's balcony railing, passes through rectangular openings in the east
// building, and reverses at the north building's round hole. The opening clears
// the car in height and width, and the car's corners meet the ring.
let quarter_turn = 1.57079633
let floor_y = 2.0 * storey_height
let rail_top = floor_y + 0.28
let arc_center = [-11.0, -13.0]
let arc_radius = 13.0
let car_length = 4.0
let car_width = 1.8
let car_height = 2.85
let car_gap = 0.4
let car_doors = [1.50, 2.25]
let opening_width = 3.8
let opening_height = 3.35
let ring_width = 3.9
let ring_radius = 1.625
// The main balcony railing's outer face (x = -9.08) to the north facade (x = -11).
let track_start = -1.4225
let track_end = quarter_turn
// The extra facade width accounts for the curved approach and the open leaves.
let opening_center = -1.40
let east_floors = [storey_height, storey_height, train_storey_height]
let north_floors = [storey_height, storey_height, train_storey_height, storey_height]
pool_building_with_openings("twilight_rooftop_north", [-18.0, 0.0, -26.0], north_floors, -1.0, tiles, white,
    [{ floor = 2 axis = "x" side = 1.0 center = -0.1 width = ring_width height = opening_height }])
pool_building_with_openings("twilight_rooftop_east", [2.0, 0.0, -13.0], east_floors, 1.0, tiles, blue, [
    { floor = 2 axis = "z" side = 1.0 center = opening_center width = opening_width height = opening_height },
    { floor = 2 axis = "z" side = -1.0 center = opening_center width = opening_width height = opening_height },
])
T.position(-11.0, floor_y, -26.1).rotation(0.0, quarter_turn, 0.0) {
    name = "twilight_rooftop_round_hole"
    round_aperture_wall("twilight_rooftop_round_hole_wall", rail_top - floor_y + car_height / 2.0,
        ring_radius, ring_width / 2.0, opening_height, 0.24, 24, white, tiles)
}
T.position(arc_center[0], 0.0, arc_center[1]) {
    name = "twilight_rooftop_rails"
    railroad_walkway(arc_radius, track_start, track_end, rail_top, hardware)
    railroad_arc("twilight_rooftop_rail_arc", arc_radius, track_start, track_end, 1.40, 0.85,
        rail_top, [0.30, 0.20, 0.12], [0.36, 0.38, 0.44], hardware)
}

// The pivot sits at the arc center and yaws; each car rides the arc at its own
// angle, so the cars follow the curve instead of hanging off it as one chord.
let train_speed = 0.35 // rad/s, about 4.6 m/s on the arc
let car_angle = (car_length + car_gap) / 2.0 / arc_radius
let train_pivot = T.position(arc_center[0], rail_top, arc_center[1]).rotation(0.0, -0.6, 0.0) {
    name = "twilight_rooftop_train"
}
let train_motion = Velocity { name = "twilight_rooftop_train_motion" train_pivot }
for car in [0, 1] {
    let label = "twilight_rooftop_train_car_" + car
    let car_mount = T.rotation(0.0, (car * 2.0 - 1.0) * car_angle, 0.0) {
        T.position(arc_radius, 0.0, 0.0) {
            let shell = boxcar(label, [0.0, 0.0, 0.0], car_length, car_width, car_height,
                [0.55, 0.20, 0.22], [0.18, 0.20, 0.25], [0.68, 0.40, 0.24], hardware, true, car_doors)
            passenger_mount(label, shell, car_width, car_doors)
            boxcar_sensor(label + "_sensor", car_length, car_width, car_height)
        }
    }
    train_pivot.attach(car_mount)
}
train_motion
let station = train_station(train_pivot, train_motion, train_speed, arc_center[1], net.is_host())
// Third storey of the east building, across the walkway from the outer doors.
T.position(5.2, floor_y, -13.0).rotation(0.0, -quarter_turn, 0.0) { station.root }

// Status board.
let status = Text { "connecting..." C.rgba(0.16, 0.17, 0.22, 1.0) }
T.position(roof_x - 2.6, roof_height + 3.4, -4.30).scale(0.22, 0.22, 0.22) { status }
let roster = { count = 0 }
fn show_status(line) {
    let role = "client"
    if net.is_host() { role = "host" }
    status.set_text(net.local_name() + " (" + role + ", peer " + net.peer_id() + ")\nremote players: " + roster.count + "\n" + line)
}



// Whole transforms travel through typed channels; every machine solves its own rigs.
let channel_port = 7011
let remote_port = 7012
if !net.is_host() { channel_port = 7012 remote_port = 7011 }
let socket = UdpSocket.bind("0.0.0.0:" + channel_port) {}
socket
let pose_type = { translation: vec3, rotation_quat_xyzw: vec4, scale: vec3 }
let grab_rank = 2
if net.is_host() { grab_rank = 1 }
// Use the imported hood's yellow base color for the combined paint.
// Shared items mounted on these frames (the truck bed) sync by one event each.
let mounts = shared_mounts()
let pickup = pickup_truck([1.0, 0.606, 0.047], null, true, {
    socket = socket vehicle_id = "arena.pickup.1" rank = grab_rank mounts = mounts bed_mount = 1 })
T.position(roof_x + 4.0, roof_height, 0.0) { name = "twilight_rooftop_pickup" pickup }
fn weapon_options(id) {
    return { socket = socket weapon_id = id rank = grab_rank mounts = mounts
        left_pointer = "#local_left_pointer" right_pointer = "#local_right_pointer" user = "#local_avatar" }
}
let pistol = ship_machine_pistol("rooftop_machine_pistol", weapon_options("arena.pistol.1"))
let second_pistol = ship_machine_pistol("rooftop_machine_pistol_2", weapon_options("arena.pistol.2"))
let launcher = shared_sta_launcher("rooftop_sta_launcher", weapon_options("arena.sta.1"))
fn weapon_stand(x, weapon) {
    return T.position(x, roof_height, -1.1) {
        T.position(0.0, 1.04, 0.0) { weapon }
        box([0.0, 0.48, 0.0], [0.55, 0.96, 0.55], pink)
    }
}
weapon_stand(roof_x - 0.25, pistol)
weapon_stand(roof_x + 0.65, second_pistol)
weapon_stand(roof_x + 1.55, launcher.root)
T.position(roof_x + 2.65, roof_height, -1.55) {
    sta_ammo_cabinet("twilight_sta_ammo_cabinet")
}
ship_input_controller(false, [pistol.grip, second_pistol.grip, launcher.grip])

let local_avatar = client_avatar
let remote_avatar = host_avatar
if net.is_host() { local_avatar = host_avatar remote_avatar = client_avatar }
let local = make_rooftop_player({ prefix = "local" local_player = true avatar = local_avatar use_xr = use_xr use_desktop = use_desktop voice_level = voice_level mouth_tuning = mouth_tuning })
let remote = make_rooftop_player({ prefix = "remote" local_player = false avatar = remote_avatar use_xr = use_xr use_desktop = use_desktop })
// The picker opens from the dock; its status line outlives each opening.
let picker_status = { text = local_avatar + " is active" }
fn report_avatar_status(message) {
    picker_status.text = message
    let line = query("#avatar_picker_status")
    if line { line.set_text(message) }
}
fn select_local_avatar(choice) {
    local.select_avatar(choice, report_avatar_status)
}
local.label.set_text(net.local_name())
let spawn_x = -1.5
if !net.is_host() { spawn_x = 1.5 }
local.root.update_transform([roof_x + spawn_x, roof_height, 0.0])
remote.root.update_transform([roof_x - spawn_x, roof_height, 0.0])
if use_desktop && !use_xr {
    I.speed(2.0) {
        name = "local_desktop_locomotion"
        InputTransformMode.forward_z() { fps_rotation() roll_axis_y() }
        local.root
    }
} else { local.locomotion }
if use_xr { XR.on() } else { XR.off() }
remote.root
let input_settings_file = null
if use_xr {
    input_settings_file = InputSettings.xr.default_file()
    InputSettings.xr.set_stick_mode(InputSettings.xr.stick_mode({ file = input_settings_file }))
}
let xr_calibration_file = null
if use_xr {
    xr_calibration_file = XrSettings.floor.default_file()
    XrSettings.floor.set_correction_meters(XrSettings.floor.correction_meters({file = xr_calibration_file}))
}
fn apply_avatar_height() { local.apply_desired_height(report_avatar_status) }
on_global("DataEvent", fn(event) {
    if event == "AvatarSettingsChanged" { apply_avatar_height() }
})
// The dock opens each panel on demand at its world placement. Grab its title
// bar into the slot in front of the player to carry it along.
let dock_panels = T { name = "twilight_rooftop_dock_panels" }
dock_panels
let dock_items = {
    avatar = { id = "avatar" label = "Avatar" root_name = "twilight_rooftop_avatar_picker"
        position = [roof_x + spawn_x, roof_height + 2.0, -2.2]
        build = fn() {
            let picker = avatar_picker_panel({
                root_name = "twilight_rooftop_avatar_picker"
                status = picker_status.text
                choices = [
                    { id = "mitten" label = "Mitten" },
                    { id = "lumae" label = "Lumae" },
                    { id = "miku" label = "Miku" },
                    { id = "succubus" label = "Succubus" },
                ]
                on_select = select_local_avatar
            })
            return T.position(-picker.width / 2.0, 0.0, 0.0) { picker.root }
        } }
    // The roof faces west over open air, so the reflected side stays clear.
    mirror = { id = "mirror" label = "Mirror" root_name = "twilight_rooftop_mirror_frame"
        position = [roof_x - 7.6, roof_height + 1.3, 0.0] rotation = [0.0, quarter_turn, 0.0]
        build = fn() { return mirror_panel({ name = "twilight_rooftop_mirror" width = 3.0 height = 1.4 quality = 1024 }) } }
    input = { id = "input" label = "Input" root_name = "twilight_rooftop_input_settings"
        position = [roof_x + spawn_x + 1.2, roof_height + 2.3, -2.2]
        build = fn() { return input_settings_panel({ root_name = "twilight_rooftop_input_settings" settings_file = input_settings_file }) } }
    humanoid = { id = "humanoid" label = "Humanoid" root_name = "twilight_rooftop_avatar_settings"
        position = [roof_x + spawn_x - 4.65, roof_height + 2.3, -2.2]
        build = fn() {
            return avatar_settings_panel({
                root_name = "twilight_rooftop_avatar_settings"
                show_xr = use_xr
                calibration_file = xr_calibration_file
                on_height_recommitted = apply_avatar_height
                camera_view = local.camera_view
            })
        } }
    graphics = { id = "graphics" label = "Graphics" root_name = "twilight_rooftop_graphics_settings"
        position = [roof_x - 4.6, roof_height + 2.3, -4.8]
        build = fn() { return graphics_settings_panel({ root_name = "twilight_rooftop_graphics_settings" }) } }
}
T.position(roof_x + spawn_x + 1.0, roof_height + 1.6, -1.6) {
    dock({
        root_name = "twilight_rooftop_dock"
        mount = dock_panels
        groups = [
            [dock_items.avatar, dock_items.mirror],
            [dock_items.input, dock_items.humanoid, dock_items.graphics],
        ]
    })
}
// Right of and in front of the player, turned to face them.
let dock_slot = T.position(0.35, 1.25, -0.45).rotation(0.0, -0.66, 0.0) {
    dock_dropzone({ name = "twilight_rooftop_dock_slot" })
}
local.root.attach(dock_slot)
// Grab this cyan cube with the desktop pointer or either XR hand. The latest
// grab/release transition wins; simultaneous claims favor the client in this demo.
let shared_prop = T.position(roof_x, roof_height + 1.1, 0.0) {
    name = "shared_prop"
    Grabbable.to_hand(true) {}
    T.scale(0.25, 0.25, 0.25) {
        R.cube() { C.rgba(0.12, 0.85, 0.95, 1.0) Raycastable.enabled() }
    }
}
let shared = shared_grabbable({ socket = socket name = "arena.prop.1" target = shared_prop rank = grab_rank mounts = mounts })
shared.root
let gohei = gohei_black_spring("shared_gohei")
gohei.update_transform([roof_x + 1.1, roof_height + 1.1, 0.0])
let shared_gohei = shared_grabbable({ socket = socket name = "arena.gohei.1" target = gohei rank = grab_rank mounts = mounts })
shared_gohei.root
let shared_objects = [shared, shared_gohei, pistol, second_pistol, launcher]
for item in shared_objects {
    if use_desktop { item.bind_pointer(query("[name='local_desktop_pointer']")) }
    if use_xr {
        item.bind_pointer(query("[name='local_left_pointer']"))
        item.bind_pointer(query("[name='local_right_pointer']"))
    }
}
if use_desktop { local.watch_pointer(query("[name='local_desktop_pointer']"), "desktop") }
if use_xr {
    local.watch_pointer(query("[name='local_left_pointer']"), "left")
    local.watch_pointer(query("[name='local_right_pointer']"), "right")
}
let links = { root = null head = null left = null right = null blink_left = null blink_right = null left_valid = null right_valid = null avatar = null }
let channel_state = { ticks = 0.0 send_elapsed = 0.0 sent_avatar = null avatar_repeats = 0 }
let connection = { root = null avatar_group = null address = null peer_avatar = null
    avatar_retry = null avatar_retry_elapsed = 0.0 avatar_retries = 0 }
let hand_curls = controller_hand_curls({
    socket = socket xr = use_xr input = query("#local_xr_locomotion")
    left_driver = local.left_driver right_driver = local.right_driver
})
let channel_groups = T { name = "channel_groups" }
channel_groups

fn publish(name, address, kind) {
    let channel = socket.publish(name).to(address).type(kind)
    connection.root.attach(channel)
    return channel
}
fn receive_transform(group, name, address, target, space) {
    let channel = socket.subscribe(name).from(address).type(pose_type)
    let configured = channel.bind_transform(target, { space = space })
    group.attach(configured)
}

fn bind_peer_avatar(head, model, choice) {
    if !connection.root { return }
    hand_curls.bind_model(model, choice)
    if connection.avatar_group { connection.avatar_group.remove_subtree() }
    let group = T { name = "peer_avatar_channels" }
    connection.root.attach(group)
    connection.avatar_group = group
    let address = connection.address
    receive_transform(group, "head", address, head, "local")
    if choice != "miku" {
        let blink_left = socket.subscribe("blink.left").from(address).type(f32)
        group.attach(blink_left.bind_morph(model, "left_eye_blink"))
        let blink_right = socket.subscribe("blink.right").from(address).type(f32)
        group.attach(blink_right.bind_morph(model, "right_eye_blink"))
    }
}
remote.on_avatar_changed(bind_peer_avatar)

// Avatar choices are events: each peer sends its choice when the other joins
// and when it switches, nothing in between. A failed or timed-out load keeps
// the current remote avatar and retries the same choice locally, since no
// further sample will arrive until the peer switches again.
let avatar_retry_delay = 2.0
let avatar_retry_limit = 3
fn receive_avatar_status(message, success) {
    if success != false || connection.avatar_retries >= avatar_retry_limit { return }
    connection.avatar_retries = connection.avatar_retries + 1
    connection.avatar_retry = connection.peer_avatar
    connection.avatar_retry_elapsed = 0.0
}

on(net, "MultiplayerConnected", fn(event) { show_status("connected; waiting for typed channels") })
on(net, "MultiplayerPeerJoined", fn(event) {
    remote.label.set_text(event.name)
    let ip = net.peer_ip(event.peer_id)
    if !ip { return }
    if connection.root { connection.root.remove_subtree() }
    let group = T { name = "peer_channels" }
    channel_groups.attach(group)
    connection.root = group
    let address = ip + ":" + remote_port
    connection.address = address
    connection.avatar_group = null
    connection.peer_avatar = null
    connection.avatar_retry = null
    connection.avatar_retries = 0
    channel_state.sent_avatar = null
    for item in shared_objects { item.connect(group, address) }
    station.connect(socket, group, address)
    pickup.connect(group, address)
    hand_curls.connect(group, address)
    links.root = publish("root", address, pose_type)
    links.head = publish("head", address, pose_type)
    links.left = publish("left-hand", address, pose_type)
    links.right = publish("right-hand", address, pose_type)
    links.blink_left = publish("blink.left", address, f32)
    links.blink_right = publish("blink.right", address, f32)
    links.left_valid = publish("left-hand.valid", address, f32)
    links.right_valid = publish("right-hand.valid", address, f32)
    links.avatar = publish("avatar", address, str)
    let avatar_channel = socket.subscribe("avatar").from(address).type(str)
    group.attach(avatar_channel)
    on(avatar_channel, "UdpSample", fn(sample) {
        // Each choice arrives as a short burst; apply it once.
        if sample.value == connection.peer_avatar { return }
        connection.peer_avatar = sample.value
        connection.avatar_retry = null
        connection.avatar_retries = 0
        remote.select_avatar(sample.value, receive_avatar_status)
    })
    receive_transform(group, "root", address, remote.root, "world")
    receive_transform(group, "left-hand", address, remote.left, "local")
    receive_transform(group, "right-hand", address, remote.right, "local")
    let left_valid = socket.subscribe("left-hand.valid").from(address).type(f32)
    connection.root.attach(left_valid)
    on(left_valid, "UdpSample", fn(sample) { remote.left_driver.set_pose_valid(sample.value > 0.5) })
    let right_valid = socket.subscribe("right-hand.valid").from(address).type(f32)
    connection.root.attach(right_valid)
    on(right_valid, "UdpSample", fn(sample) { remote.right_driver.set_pose_valid(sample.value > 0.5) })
    remote.bind_avatar()
    roster.count = 1
    show_status(event.name + " joined; typed pose and face channels")
})
on(net, "MultiplayerPeerLeft", fn(event) {
    for item in shared_objects { item.disconnect() }
    station.disconnect()
    pickup.disconnect()
    hand_curls.disconnect()
    remote.cancel_avatar_load()
    connection.avatar_retry = null
    if connection.root { connection.root.remove_subtree() connection.root = null }
    links.root = null
    links.avatar = null
    roster.count = 0
    show_status(event.name + " left; holding last received pose")
})
on(net, "MultiplayerError", fn(event) { show_status("session " + event.phase + ": " + event.message) })
on_global("UdpState", fn(event) { if event.message != "" { show_status("channel: " + event.message) } })
on_global("FrameTick", fn(event) {
    station.tick(event.dt_sec)
    channel_state.ticks = channel_state.ticks + event.dt_sec
    if !use_xr {
        // Scripted hand targets exercise receiving-side arm IK without tracking hardware.
        local.left_driver.set_pose_valid(true)
        local.right_driver.set_pose_valid(true)
        let wave = Math.sin(channel_state.ticks * 2.0) * 0.15
        local.left.update_transform([-0.35, -0.35 + wave, -0.25])
        local.right.update_transform([0.35, -0.35 - wave, -0.25])
    }
    if connection.avatar_retry {
        connection.avatar_retry_elapsed = connection.avatar_retry_elapsed + event.dt_sec
        if connection.avatar_retry_elapsed >= avatar_retry_delay {
            let retry_choice = connection.avatar_retry
            connection.avatar_retry = null
            remote.select_avatar(retry_choice, receive_avatar_status)
        }
    }
    channel_state.send_elapsed = channel_state.send_elapsed + event.dt_sec
    if channel_state.send_elapsed < 0.05 { return }
    hand_curls.tick(channel_state.send_elapsed)
    channel_state.send_elapsed = 0.0
    // A switch (or a join, which clears sent_avatar) starts a short burst so
    // one lost packet cannot drop the event. Repeats wait for the channel.
    let choice = local.avatar_id()
    if choice != channel_state.sent_avatar {
        channel_state.sent_avatar = choice
        channel_state.avatar_repeats = 4
    }
    if channel_state.avatar_repeats > 0 && links.avatar && links.avatar.ready() {
        links.avatar.write(choice)
        channel_state.avatar_repeats = channel_state.avatar_repeats - 1
    }
    if links.root && links.root.ready() {
        links.root.write(local.root.world_trs())
        if links.head.ready() { links.head.write(local.head_pose()) }
        if links.left.ready() { links.left.write(local.left.trs()) }
        if links.right.ready() { links.right.write(local.right.trs()) }
        if links.blink_left.ready() { links.blink_left.write(local.blink("left_eye_blink")) }
        if links.blink_right.ready() { links.blink_right.write(local.blink("right_eye_blink")) }
        let left_tracked = 0.0
        let right_tracked = 0.0
        if local.left_driver.pose_valid() { left_tracked = 1.0 }
        if local.right_driver.pose_valid() { right_tracked = 1.0 }
        if links.left_valid.ready() { links.left_valid.write(left_tracked) }
        if links.right_valid.ready() { links.right_valid.write(right_tracked) }
    }
})
