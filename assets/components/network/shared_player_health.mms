import { f32 } from "types"

let full = 10.0
let health_sample = { life: f32, points: f32, target_life: f32, damage: f32 }

// Two peers: each owns its local health and reports cumulative damage to the
// other. A new life rejects damage still in flight from before a picker reset.
export fn shared_player_health(socket) {
    let state = {
        local_bar = null remote_bar = null hit_zone = null
        outgoing = null life = 1.0 points = full received = 0.0
        remote_life = 0.0 remote_points = full sent = 0.0
    }

    fn send() {
        if state.outgoing && state.outgoing.ready() {
            state.outgoing.write({
                life = state.life points = state.points
                target_life = state.remote_life damage = state.sent
            })
        }
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
    }
    fn set_remote_points(points) {
        if points == state.remote_points { return }
        state.remote_points = points
        state.remote_bar.set_health(points / full)
    }
    fn reset() {
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
    fn pistol_hit(hit) { record_hit(hit.renderable.player_hit_zone, 1.0) }
    fn rocket_hit(hit) { record_hit(hit.other_zone, 5.0) }
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
        set_remote_points(full)
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
            if sample.life != state.remote_life {
                state.remote_life = sample.life
                state.sent = 0.0
            }
            set_remote_points(Math.clamp(Math.floor(sample.points), 0.0, full))
            if sample.target_life == state.life {
                let damage = Math.clamp(Math.floor(sample.damage), 0.0, full)
                if damage > state.received {
                    set_points(Math.clamp(state.points - (damage - state.received), 0.0, full))
                    state.received = damage
                    send()
                }
            }
        })
        heartbeat.loop_anim()
    }
    return {
        root = heartbeat bind = bind connect = connect disconnect = disconnect
        pistol_hit = pistol_hit rocket_hit = rocket_hit can_fire = can_fire reset = reset
    }
}
