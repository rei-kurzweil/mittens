import { f32 } from "types"

let full = 10.0
let health_sample = {
    life: f32, points: f32, target_life: f32, damage: f32, shield: f32, shield_damage: f32,
    cue_revision: f32, cue_kind: f32
}

// Two peers: each owns its local health and reports cumulative damage to the
// other. A new life rejects damage still in flight from before a picker reset.
export fn shared_player_health(socket) {
    let state = {
        local_bar = null remote_bar = null hit_zone = null shields = null on_death = null on_damage = null
        outgoing = null life = 1.0 points = full received = 0.0
        remote_life = 0.0 remote_points = full sent = 0.0
        shield_sent_key = 0.0 shield_sent = 0.0 shield_received_key = 0.0 shield_received = 0.0
        cue_revision = 0.0 cue_kind = 0.0 peer_cue_revision = 0.0
    }

    fn send() {
        if state.outgoing && state.outgoing.ready() {
            state.outgoing.write({
                life = state.life points = state.points
                target_life = state.remote_life damage = state.sent
                shield = state.shield_sent_key shield_damage = state.shield_sent
                cue_revision = state.cue_revision cue_kind = state.cue_kind
            })
        }
    }
    fn announce_damage(health_hit, shield_hit) {
        if !health_hit && !shield_hit { return }
        let kind = 2.0
        if health_hit {
            kind = 1.0
            if shield_hit { kind = 3.0 }
        }
        state.cue_revision = state.cue_revision + 1.0
        state.cue_kind = kind
        if state.on_damage { state.on_damage(kind) }
        send()
    }
    // Replace lost samples with the current state. Changes also send immediately.
    let heartbeat = Animation.paused().length(1.0) {
        name = "arena_health_heartbeat"
        Keyframe.at(0.0) { send() }
    }
    fn set_points(points) {
        if points == state.points { return }
        state.points = points
        state.local_bar.set_health(points / full)
        if points == 0.0 {
            if state.shields { state.shields.drop_local() }
            if state.on_death { state.on_death(true) }
        }
    }
    fn set_remote_points(points, same_life) {
        if points == state.remote_points { return }
        state.remote_points = points
        state.remote_bar.set_health(points / full)
        if points == 0.0 && same_life && state.on_death { state.on_death(false) }
    }
    fn reset() {
        if state.shields { state.shields.drop_local() }
        state.life = state.life + 1.0
        state.received = 0.0
        set_points(full)
        send()
    }
    fn record_hit(zone, damage) {
        if !zone || zone != state.hit_zone || state.remote_life == 0.0 || state.remote_points <= 0.0 { return }
        let sent = Math.clamp(state.sent + damage, 0.0, full)
        if sent == state.sent { return }
        state.sent = sent
        send()
    }
    fn pistol_hit(hit) {
        let shield = hit.renderable.energy_shield
        if shield && state.shields && shield == state.shields.remote_equipped() {
            shield.take_damage(10.0)
            let key = shield.wear_key()
            if key != state.shield_sent_key {
                state.shield_sent_key = key
                state.shield_sent = 0.0
            }
            state.shield_sent = state.shield_sent + 10.0
            send()
            return
        }
        record_hit(hit.renderable.player_hit_zone, 1.0)
    }
    fn rocket_hit(hit) { record_hit(hit.other_zone, 5.0) }
    fn within_blast(point, mount) {
        let center = mount.world_trs().translation
        let x = point[0] - center[0]
        let y = point[1] - center[1]
        let z = point[2] - center[2]
        return x * x + y * y + z * z <= 1.5 * 1.5
    }
    // Shared rocket impacts already replay once on each peer. Each peer
    // computes splash against its own player and updates the visible shields.
    fn rocket_splash(point) {
        if !state.shields { return }
        if state.points > 0.0 && within_blast(point, state.shields.local_mount) {
            let shield = state.shields.local_equipped()
            let shield_hit = shield && shield.integrity() > 0.0
            if shield { shield.take_damage(50.0) }
            set_points(Math.clamp(state.points - 5.0, 0.0, full))
            announce_damage(true, shield_hit)
        }
        let peer_shield = state.shields.remote_equipped()
        if peer_shield && within_blast(point, state.shields.remote_mount) { peer_shield.take_damage(50.0) }
    }
    fn receive_shield(sample) {
        if !state.shields || sample.shield == 0.0 { return false }
        let shield = state.shields.local_equipped()
        if !shield || shield.wear_key() != sample.shield { return false }
        if sample.shield != state.shield_received_key {
            state.shield_received_key = sample.shield
            state.shield_received = 0.0
        }
        if sample.shield_damage > state.shield_received {
            let was_blocking = shield.integrity() > 0.0
            shield.take_damage(sample.shield_damage - state.shield_received)
            state.shield_received = sample.shield_damage
            return was_blocking
        }
        return false
    }
    fn can_fire(user) { return state.points > 0.0 }
    fn bind(local, remote, opponent_zone) {
        state.local_bar = local
        state.remote_bar = remote
        state.hit_zone = opponent_zone
    }
    fn disconnect() {
        state.outgoing = null
        state.remote_life = 0.0
        state.sent = 0.0
        state.received = 0.0
        state.shield_sent_key = 0.0
        state.shield_sent = 0.0
        state.shield_received_key = 0.0
        state.shield_received = 0.0
        state.peer_cue_revision = 0.0
        set_remote_points(full, false)
        heartbeat.pause()
    }
    fn connect(group, address) {
        disconnect()
        let outgoing = socket.publish("arena.player.health").to(address).type(health_sample)
        let incoming = socket.subscribe("arena.player.health").from(address).type(health_sample)
        group.attach(outgoing)
        group.attach(incoming)
        state.outgoing = outgoing
        on(incoming, "UdpSample", fn(event) {
            if state.outgoing != outgoing { return }
            let sample = event.value
            if sample.life < state.remote_life || sample.life < 1.0 { return }
            let known_peer = state.remote_life > 0.0
            let same_life = sample.life == state.remote_life
            if !same_life {
                state.remote_life = sample.life
                state.sent = 0.0
            }
            set_remote_points(Math.clamp(Math.floor(sample.points), 0.0, full), same_life)
            if sample.cue_revision > state.peer_cue_revision {
                state.peer_cue_revision = sample.cue_revision
                if known_peer && state.on_damage { state.on_damage(sample.cue_kind) }
            }
            if sample.target_life == state.life {
                let shield_hit = receive_shield(sample)
                let damage = Math.clamp(Math.floor(sample.damage), 0.0, full)
                let health_hit = false
                if damage > state.received {
                    let points = Math.clamp(state.points - (damage - state.received), 0.0, full)
                    health_hit = points < state.points
                    set_points(points)
                    state.received = damage
                }
                announce_damage(health_hit, shield_hit)
            }
        })
        heartbeat.loop_anim()
    }
    return {
        root = heartbeat bind = bind connect = connect disconnect = disconnect
        bind_shields = fn(shields) { state.shields = shields }
        bind_death = fn(callback) { state.on_death = callback }
        bind_damage = fn(callback) { state.on_damage = callback }
        pistol_hit = pistol_hit rocket_hit = rocket_hit rocket_splash = rocket_splash can_fire = can_fire reset = reset
    }
}
