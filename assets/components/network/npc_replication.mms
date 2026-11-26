import { f32, vec2, vec3, vec4 } from "types"
import { make_rooftop_player } from "../characters/rooftop_player.mms"
import { lie_down, get_up } from "../characters/downed.mms"
import { mitten_right_wrist_basis, pistol_grip } from "../poses/bisket/pistol_grip.mms"
import { energy_shield } from "../energy_sheild.mms"
import { player_death } from "../player_death.mms"
import { ship_machine_pistol } from "../weapons/ship_machine_pistol.mms"
import { laser } from "../laser.mms"
import { muzzle_flash } from "../muzzle-flash.mms"

// A host-authoritative NPC shared with one peer. The host runs the NPC
// (practice_target) and the client shows a puppet of it:
//   <name>.pose     host -> client, latest value: root (world), head and
//                   hands (local) plus small state, 20 Hz while spawned
//   <name>.actions  host -> client, each event once and in order
//                   (npc_action_window): spawn, remove, shot, down, respawn
//   <name>.peer     client -> host, latest value: cumulative damage to the
//                   NPC's current life and the latest spawn/remove request
// Cumulative counters survive loss like shared_player_health's reports.
let ONE = [1.0, 1.0, 1.0]
let IDENTITY = [0.0, 0.0, 0.0, 1.0]
let FULL = 10.0
// A pistol point's worth of shield, as in target_practice.mms.
let SHIELD_PER_POINT = 10.0
// Action kinds; a shot's argument is its fire mode, a fall's its lying kind.
let SPAWN = 1.0
let REMOVE = 2.0
let SHOT = 3.0
let DOWN = 4.0
let RESPAWN = 5.0
// Shot sounds as laser_machine_pistol plays them: each semi shot, or once per
// three-shot burst or auto cycle. A pause this long starts a new run.
let SHOT_GAIN = 0.6
let SHOT_RUN_GAP = 0.4

let pose_sample = {
    root_pos: vec3, root_rot: vec4, head_pos: vec3, head_rot: vec4,
    left_pos: vec3, left_rot: vec4, right_pos: vec3, right_rot: vec4,
    life: f32, actions: f32, spawned: f32, alive: f32, health: f32, shield: f32, lying: f32, holding: f32
}
let peer_sample = { life: f32, points: f32, shield: f32, request: f32, wanted: f32 }
// Up to four consecutive events: kind and argument, then a pose (shots).
let action_sample = {
    first: f32, count: f32,
    k0: vec2, p0: vec3, q0: vec4, k1: vec2, p1: vec3, q1: vec4,
    k2: vec2, p2: vec3, q2: vec4, k3: vec2, p3: vec3, q3: vec4,
}

// Lying kinds on the wire: 0 standing, 1 on the back, 2 on the side.
fn lying_code(kind) {
    if kind == "back" { return 1.0 }
    if kind == "side" { return 2.0 }
    return 0.0
}
fn lying_kind(code) {
    if code == 2.0 { return "side" }
    return "back"
}
// Fire modes on the wire, as pistol_replication sends them.
fn mode_code(mode) {
    if mode == "burst" { return 1.0 }
    if mode == "auto" { return 2.0 }
    return 0.0
}

// Discrete events that must apply once each, in order, over the
// latest-value transport: shared_impact's sequence window. Each sample
// carries the oldest unacknowledged sequence (first), how many follow
// (count, up to four) and their slots; the sender resends that window on a
// 20 Hz pump until the receiver acknowledges it. options: { socket, name,
// apply(sequence, kind, arg, position, rotation) }.
export fn npc_action_window(options) {
    let empty = { kind = [0.0, 0.0] position = [0.0, 0.0, 0.0] rotation = IDENTITY }
    let state = { generation = 0 outgoing = null queue = {} first = 1 next = 1 received = 0 elapsed = 0.0 pump = null }

    fn queue_key(sequence) { return "" + (sequence % 64) }
    fn queued(sequence) {
        if sequence >= state.next { return empty }
        return state.queue[queue_key(sequence)]
    }

    fn send() {
        if !state.outgoing || !state.outgoing.ready() || state.first == state.next { return }
        let first = state.first
        let sample = { first = first count = Math.clamp(state.next - first, 0, 4) }
        for i in range(4) {
            let event = queued(first + i)
            sample["k" + i] = event.kind
            sample["p" + i] = event.position
            sample["q" + i] = event.rotation
        }
        state.outgoing.write(sample)
    }

    // Unconnected (solo) events are dropped: there is nobody to replay them.
    fn push(kind, arg, position, rotation) {
        if !state.outgoing || state.next - state.first >= 64 { return }
        state.queue[queue_key(state.next)] = { kind = [kind, arg] position = position rotation = rotation }
        state.next = state.next + 1
        if state.next == state.first + 1 { state.pump.bypass(false) }
        send()
    }

    fn disconnect() {
        state.generation = state.generation + 1
        state.outgoing = null
        state.queue = {}
        state.first = 1
        state.next = 1
        state.received = 0
        state.elapsed = 0.0
        state.pump.bypass(true)
    }

    fn connect(group, address) {
        disconnect()
        let generation = state.generation
        let name = options.name + ".actions"
        let ack_name = name + ".ack"
        let outgoing = options.socket.publish(name).to(address).type(action_sample)
        let incoming = options.socket.subscribe(name).from(address).type(action_sample)
        let ack = options.socket.publish(ack_name).to(address).type(f32)
        let acknowledged = options.socket.subscribe(ack_name).from(address).type(f32)
        group.attach(outgoing)
        group.attach(incoming)
        group.attach(ack)
        group.attach(acknowledged)
        state.outgoing = outgoing
        on(incoming, "UdpSample", fn(event) {
            if generation != state.generation { return }
            let sample = event.value
            for i in range(Math.clamp(Math.floor(sample.count), 0, 4)) {
                let sequence = sample.first + i
                if sequence == state.received + 1 {
                    let kind = sample["k" + i]
                    state.received = sequence
                    options.apply(sequence, kind[0], kind[1], sample["p" + i], sample["q" + i])
                }
            }
            if ack.ready() { ack.write(state.received) }
        })
        on(acknowledged, "UdpSample", fn(event) {
            if generation != state.generation { return }
            let last = Math.floor(event.value)
            if last < state.first || last >= state.next { return }
            for i in range(last - state.first + 1) { state.queue[queue_key(state.first + i)] = null }
            state.first = last + 1
            if state.first == state.next { state.pump.bypass(true) }
            send()
        })
    }

    state.pump = on_global("FrameTick", fn(event) {
        state.elapsed = state.elapsed + event.dt_sec
        if state.elapsed < 0.05 { return }
        state.elapsed = 0.0
        send()
    })
    state.pump.bypass(true)
    return {
        push = push connect = connect disconnect = disconnect
        // The last sequence sent, and the last applied.
        sent = fn() { return state.next - 1 }
        received = fn() { return state.received }
    }
}

// Which of several players an NPC aims at: the nearest living one, switching
// only when another is clearly closer. root follows the chosen player's mount,
// for practice_target's player option; alive() and hit(points) go to the
// chosen player. players: [{ mount, alive(), hit(points) }]. origin() is the
// NPC's position, or null while there is none. Runs between start() and stop().
export fn npc_target_switch(name, players, origin) {
    let SWITCH_MARGIN = 1.5
    let root = T { name = name }
    let state = { index = 0 elapsed = 0.0 pump = null }
    fn distance(index, from) {
        let p = players[index].mount.world_trs().translation
        let x = p[0] - from[0]
        let y = p[1] - from[1]
        let z = p[2] - from[2]
        return Math.sqrt(x * x + y * y + z * z)
    }
    fn choose() {
        let from = origin()
        if !from { return }
        let best = -1
        let best_distance = 0.0
        for index in range(len(players)) {
            if players[index].alive() {
                let d = distance(index, from)
                if best < 0 || d < best_distance {
                    best = index
                    best_distance = d
                }
            }
        }
        if best < 0 || best == state.index { return }
        if players[state.index].alive() && distance(state.index, from) - best_distance < SWITCH_MARGIN { return }
        state.index = best
    }
    fn follow() {
        let p = players[state.index].mount.world_trs().translation
        root.world_trs({ translation = p rotation_quat_xyzw = IDENTITY scale = ONE })
    }
    state.pump = on_global("FrameTick", fn(event) {
        state.elapsed = state.elapsed + event.dt_sec
        if state.elapsed >= 0.5 {
            state.elapsed = 0.0
            choose()
        }
        follow()
    })
    state.pump.bypass(true)
    return {
        root = root
        alive = fn() { return players[state.index].alive() }
        hit = fn(points) { players[state.index].hit(points) }
        start = fn() {
            state.elapsed = 0.0
            choose()
            follow()
            state.pump.bypass(false)
        }
        stop = fn() { state.pump.bypass(true) }
    }
}

// Replays an NPC's machine pistol shots: beams, muzzle flash and sounds at
// the received muzzle pose, with no hits (the host owns them).
fn npc_shot_effects(label) {
    let lasers = []
    for i in range(3) {
        lasers.push(laser({ name = label + "_" + i length = 60.0 beam_radius = 0.012 ray_origin_offset = 0.18 }))
    }
    let flash = muzzle_flash({ name = label size = 0.22 beats_per_second = 2.0 })
    let single_sound = AudioClip.wav("assets/audio/tool/laser_machine_pistol_A.0.wav") { name = label + "_single_sound" AudioGain.new(SHOT_GAIN) }
    let burst_sound = AudioClip.wav("assets/audio/tool/laser_machine_pistol_C.0.wav") { name = label + "_sound" AudioGain.new(SHOT_GAIN) }
    let auto_sound = AudioClip.wav("assets/audio/tool/laser_machine_pistol_B.0.wav") { name = label + "_auto_sound" AudioGain.new(SHOT_GAIN) }
    let muzzle = T {
        name = label + "_muzzle"
        Raycastable.disabled()
        flash.root
        AudioOutput { single_sound burst_sound auto_sound }
    }
    let root = T {
        name = label
        muzzle
        for slot in lasers { slot.root }
    }
    let state = { next = 0 run = 0 mode = -1.0 last = -1000.0 }
    fn play(position, rotation, mode, now) {
        let pose = { translation = position rotation_quat_xyzw = rotation scale = ONE }
        lasers[state.next % 3].play_visual(pose)
        state.next = state.next + 1
        muzzle.world_trs(pose)
        flash.play()
        if mode != state.mode || now - state.last > SHOT_RUN_GAP { state.run = 0 }
        state.mode = mode
        state.last = now
        if mode == 0.0 { single_sound.play() }
        else if state.run % 3 == 0 {
            if mode == 1.0 { burst_sound.play() } else { auto_sound.play() }
        }
        state.run = state.run + 1
    }
    return { root = root play = play }
}

// options:
//   socket, name (the channel prefix, e.g. "mitten"), host (true on the
//   machine that runs the NPC), prefix (the NPC's; the puppet adds
//   "_puppet"), avatar, position (the puppet's spawn point before the
//   first pose), health_view, on_damage(cue) (the client's hit cues)
//   host only: current() the live practice_target or null; spawn() and
//   remove() for the client's requests
// The host calls spawned(), removed(), shot(pose, mode), down(kind) and
// respawned() from the NPC's hooks. The client routes its pistol hits through
// pistol_hit(hit) and its pedestal presses through request(wanted).
export fn shared_npc(options) {
    let host = options.host == true
    let root = T { name = options.prefix + "_replication" }
    let link = {
        generation = 0 pose = null peer = null actions = null pump = null elapsed = 0.0 clock = 0.0
        // Host: the NPC's life (one per spawn or respawn), what the client
        // has reported for it so far, and the last request applied.
        life = 0.0 points_received = 0.0 shield_received = 0.0 request_seen = 0.0
        // Client: the puppet, the newest action sequence a pose has stood
        // for, and the cumulative report for the life it last saw.
        puppet = null serial = 0 pose_actions = 0.0
        report_life = 0.0 points_sent = 0.0 shield_sent = 0.0 request = 0.0 wanted = 0.0
    }
    let shots = npc_shot_effects(options.prefix + "_puppet_shot")
    root.attach(shots.root)

    // Client puppet: a non-local rooftop player wearing the NPC's shield and
    // holding a pistol that never fires.
    fn wear_shield(puppet) {
        if puppet.shield { puppet.shield.remove_subtree() }
        let shield = energy_shield(options.prefix + "_puppet_shield", 2.0, { worn = true })
        puppet.player.shield_mount.attach(shield)
        shield.set_worn(true, true)
        puppet.shield = shield
    }
    fn set_alive(puppet, alive) {
        if alive == puppet.alive { return }
        puppet.alive = alive
        if alive { wear_shield(puppet) }
        else if puppet.shield {
            puppet.shield.remove_subtree()
            puppet.shield = null
        }
    }
    // Retried by later poses until the body has loaded (lie_down refuses).
    fn set_lying(puppet, code) {
        if code == puppet.lying { return }
        if puppet.lying != 0 {
            get_up(puppet.player)
            puppet.lying = 0
        }
        if code != 0 && lie_down(puppet.player, lying_kind(code), null) { puppet.lying = code }
    }
    fn spawn_puppet() {
        if link.puppet { return }
        link.serial = link.serial + 1
        let prefix = options.prefix + "_puppet"
        let player = make_rooftop_player({
            prefix = prefix local_player = false avatar = options.avatar use_xr = true use_desktop = false
            opaque_bucket = "near" show_health = true health_view = options["health_view"] shield_mount = true
            relaxed_pose = true
        })
        player.label.set_text(options.prefix)
        player.left_driver.set_pose_valid(false)
        player.right_driver.set_pose_valid(true)
        // Held as practice_target holds it; the label stays unique per spawn.
        let wrist_basis = null
        if options.avatar == "mitten" { wrist_basis = mitten_right_wrist_basis() }
        let gun = ship_machine_pistol(prefix + "_pistol_" + link.serial, { opaque_bucket = "near" world_damage = false })
        for grab in gun.grip.query_all("grabbable") { grab.remove_subtree() }
        let grip_offset = [0.0, 0.0, -0.105]
        if wrist_basis { grip_offset = [-0.018, 0.0, -0.105] }
        let gun_mount = T.position(grip_offset[0], grip_offset[1], grip_offset[2]) { name = prefix + "_gun_mount" }
        let mount = gun_mount
        if wrist_basis {
            let q = wrist_basis
            mount = T.rotation_quat(-q[0], -q[1], -q[2], q[3]) { gun_mount }
        }
        player.right.attach(mount)
        gun_mount.attach(gun)
        if wrist_basis { gun.grip.set_rotation_quat([0.0, 0.0, 0.0, 1.0], "local") }
        on(player.model, "GLTFInitialized", fn(event) {
            pistol_grip(event.gltf, "Right")
            if wrist_basis {
                let wrist = event.gltf.query("#J_Bip_R_Hand")
                if wrist { wrist.attach(mount) }
            }
        })
        player.root.update_transform(options.position)
        root.attach(player.root)
        let puppet = { player = player shield = null alive = false lying = 0 }
        set_alive(puppet, true)
        link.puppet = puppet
    }
    fn remove_puppet() {
        let puppet = link.puppet
        if !puppet { return }
        link.puppet = null
        puppet.player.dispose()
        puppet.player.root.remove_subtree()
    }

    fn send_report() {
        if host || !link.peer || !link.peer.ready() { return }
        link.peer.write({
            life = link.report_life points = link.points_sent shield = link.shield_sent
            request = link.request wanted = link.wanted
        })
    }
    // A new life starts a new cumulative report.
    fn see_life(life) {
        if life == link.report_life { return }
        link.report_life = life
        link.points_sent = 0.0
        link.shield_sent = 0.0
        send_report()
    }

    fn apply_action(sequence, kind, arg, position, rotation) {
        if host { return }
        // A pose may already stand for this event's state; then only its
        // one-off effects play.
        let current = sequence > link.pose_actions
        if kind == SPAWN && current { spawn_puppet() }
        if kind == REMOVE && current { remove_puppet() }
        let puppet = link.puppet
        if !puppet { return }
        if kind == SHOT { shots.play(position, rotation, arg, link.clock) }
        if kind == DOWN {
            player_death(puppet.player.shield_mount.world_trs().translation, 2.0)
            if current {
                set_alive(puppet, false)
                set_lying(puppet, Math.floor(arg))
            }
        }
        if kind == RESPAWN && current {
            set_lying(puppet, 0)
            set_alive(puppet, true)
            puppet.player.health.set_health(1.0)
        }
    }
    let actions = npc_action_window({ socket = options.socket name = options.name apply = apply_action })

    fn receive_pose(sample) {
        // State (spawned, alive, lying) only from a pose no older than the
        // events already applied, so a late pose cannot undo one.
        if sample.actions >= actions.received() {
            link.pose_actions = Math.clamp(sample.actions, link.pose_actions, 1000000000.0)
            if sample.spawned > 0.5 { spawn_puppet() } else { remove_puppet() }
            if link.puppet {
                set_alive(link.puppet, sample.alive > 0.5)
                set_lying(link.puppet, Math.clamp(Math.floor(sample.lying + 0.5), 0, 2))
            }
        }
        let puppet = link.puppet
        if !puppet { return }
        see_life(sample.life)
        let player = puppet.player
        player.root.world_trs({ translation = sample.root_pos rotation_quat_xyzw = sample.root_rot scale = ONE })
        player.head.trs({ translation = sample.head_pos rotation_quat_xyzw = sample.head_rot scale = ONE })
        player.left.trs({ translation = sample.left_pos rotation_quat_xyzw = sample.left_rot scale = ONE })
        player.right.trs({ translation = sample.right_pos rotation_quat_xyzw = sample.right_rot scale = ONE })
        player.right_driver.set_pose_valid(sample.holding > 0.5)
        player.health.set_health(Math.clamp(sample.health, 0.0, 1.0))
        // The host's shield is the truth: follow it down (rockets, the host's
        // own shots). Hits seen here already took their share locally.
        if puppet.shield {
            let integrity = puppet.shield.integrity()
            if sample.shield < integrity - 0.5 { puppet.shield.take_damage(integrity - sample.shield) }
        }
    }

    fn send_pose() {
        if !host || !link.pose || !link.pose.ready() { return }
        let target = options.current()
        let sample = {
            root_pos = [0.0, 0.0, 0.0] root_rot = IDENTITY head_pos = [0.0, 0.0, 0.0] head_rot = IDENTITY
            left_pos = [0.0, 0.0, 0.0] left_rot = IDENTITY right_pos = [0.0, 0.0, 0.0] right_rot = IDENTITY
            life = link.life actions = actions.sent() spawned = 0.0 alive = 0.0 health = 0.0 shield = 0.0
            lying = 0.0 holding = 0.0
        }
        if target {
            let npc = target.npc
            let body = npc.root.world_trs()
            let head = npc.head.trs()
            let left = npc.left.trs()
            let right = npc.right.trs()
            sample.root_pos = body.translation
            sample.root_rot = body.rotation_quat_xyzw
            sample.head_pos = head.translation
            sample.head_rot = head.rotation_quat_xyzw
            sample.left_pos = left.translation
            sample.left_rot = left.rotation_quat_xyzw
            sample.right_pos = right.translation
            sample.right_rot = right.rotation_quat_xyzw
            sample.spawned = 1.0
            if target.alive() { sample.alive = 1.0 }
            sample.health = target.health_fraction()
            sample.shield = target.shield_integrity()
            sample.lying = lying_code(target.lying())
            if target.armed() { sample.holding = 1.0 }
        }
        link.pose.write(sample)
    }

    fn receive_report(sample) {
        if sample.request > link.request_seen {
            link.request_seen = sample.request
            if sample.wanted > 0.5 { options.spawn() } else { options.remove() }
        }
        let target = options.current()
        if !target || sample.life != link.life { return }
        let points = Math.clamp(sample.points - link.points_received, 0.0, FULL)
        let shield = Math.clamp(sample.shield - link.shield_received, 0.0, 1000.0)
        if points <= 0.0 && shield <= 0.0 { return }
        link.points_received = Math.clamp(sample.points, link.points_received, 1000000.0)
        link.shield_received = Math.clamp(sample.shield, link.shield_received, 1000000.0)
        target.take_damage(points, shield)
    }

    // The host's pose goes out at 20 Hz while the NPC exists and once a
    // second otherwise; the client's report once a second and on each change.
    link.pump = on_global("FrameTick", fn(event) {
        link.clock = link.clock + event.dt_sec
        link.elapsed = link.elapsed + event.dt_sec
        let period = 1.0
        if host && options.current() { period = 0.05 }
        if link.elapsed < period { return }
        link.elapsed = 0.0
        if host { send_pose() } else { send_report() }
    })
    link.pump.bypass(true)

    fn disconnect() {
        link.generation = link.generation + 1
        link.pose = null
        link.peer = null
        link.pose_actions = 0.0
        link.points_received = 0.0
        link.shield_received = 0.0
        link.request_seen = 0.0
        link.report_life = 0.0
        link.points_sent = 0.0
        link.shield_sent = 0.0
        link.request = 0.0
        link.wanted = 0.0
        actions.disconnect()
        link.pump.bypass(true)
        // Without the host there is no NPC to show.
        remove_puppet()
    }
    fn connect(group, address) {
        disconnect()
        let generation = link.generation
        actions.connect(group, address)
        let pose_name = options.name + ".pose"
        let peer_name = options.name + ".peer"
        if host {
            link.pose = options.socket.publish(pose_name).to(address).type(pose_sample)
            let incoming = options.socket.subscribe(peer_name).from(address).type(peer_sample)
            group.attach(link.pose)
            group.attach(incoming)
            on(incoming, "UdpSample", fn(event) {
                if generation == link.generation { receive_report(event.value) }
            })
        } else {
            link.peer = options.socket.publish(peer_name).to(address).type(peer_sample)
            let incoming = options.socket.subscribe(pose_name).from(address).type(pose_sample)
            group.attach(link.peer)
            group.attach(incoming)
            on(incoming, "UdpSample", fn(event) {
                if generation == link.generation { receive_pose(event.value) }
            })
        }
        link.elapsed = 0.0
        link.pump.bypass(false)
    }

    // Host hooks. A new life clears what the client reported for the last.
    fn new_life() {
        link.life = link.life + 1.0
        link.points_received = 0.0
        link.shield_received = 0.0
    }
    fn spawned() {
        new_life()
        actions.push(SPAWN, 0.0, [0.0, 0.0, 0.0], IDENTITY)
        send_pose()
    }
    fn removed() {
        actions.push(REMOVE, 0.0, [0.0, 0.0, 0.0], IDENTITY)
        send_pose()
    }
    fn shot(pose, mode) { actions.push(SHOT, mode_code(mode), pose.translation, pose.rotation_quat_xyzw) }
    fn down(kind) { actions.push(DOWN, lying_code(kind), [0.0, 0.0, 0.0], IDENTITY) }
    fn respawned() {
        new_life()
        actions.push(RESPAWN, 0.0, [0.0, 0.0, 0.0], IDENTITY)
    }

    // Client: a local shot on the puppet takes its share of the shield here
    // at once (the flash), and both reach the host as cumulative damage.
    fn pistol_hit(hit) {
        let puppet = link.puppet
        if host || !puppet || !puppet.alive || link.report_life == 0.0 { return }
        let shield = hit.renderable.energy_shield
        let cue = 0.0
        if shield && shield == puppet.shield {
            shield.take_damage(SHIELD_PER_POINT)
            link.shield_sent = link.shield_sent + SHIELD_PER_POINT
            cue = 2.0
        } else if hit.renderable.player_hit_zone == puppet.player.hit_zone {
            link.points_sent = link.points_sent + 1.0
            cue = 1.0
        }
        if cue == 0.0 { return }
        if options["on_damage"] { options.on_damage(cue) }
        send_report()
    }
    // Client: ask the host to spawn (true) or remove (false) the NPC.
    fn request(wanted) {
        if host || !link.peer { return }
        link.request = link.request + 1.0
        link.wanted = 0.0
        if wanted { link.wanted = 1.0 }
        send_report()
    }

    return {
        root = root connect = connect disconnect = disconnect
        spawned = spawned removed = removed shot = shot down = down respawned = respawned
        pistol_hit = pistol_hit request = request
    }
}
