// Two-player LAN/loopback multiplayer: one host, one client, same arena.
// The host owns physics (the bouncing ball); each player owns their own pose.
// No interpolation yet, so remote motion steps at the 20 Hz send rate.
//
//   MITTENS_NET_ROLE=host   MITTENS_NET_NAME=rei cargo run --release -- load examples/multiplayer-basic.mms
//   MITTENS_NET_ROLE=client MITTENS_NET_NAME=dan cargo run --release -- load examples/multiplayer-basic.mms
//
// Across machines: the host binds 0.0.0.0:7010 by default; the client sets
// MITTENS_NET_ADDR=<host-ip>:7010 (numeric address, UDP port open on the host).
// WASD + right-drag to move/look. E kicks the ball (clients ask the host to).

let net = NetSession.from_env() {}
net

RendererSettings { window_size(1280, 800) }
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

// Local player: first-person camera; its pose is what other peers see.
let spawn_x = 1.5
if net.is_host() { spawn_x = -1.5 }
I.speed(4.0) {
    InputTransformMode.forward_z() { fps_rotation() roll_axis_y() }
    T.position(spawn_x, 1.6, 5.0) {
        name = "local_player"
        NetSync.local_player()
        C3D { fov(74.0) near(0.05) Pointer {} }
    }
}

// Remote player proxy. The synced transform is the remote eye, so the body
// hangs below it and the nose marks the look direction.
let proxies = T { name = "net_proxies" }
proxies
fn remote_player(event) {
    return T {
        name = "net_proxy_" + event.peer_id
        NetSync.remote_player(event.peer_id)
        T.scale(0.32, 0.32, 0.32) { R.sphere() { C.rgba(0.98, 0.82, 0.72, 1.0) } }
        T.position(0.0, 0.0, -0.17).scale(0.07, 0.07, 0.12) { R.cube() { C.rgba(0.95, 0.45, 0.50, 1.0) } }
        T.position(0.0, -0.75, 0.0).scale(0.42, 0.95, 0.26) { R.cube() { C.rgba(0.40, 0.56, 0.86, 1.0) } }
        T.position(-0.3, 0.32, 0.0).scale(0.06, 0.06, 0.06) { Text { event.name C.rgba(0.10, 0.10, 0.14, 1.0) } }
    }
}

on(net, "NetConnected", fn(event) { show_status("connected") })
on(net, "NetPeerJoined", fn(event) {
    proxies.attach(remote_player(event))
    roster.count = roster.count + 1
    show_status(event.name + " joined")
})
on(net, "NetPeerLeft", fn(event) {
    let proxy = query("[name='net_proxy_" + event.peer_id + "']")
    if proxy { proxy.remove_subtree() }
    roster.count = roster.count - 1
    show_status(event.name + " left")
})
on(net, "NetError", fn(event) { show_status("net " + event.phase + ": " + event.message) })

// Host-authoritative ball. Clients receive its pose and never simulate it:
// NetSync.host disables this Velocity on clients.
let ball = T.position(0.0, 3.0, 0.0) {
    name = "ball"
    NetSync.host("ball")
    T.scale(ball_radius * 2.0, ball_radius * 2.0, ball_radius * 2.0) {
        R.sphere() { C.rgba(1.0, 0.62, 0.20, 1.0) EM.on() }
    }
}
let ball_velocity = Velocity { name = "ball_velocity" ball }
Gravity.enabled(true) { ball_velocity }

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
    on(net, "NetMessage", fn(event) {
        if event.name == "kick" { kick(event.sender) }
    })
}

on_global("KeyDown", fn(event) {
    if event.code == "KeyE" {
        if net.is_host() { kick(1) } else { net.send("kick", "") }
    }
})

XR.off()
