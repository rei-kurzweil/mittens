// XR-only rooftop hangar. Run solo (host by default):
//   cargo run --release -- load examples/interceptor.mms
// For a second player, set MITTENS_MULTIPLAYER_ROLE=client and
// MITTENS_MULTIPLAYER_ADDR=<host IPv4>:7010. Poses use UDP 7011/7012.

import { f32, str, vec3, vec4 } from "types"
import { star_kawaii_background } from "../assets/components/backgrounds/star_kawaii_background.mms"
import { sky_cloud_banks } from "../assets/components/backgrounds/clouds.mms"
import { emissive_surface } from "../assets/components/materials/emissive_surface.mms"
import { truss } from "../assets/components/truss.mms"
import { fall_respawn } from "../assets/components/fall_respawn.mms"
import { make_rooftop_player } from "../assets/components/characters/rooftop_player.mms"
import { avatar_picker_panel } from "../assets/components/panels/avatar_picker_panel.mms"
import { graphics_settings_panel } from "../assets/components/panels/graphics_settings_panel.mms"
import { input_settings_panel } from "../assets/components/panels/input_settings_panel.mms"
import { avatar_settings_panel } from "../assets/components/panels/avatar_settings_panel.mms"
import { mirror_panel } from "../assets/components/panels/mirror_panel.mms"
import { dock, dock_dropzone } from "../assets/components/ui/dock.mms"

RendererSettings { window_visible(false) }
XR.on()
Clock.bpm(120.0)
BGC.rgba(0.009, 0.004, 0.007, 1.0)
RenderGraph {
    EmissivePass {}
    Bloom { intensity(0.53) radius_ndc(0.023) emissive_scale(1.0) half_res(true) }
}

let glow = emissive_surface()
let sky = Material(MaterialDefinition(MaterialBase())
    .fragment("assets/shaders/interceptor-night-sky.frag")
    .inputs({ horizon_color: vec3, moon_radius: f32, zenith_color: vec3, moon_direction: vec3 }))
    .inputs({ horizon_color = [0.28, 0.15, 0.39], moon_radius = 0.029,
        zenith_color = [0.009, 0.004, 0.007], moon_direction = [0.18, 0.15, -1.0] })
Raycastable.disabled() {
    Selectable.off()
    BG {
        name = "interceptor_night_sky"
        T.scale(160.0, 160.0, 160.0) { R.sphere() { sky } }
    }
    BG.occlusion_and_lighting() {
        name = "interceptor_stars_and_clouds"
        T { glow star_kawaii_background([0.57, 0.42, 0.23, 1.0], { scale = 0.75 }) }
        sky_cloud_banks([
            [-35.0, 8.0, -51.0, 3.7, 0.55, 5],
            [26.0, 12.0, -55.0, 3.3, -0.36, 5],
            [54.0, 17.0, -18.0, 3.1, -1.22, 3],
            [27.0, 9.0, 47.0, 4.1, 0.61, 5],
            [-42.0, 12.0, 30.0, 3.6, -0.98, 3],
        ], [0.047, 0.026, 0.064]).root
    }
}

AL.rgb(0.18, 0.16, 0.22)
T.position(0.18, 0.38, -1.0) { DL.color(0.76, 0.74, 1.0).intensity(0.36) }

let far_anime = Material(MaterialDefinition.anime().opaque_bucket("far")).inputs({
    shade_color = [0.28, 0.24, 0.35] shade_strength = 0.65 rim_strength = 0.016
})
let hangar_anime = Material(MaterialDefinition.anime()).inputs({
    shade_color = [0.37, 0.34, 0.48] shade_strength = 0.56 rim_strength = 0.12
    rim_color = [0.82, 0.72, 1.0]
})
let hull_anime = Material(MaterialDefinition.anime().opaque_bucket("near")).inputs({
    shade_color = [0.36, 0.34, 0.50] shade_strength = 0.53 rim_strength = 0.24
    rim_color = [0.80, 0.78, 1.0]
})

fn box(position, size, color, material, solid) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        R.cube() { material C.rgba(color[0], color[1], color[2], 1.0) }
        if solid { Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.85) {} } }
    }
}

// The skyline is ordinary world geometry. Each wing of the city batches its
// tower shells and its windows by tint. The nearest facade is over 50 m from
// either hangar entrance.
let tower_count = 12
fn tower(side, index) {
    let tier = 0
    if index >= 7 { tier = 1 index = index - 7 }
    let seed = side * 29 + tier * 13 + index
    return {
        x = (index - (3 - tier)) * (18.0 + 7.0 * tier) + ((seed * 3) % 5 - 2)
        z = -78.0 - 38.0 * tier - (seed * 7) % 10
        width = 10.0 + (seed * 3) % 7
        depth = 9.0 + (seed * 5) % 7
        height = 23.0 + (seed * 11) % 23 + tier * 6.0
        seed = seed
    }
}

let window_colors = [[0.86, 0.64, 0.40], [0.39, 0.70, 0.85], [0.83, 0.43, 0.67]]
fn window(position, size, color, strength) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        R.cube() { C.rgba(color[0], color[1], color[2], 1.0) EM.on().intensity(strength) }
    }
}

fn facade_windows(building, tint) {
    let rows = Math.floor(building.height / 3.4) - 1
    let columns = 3 + building.seed % 3
    let facing_side = -1.0
    if building.x < 0.0 { facing_side = 1.0 }
    let color = window_colors[tint]
    let strength = [1.5, 1.65, 1.55][tint]
    return T {
        for row in range(rows) {
            let y = 4.0 + row * 3.15
            for column in range(columns) {
                let cell = building.seed * 11 + row * 17 + column * 13
                if cell % 11 < 6 && (row + column + building.seed) % 3 == tint {
                    let x = building.x + ((column + 0.5) / columns - 0.5) * (building.width - 2.0)
                    window([x, y, building.z + building.depth / 2.0 + 0.045],
                        [1.05 + (cell % 3) * 0.29, 0.85, 0.07], color, strength)
                }
            }
            if (row * 7 + building.seed) % 9 < 6 && (row + 2 * building.seed) % 3 == tint {
                let x = building.x + facing_side * (building.width / 2.0 + 0.045)
                for slot in [0.27, -0.13] {
                    window([x, y, building.z + slot * building.depth], [0.07, 0.85, 1.25], color, strength)
                }
            }
        }
    }
}

Raycastable.disabled() {
    Selectable.off()
    T {
        name = "interceptor_distant_city"
        box([0.0, -0.48, 0.0], [420.0, 0.8, 420.0], [0.012, 0.009, 0.016], far_anime, false)
        for side in range(4) {
            T.rotation(0.0, side * Math.pi / 2.0, 0.0) {
                CombineMesh {
                    name = "interceptor_towers_" + side
                    far_anime
                    for index in range(tower_count) {
                        let building = tower(side, index)
                        box([building.x, building.height / 2.0, building.z],
                            [building.width, building.height, building.depth],
                            [0.025, 0.017, 0.034], null, false)
                    }
                }
                for tint in range(3) {
                    CombineMesh {
                        name = "interceptor_city_windows_" + side + "_" + tint
                        glow
                        for index in range(tower_count) { facade_windows(tower(side, index), tint) }
                    }
                }
            }
        }
    }
}

let roof_y = 28.0
let walls = [0.33, 0.36, 0.45]
let trim = [0.16, 0.17, 0.24]
let door = [0.24, 0.27, 0.36]
box([0.0, roof_y / 2.0 - 0.25, 0.0], [27.9, roof_y - 0.5, 33.9], [0.043, 0.036, 0.061], far_anime, false)
T.position(0.0, roof_y, 0.0) {
    name = "interceptor_rooftop_hangar"
    box([0.0, -0.25, 0.0], [28.0, 0.5, 34.0], [0.19, 0.19, 0.25], hangar_anime, true)
    box([0.0, 7.64, 0.0], [20.6, 0.36, 24.9], walls, hangar_anime, true)
    for side in [-1, 1] {
        box([side * 10.03, 3.75, 0.0], [0.30, 7.5, 24.5], walls, hangar_anime, true)
        box([0.0, 6.78, side * 12.20], [20.3, 1.44, 0.38], walls, hangar_anime, true)
        box([0.0, 6.03, side * 12.30], [20.3, 0.15, 0.56], trim, hangar_anime, false)
        for edge in [-1, 1] {
            // Three sliding leaves parked beside each opening, with tracks above.
            box([edge * 8.45, 2.99, side * 12.15], [3.3, 5.98, 0.26], walls, hangar_anime, true)
            for leaf in range(3) {
                let x = edge * (7.20 + leaf * 1.02)
                box([x, 2.97, side * 11.94], [0.98, 5.94, 0.19], door, hangar_anime, false)
            }
        }
        box([side * 13.85, 0.53, 0.0], [0.28, 1.06, 34.0], trim, hangar_anime, true)
        box([0.0, 0.53, side * 16.86], [27.5, 1.06, 0.28], trim, hangar_anime, true)
    }
    for z in [-9.0, -3.0, 3.0, 9.0] {
        T.position(0.0, 6.92, z).scale(1.08, 0.78, 0.78) { hangar_anime truss(16) }
        for side in [-1, 1] {
            box([side * 9.70, 3.47, z], [0.44, 6.94, 0.55], trim, hangar_anime, true)
        }
        box([0.0, 6.32, z], [3.4, 0.14, 0.24], trim, hangar_anime, false)
        T { glow window([0.0, 6.24, z], [3.15, 0.045, 0.17], [0.73, 0.68, 0.55], 1.35) }
        T.position(0.0, 6.12, z) { PL.color(0.86, 0.79, 0.96).intensity(1.6).distance(12.0) }
    }
    // The pad is flush with the hull's lowest point after undoing the GLB's
    // authored scene offset. The outer transform yaws the whole interceptor.
    box([0.0, 0.045, 0.0], [7.8, 0.09, 8.6], [0.11, 0.12, 0.18], hangar_anime, true)
    for edge in [-1, 1] {
        box([edge * 3.77, 0.094, 0.0], [0.075, 0.007, 8.34], [0.60, 0.46, 0.30], hangar_anime, false)
        box([0.0, 0.094, edge * 4.17], [7.56, 0.007, 0.075], [0.60, 0.46, 0.30], hangar_anime, false)
    }
    T.rotation(0.0, -0.24, 0.0) {
        name = "interceptor_display"
        T.position(0.0, 1.09, -1.1968) {
            GLTF.new("assets/models/interceptor.glb") { name = "interceptor_hull" hull_anime }
        }
    }
}
fall_respawn("interceptor_rooftop_return", { min_x = -5.0 max_x = 5.0 min_z = 7.0 max_z = 9.0 }, roof_y, 12.0)

let net = MultiplayerSession.from_env() {}
net
let spawn_x = -2.5
let local_port = 7011
let peer_port = 7012
if !net.is_host() { spawn_x = 2.5 local_port = 7012 peer_port = 7011 }
let socket = UdpSocket.bind("0.0.0.0:" + local_port) {}
socket

let microphone = AudioInput {}
let voice_level = Amplitude.rolling_window(0.080).highpass(120.0).highpass_resonance(0.707).from(microphone) {}
let local = make_rooftop_player({
    prefix = "local" local_player = true avatar = "mitten"
    use_xr = true use_desktop = false camera_far = 240.0 opaque_bucket = "near"
    voice_level = voice_level mouth_tuning = { center_rms = 0.0475 range_rms = 0.085 amount = 1.0 }
})
local.label.set_text(net.local_name())
local.root.update_transform([spawn_x, roof_y, 7.6])
local.locomotion
local.watch_pointer(query("#local_left_pointer"), "left")
local.watch_pointer(query("#local_right_pointer"), "right")
let calibration_file = XrSettings.floor.default_file()
XrSettings.floor.set_correction_meters(XrSettings.floor.correction_meters({ file = calibration_file }))

let picker_status = { text = "Mitten is active" }
fn report_avatar(message) {
    picker_status.text = message
    let line = query("#avatar_picker_status")
    if line { line.set_text(message) }
}
fn apply_avatar_height() { local.apply_desired_height(report_avatar) }
on_global("DataEvent", fn(event) {
    if event == "AvatarSettingsChanged" { apply_avatar_height() }
})

let panels = T { name = "interceptor_panels" }
panels
T.position(spawn_x + 0.86, roof_y + 1.74, 6.15) {
    dock({ root_name = "interceptor_dock", mount = panels, groups = [
        [
            { id = "avatar", label = "Avatar", root_name = "interceptor_avatar_picker",
                position = [spawn_x - 0.95, roof_y + 2.3, 4.82], build = fn() {
                    let picker = avatar_picker_panel({
                        root_name = "interceptor_avatar_picker", status = picker_status.text,
                        choices = [{ id = "mitten" label = "Mitten" }, { id = "lumae" label = "Lumae" },
                            { id = "miku" label = "Miku" }, { id = "succubus" label = "Succubus" }],
                        on_select = fn(choice) { local.select_avatar(choice, report_avatar) }
                    })
                    return T.position(-picker.width / 2.0, 0.0, 0.0) { picker.root }
                } },
            { id = "mirror", label = "Mirror", root_name = "interceptor_mirror_frame",
                position = [7.9, roof_y + 1.32, 6.5], rotation = [0.0, -Math.pi / 2.0, 0.0],
                build = fn() { return mirror_panel({ name = "interceptor_mirror", width = 2.8, height = 1.6, quality = 1024 }) } },
        ],
        [
            { id = "input", label = "Input", root_name = "interceptor_input",
                position = [spawn_x + 1.9, roof_y + 2.36, 4.82],
                build = fn() { return input_settings_panel({ root_name = "interceptor_input" }) } },
            { id = "graphics", label = "Graphics", root_name = "interceptor_graphics",
                position = [spawn_x + 3.55, roof_y + 2.36, 4.82],
                build = fn() { return graphics_settings_panel({ root_name = "interceptor_graphics" }) } },
            { id = "humanoid", label = "Humanoid", root_name = "interceptor_humanoid",
                position = [spawn_x - 4.05, roof_y + 2.36, 4.82], build = fn() {
                    return avatar_settings_panel({ root_name = "interceptor_humanoid", show_xr = true,
                        calibration_file = calibration_file, on_height_recommitted = apply_avatar_height,
                        camera_view = local.camera_view })
                } },
        ],
    ] })
}
let dock_slot = T.position(0.35, 1.25, -0.45).rotation(0.0, -0.66, 0.0) {
    dock_dropzone({ name = "interceptor_dock_slot" })
}
local.root.attach(dock_slot)

// One coherent pose sample per tick. The remote rig is created on join, and
// solves its own arms and legs from the received head and grip targets.
let pose_sample = {
    root_pos: vec3, root_rot: vec4, root_scale: vec3,
    head_pos: vec3, head_rot: vec4, left_pos: vec3, left_rot: vec4, right_pos: vec3, right_rot: vec4,
    tracking: vec3, avatar: str, height: f32
}
let peer_scene = T { name = "interceptor_peer_scene" }
peer_scene
let peer = { id = null group = null player = null head = null outgoing = null avatar = null elapsed = 0.0 }

on(net, "MultiplayerPeerJoined", fn(event) {
    if peer.id != null { return }
    let ip = net.peer_ip(event.peer_id)
    if !ip { return }
    let address = ip + ":" + peer_port
    let remote = make_rooftop_player({
        prefix = "remote" local_player = false avatar = "mitten" use_xr = true use_desktop = false
    })
    remote.label.set_text(event.name)
    remote.root.update_transform([-spawn_x, roof_y, 7.6])
    let outgoing = socket.publish("interceptor.pose").to(address).type(pose_sample)
    let incoming = socket.subscribe("interceptor.pose").from(address).type(pose_sample)
    let group = T { name = "interceptor_peer" remote.root outgoing incoming }
    peer.id = event.peer_id
    peer.group = group
    peer.player = remote
    peer.head = remote.head
    peer.outgoing = outgoing
    peer.avatar = "mitten"
    remote.on_avatar_changed(fn(head, model, choice) { peer.head = head })
    on(incoming, "UdpSample", fn(event) {
        if peer.group != group { return }
        let pose = event.value
        if pose.avatar != peer.avatar {
            peer.avatar = pose.avatar
            remote.select_avatar(pose.avatar, fn(message) {})
        }
        let height = null
        if pose.height > 0.0 { height = pose.height }
        remote.set_remote_height(height, fn(message) {})
        remote.root.world_trs({ translation = pose.root_pos, rotation_quat_xyzw = pose.root_rot, scale = pose.root_scale })
        peer.head.trs({ translation = pose.head_pos, rotation_quat_xyzw = pose.head_rot, scale = [1.0, 1.0, 1.0] })
        remote.left.trs({ translation = pose.left_pos, rotation_quat_xyzw = pose.left_rot, scale = [1.0, 1.0, 1.0] })
        remote.right.trs({ translation = pose.right_pos, rotation_quat_xyzw = pose.right_rot, scale = [1.0, 1.0, 1.0] })
        remote.left_driver.set_pose_valid(pose.tracking[0] > 0.5)
        remote.right_driver.set_pose_valid(pose.tracking[1] > 0.5)
        remote.set_remote_flying(pose.tracking[2] > 0.5)
    })
    peer_scene.attach(group)
})
on(net, "MultiplayerPeerLeft", fn(event) {
    if peer.id != event.peer_id { return }
    peer.player.cancel_avatar_load()
    peer.group.remove_subtree()
    peer.id = null
    peer.group = null
    peer.player = null
    peer.head = null
    peer.outgoing = null
})
on_global("FrameTick", fn(event) {
    if !peer.outgoing || !peer.outgoing.ready() { return }
    peer.elapsed = peer.elapsed + event.dt_sec
    if peer.elapsed < 0.05 { return }
    peer.elapsed = 0.0
    let root = local.root.world_trs()
    let head = local.head_pose()
    let left = local.left.trs()
    let right = local.right.trs()
    let left_tracked = 0.0
    let right_tracked = 0.0
    let flying = 0.0
    if local.left_driver.pose_valid() { left_tracked = 1.0 }
    if local.right_driver.pose_valid() { right_tracked = 1.0 }
    if local.flying() { flying = 1.0 }
    let height = local.desired_height()
    if height == null { height = 0.0 }
    peer.outgoing.write({
        root_pos = root.translation, root_rot = root.rotation_quat_xyzw, root_scale = root.scale,
        head_pos = head.translation, head_rot = head.rotation_quat_xyzw,
        left_pos = left.translation, left_rot = left.rotation_quat_xyzw,
        right_pos = right.translation, right_rot = right.rotation_quat_xyzw,
        tracking = [left_tracked, right_tracked, flying], avatar = local.avatar_id(), height = height
    })
})
