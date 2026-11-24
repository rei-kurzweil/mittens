import { make_rooftop_player } from "./characters/rooftop_player.mms"
import { energy_shield } from "./energy_sheild.mms"
import { player_death } from "./player_death.mms"

// Local, single-player versions of the arena's combat rules
// (shared_player_health.mms): pistol hits take 1 of 10 points. Rocket
// splash within 2 m takes 4.9 points (49%) and 50 shield, so two close hits
// leave a sliver; out to 6 m it takes 25 shield only.
// Damage cues: 1 health, 2 shield, 3 both.
let FULL = 10.0
let INNER_BLAST = { radius = 2.0 shield = 50.0 health = 4.9 }
let OUTER_BLAST = { radius = 6.0 shield = 25.0 health = 0.0 }
// The death burst (player_death at 2 beats per second) lasts six seconds.
let RESPAWN_SECONDS = 6.0

// bar: a health_bar. center: the transform splash distance is measured from.
// on_damage(cue), on_death() and on_respawn() are optional.
export fn practice_health(bar, center, options) {
    let state = { points = FULL down = 0.0 }
    fn callback(name) { if options { return options[name] } return null }
    fn alive() { return state.points > 0.0 }
    fn damage(amount, cue) {
        if !alive() || amount <= 0.0 { return }
        state.points = Math.clamp(state.points - amount, 0.0, FULL)
        // Fractional damage can leave a rounding sliver; treat it as zero.
        if state.points < 0.001 { state.points = 0.0 }
        bar.set_health(state.points / FULL)
        let on_damage = callback("on_damage")
        if on_damage { on_damage(cue) }
        if state.points == 0.0 {
            state.down = RESPAWN_SECONDS
            player_death(center.world_trs().translation, 2.0)
            let on_death = callback("on_death")
            if on_death { on_death() }
        }
    }
    // The rocket blast ring (its shield and health damage) reaching center
    // from an explosion at point, or null beyond the outer ring.
    fn blast_at(point) {
        let c = center.world_trs().translation
        let x = point[0] - c[0]
        let y = point[1] - c[1]
        let z = point[2] - c[2]
        let d2 = x * x + y * y + z * z
        if d2 <= INNER_BLAST.radius * INNER_BLAST.radius { return INNER_BLAST }
        if d2 <= OUTER_BLAST.radius * OUTER_BLAST.radius { return OUTER_BLAST }
        return null
    }
    on_global("FrameTick", fn(event) {
        if state.down <= 0.0 { return }
        state.down = state.down - event.dt_sec
        if state.down > 0.0 { return }
        state.points = FULL
        bar.set_health(1.0)
        let on_respawn = callback("on_respawn")
        if on_respawn { on_respawn() }
    })
    return { alive = alive damage = damage blast_at = blast_at }
}

// A rooftop player that strafes left and right, wears an energy shield and
// takes pistol and rocket damage. It drops the shield when it goes down and
// comes back with a fresh one.
// Required: prefix, avatar, use_xr, use_desktop, position, yaw.
// Optional: strafe (metres either side, default 2.5), speed (radians per
// second, default 0.8), health_view (the bar faces it), on_damage(cue).
export fn practice_target(options) {
    let npc = make_rooftop_player({
        prefix = options.prefix local_player = false avatar = options.avatar
        use_xr = options.use_xr use_desktop = options.use_desktop opaque_bucket = "near"
        show_health = true health_view = options["health_view"] shield_mount = true
    })
    let strafe = 2.5
    if options["strafe"] != null { strafe = options.strafe }
    let speed = 0.8
    if options["speed"] != null { speed = options.speed }
    let on_damage = options["on_damage"]
    let state = { shield = null time = 0.0 }
    fn drop_shield() {
        if state.shield { state.shield.remove_subtree() }
        state.shield = null
    }
    fn wear_shield() {
        drop_shield()
        let shield = energy_shield(options.prefix + "_shield", 2.0)
        npc.shield_mount.attach(shield)
        shield.set_worn(true, true)
        state.shield = shield
    }
    let health = practice_health(npc.health, npc.shield_mount, {
        on_damage = on_damage on_death = drop_shield on_respawn = wear_shield
    })
    wear_shield()

    fn pistol_hit(hit) {
        if !health.alive() { return }
        let shield = hit.renderable.energy_shield
        if shield && shield == state.shield {
            shield.take_damage(10.0)
            if on_damage { on_damage(2.0) }
            return
        }
        if hit.renderable.player_hit_zone == npc.hit_zone { health.damage(1.0, 1.0) }
    }
    fn rocket_splash(point) {
        let blast = health.blast_at(point)
        if !health.alive() || !blast { return }
        let shield = state.shield
        let shield_hit = shield && shield.integrity() > 0.0
        if shield { shield.take_damage(blast.shield) }
        if blast.health > 0.0 {
            let cue = 1.0
            if shield_hit { cue = 3.0 }
            health.damage(blast.health, cue)
        } else if shield_hit && on_damage {
            on_damage(2.0)
        }
    }

    let p = options.position
    let frame = T.position(p[0], p[1], p[2]).rotation(0.0, options.yaw, 0.0) {
        name = options.prefix + "_frame"
        npc.root
    }
    // Strafe along the frame's X; hold still while down.
    on_global("FrameTick", fn(event) {
        if !health.alive() { return }
        state.time = state.time + event.dt_sec
        npc.root.update_transform([strafe * Math.sin(state.time * speed), 0.0, 0.0])
    })
    return {
        root = frame npc = npc pistol_hit = pistol_hit rocket_splash = rocket_splash alive = health.alive
        center = fn() { return npc.shield_mount.world_trs().translation }
        shield_integrity = fn() {
            if state.shield { return state.shield.integrity() }
            return 0.0
        }
    }
}
