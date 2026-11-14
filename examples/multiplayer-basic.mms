// View flags: Dan keeps these defaults. Rei sets XR=true and desktop=false locally.
// Network role is separate: Rei launches as host, Dan as client (commands below).
let use_xr = false
let use_desktop = true

import { f32, vec3, vec4 } from "types"
import { bisket_anime_shading } from "../assets/components/materials/bisket_anime_shading.mms"
import { bisket_humanoid_bone_map } from "../assets/components/humanoid_bone_maps/bisket.mms"
import { bisket_colliders } from "../assets/components/colliders/bisket.mms"
import { bisket_shirt_physics } from "../assets/components/secondary_motion/bisket-shirt-physics.mms"
import { mitten_bow_secondary_motion } from "../assets/components/secondary_motion/mitten-bow.mms"
import { vroid_arm_ik_bend_plane_prototype } from "../assets/components/arm_ik/vroid.mms"

// Two-player LAN/loopback multiplayer: one host, one client, same arena.
// The host owns physics (the bouncing ball); each player owns their own pose.
// No interpolation yet, so remote motion steps at the 20 Hz send rate.
//
//   MITTENS_MULTIPLAYER_ROLE=host   MITTENS_MULTIPLAYER_NAME=rei cargo run --release -- load examples/multiplayer-basic.mms
//   MITTENS_MULTIPLAYER_ROLE=client MITTENS_MULTIPLAYER_NAME=dan cargo run --release -- load examples/multiplayer-basic.mms
//
// Across machines: the host binds 0.0.0.0:7010 by default; the client sets
// MITTENS_MULTIPLAYER_ADDR=<host-ip>:7010 (numeric address, UDP port open on the host).
// Desktop: WASD + right-drag to move/look; E kicks the ball. XR: tracked head/hands
// and gamepad locomotion. XR-only mode creates no winit window or Camera3D.

let net = MultiplayerSession.from_env() {}
net

RendererSettings { window_visible(use_desktop) window_size(1280, 800) }
BGC.rgba(0.62, 0.70, 0.86, 1.0)
AL.rgb(0.30, 0.31, 0.36)
T.position(-0.4, 0.9, 0.5) { DL.color(1.0, 0.94, 0.86).intensity(1.2) }
T.position(0.6, 0.5, -0.4) { DL.color(0.62, 0.74, 1.0).intensity(0.35) }

let arena_half = 7.0
let ball_radius = 0.35

// Arena floor and posts so motion has a reference.
T.position(0.0, -0.1, 0.0).scale(arena_half * 2.0, 0.2, arena_half * 2.0) {
    R.cube() { C.rgba(0.80, 0.78, 0.74, 1.0) }
}
for corner in [[-1.0, -1.0], [1.0, -1.0], [-1.0, 1.0], [1.0, 1.0]] {
    T.position(corner[0] * arena_half, 0.6, corner[1] * arena_half).scale(0.3, 1.2, 0.3) {
        R.cube() { C.rgba(0.36, 0.42, 0.62, 1.0) }
    }
}

// Status board.
let status = Text { "connecting..." C.rgba(0.16, 0.17, 0.22, 1.0) }
T.position(-2.6, 3.2, -7.0).scale(0.22, 0.22, 0.22) { status }
let roster = { count = 0 }
fn show_status(line) {
    let role = "client"
    if net.is_host() { role = "host" }
    status.set_text(net.local_name() + " (" + role + ", peer " + net.peer_id() + ")\nremote players: " + roster.count + "\n" + line)
}

// Host simulates the ball. Clients bind its typed pose without a simulation writer.
let ball = T.position(0.0, 3.0, 0.0) {
    name = "ball"
    T.scale(ball_radius * 2.0, ball_radius * 2.0, ball_radius * 2.0) {
        R.sphere() { C.rgba(1.0, 0.62, 0.20, 1.0) EM.on() }
    }
}
let ball_velocity = Velocity { name = "ball_velocity" }
if net.is_host() { ball_velocity.attach(ball) Gravity.enabled(true) { ball_velocity } }
else { ball }


// Whole transforms travel through typed channels; every machine solves its own rigs.
let channel_port = 7011
let remote_port = 7012
if !net.is_host() { channel_port = 7012 remote_port = 7011 }
let socket = UdpSocket.bind("0.0.0.0:" + channel_port) {}
socket
let pose_type = { translation: vec3, rotation_quat_xyzw: vec4, scale: vec3 }

fn make_mitten(prefix, local_player) {
    let model = GLTF.new("assets/models/mitten.glb") {
        name = prefix + "_model"
        bisket_anime_shading()
        bisket_humanoid_bone_map()
        MorphTargetMap.new()
            .slot("left_eye_blink", "Fcl_EYE_Close_L")
            .slot("right_eye_blink", "Fcl_EYE_Close_R")
            .slot("viseme_aa", "Fcl_MTH_A")
        bisket_colliders()
        bisket_shirt_physics(false)
        mitten_bow_secondary_motion()
    }
    let left = T.position(-0.35, -0.35, -0.25) { name = prefix + "_left_target" }
    let right = T.position(0.35, -0.35, -0.25) { name = prefix + "_right_target" }
    let external_hands = !local_player || !use_xr
    let left_driver = XRHand.new(true, "Left", "GripAim").external(external_hands) { left }
    let right_driver = XRHand.new(true, "Right", "GripAim").external(external_hands) { right }
    let root_name = prefix + "_root"
    let avatar = AVC.movement_target("[name='" + root_name + "']") {
        name = prefix + "_avatar"
        idle_blink(local_player)
        if !use_xr { blink_interval(1.0) blink_duration(0.20) }
        if !local_player { collision_disabled() }
        initial_yaw(3.14159)
        arm_ik(vroid_arm_ik_bend_plane_prototype())
        hand_rotation_smoothing(220.0)
        T { model }
        left_driver
        right_driver
        if local_player && use_xr {
            CXR {}
            XREyeTracking.on().priority(["htc", "vrchat_osc"]).enable_pupil_direction_tracking(false)
        }
    }
    let head = T.position(0.0, 1.65, 0.0) { name = prefix + "_head_target" avatar }
    let label = Text { prefix C.rgba(0.12, 0.14, 0.20, 1.0) }
    let root = T { name = root_name T.position(-0.25, 2.2, 0.0).scale(0.1, 0.1, 0.1) { label } }
    if local_player && use_xr {
        let driver = InputXR.on() { InputXRGamepad { locomotion() speed(1.5) } head }
        root.attach(driver)
    } else { let driver = PoseInput { head } root.attach(driver) }
    return { root = root head = head left = left right = right model = model avatar = avatar left_driver = left_driver right_driver = right_driver label = label }
}

let local = make_mitten("local", true)
let remote = make_mitten("remote", false)
local.label.set_text(net.local_name())
let spawn_x = -1.5
if !net.is_host() { spawn_x = 1.5 }
local.root.update_transform([spawn_x, 0.0, 0.0])
remote.root.update_transform([-spawn_x, 0.0, 0.0])
if use_desktop && !use_xr {
    I.speed(2.0) { InputTransformMode.forward_z() { fps_rotation() roll_axis_y() } local.root }
} else { local.root }
if use_desktop {
    // Observer camera keeps both fully solved avatars visible in the desktop demo.
    T.position(0.0, 1.8, 6.0) { C3D { fov(65.0) near(0.05) Pointer {} } }
}
if use_xr { XR.on() } else { XR.off() }
remote.root
let links = { root = null head = null left = null right = null blink_left = null blink_right = null left_valid = null right_valid = null ball = null }
let channel_state = { ticks = 0.0 send_elapsed = 0.0 }
let connection = { root = null }
let channel_groups = T { name = "channel_groups" }
channel_groups

fn publish(name, address, kind) {
    let channel = socket.publish(name).to(address).type(kind)
    connection.root.attach(channel)
    return channel
}
fn receive_transform(name, address, target, space) {
    let channel = socket.subscribe(name).from(address).type(pose_type)
    let configured = channel.bind_transform(target, { space = space })
    connection.root.attach(configured)
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
    links.root = publish("root", address, pose_type)
    links.head = publish("head", address, pose_type)
    links.left = publish("left-hand", address, pose_type)
    links.right = publish("right-hand", address, pose_type)
    links.blink_left = publish("blink.left", address, f32)
    links.blink_right = publish("blink.right", address, f32)
    links.left_valid = publish("left-hand.valid", address, f32)
    links.right_valid = publish("right-hand.valid", address, f32)
    receive_transform("root", address, remote.root, "world")
    receive_transform("head", address, remote.head, "local")
    receive_transform("left-hand", address, remote.left, "local")
    receive_transform("right-hand", address, remote.right, "local")
    let left_valid = socket.subscribe("left-hand.valid").from(address).type(f32)
    connection.root.attach(left_valid)
    on(left_valid, "UdpSample", fn(sample) { remote.left_driver.set_pose_valid(sample.value > 0.5) })
    let right_valid = socket.subscribe("right-hand.valid").from(address).type(f32)
    connection.root.attach(right_valid)
    on(right_valid, "UdpSample", fn(sample) { remote.right_driver.set_pose_valid(sample.value > 0.5) })
    let blink_left = socket.subscribe("blink.left").from(address).type(f32)
    let left_binding = blink_left.bind_morph(remote.model, "left_eye_blink")
    connection.root.attach(left_binding)
    let blink_right = socket.subscribe("blink.right").from(address).type(f32)
    let right_binding = blink_right.bind_morph(remote.model, "right_eye_blink")
    connection.root.attach(right_binding)
    if net.is_host() { links.ball = publish("ball", address, pose_type) }
    else { receive_transform("ball", address, ball, "world") }
    roster.count = 1
    show_status(event.name + " joined; typed pose and face channels")
})
on(net, "MultiplayerPeerLeft", fn(event) {
    if connection.root { connection.root.remove_subtree() connection.root = null }
    links.root = null
    roster.count = 0
    show_status(event.name + " left; holding last received pose")
})
on(net, "MultiplayerError", fn(event) { show_status("session " + event.phase + ": " + event.message) })
on_global("UdpState", fn(event) { if event.message != "" { show_status("channel: " + event.message) } })
on_global("FrameTick", fn(event) {
    channel_state.ticks = channel_state.ticks + event.dt_sec
    if !use_xr {
        // Scripted hand targets exercise receiving-side arm IK without tracking hardware.
        local.left_driver.set_pose_valid(true)
        local.right_driver.set_pose_valid(true)
        let wave = Math.sin(channel_state.ticks * 2.0) * 0.15
        local.left.update_transform([-0.35, -0.35 + wave, -0.25])
        local.right.update_transform([0.35, -0.35 - wave, -0.25])
    }
    channel_state.send_elapsed = channel_state.send_elapsed + event.dt_sec
    if channel_state.send_elapsed < 0.05 { return }
    channel_state.send_elapsed = 0.0
    if links.root && links.root.ready() {
        links.root.write(local.root.world_trs())
        if links.head.ready() { links.head.write(local.head.trs()) }
        if links.left.ready() { links.left.write(local.left.trs()) }
        if links.right.ready() { links.right.write(local.right.trs()) }
        if links.blink_left.ready() { links.blink_left.write(local.model.morph_weight("left_eye_blink")) }
        if links.blink_right.ready() { links.blink_right.write(local.model.morph_weight("right_eye_blink")) }
        let left_tracked = 0.0
        let right_tracked = 0.0
        if local.left_driver.pose_valid() { left_tracked = 1.0 }
        if local.right_driver.pose_valid() { right_tracked = 1.0 }
        if links.left_valid.ready() { links.left_valid.write(left_tracked) }
        if links.right_valid.ready() { links.right_valid.write(right_tracked) }
        if links.ball && links.ball.ready() { links.ball.write(ball.world_trs()) }
    }
})


fn kick(sender) {
    let side = 1.5
    if sender == 1 { side = -1.5 }
    ball_velocity.translate_world([side, 6.5, -1.0])
}

if net.is_host() {
    ball_velocity.translate_world([2.0, 0.0, 1.2])
    // Minimal hand-rolled bounce: floor restitution and arena walls.
    on_global("FrameTick", fn(event) {
        let p = ball.translation()
        let v = ball_velocity.linear()
        let dx = 0.0
        let dy = 0.0
        let dz = 0.0
        if p[1] < ball_radius && v[1] < 0.0 { dy = -1.85 * v[1] }
        if p[0] > arena_half - ball_radius && v[0] > 0.0 { dx = -2.0 * v[0] }
        if p[0] < ball_radius - arena_half && v[0] < 0.0 { dx = -2.0 * v[0] }
        if p[2] > arena_half - ball_radius && v[2] > 0.0 { dz = -2.0 * v[2] }
        if p[2] < ball_radius - arena_half && v[2] < 0.0 { dz = -2.0 * v[2] }
        if dx != 0.0 || dy != 0.0 || dz != 0.0 { ball_velocity.translate_world([dx, dy, dz]) }
    })
    // Client intents arrive as app messages; the host decides what happens.
    on(net, "MultiplayerMessage", fn(event) {
        if event.name == "kick" { kick(event.sender) }
    })
}

on_global("KeyDown", fn(event) {
    if event.code == "KeyE" {
        if net.is_host() { kick(1) } else { net.send("kick", "") }
    }
})
