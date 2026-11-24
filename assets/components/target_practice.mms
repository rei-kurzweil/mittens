import { make_rooftop_player } from "./characters/rooftop_player.mms"
import { energy_shield } from "./energy_sheild.mms"
import { player_death } from "./player_death.mms"
import { ship_machine_pistol } from "./weapons/ship_machine_pistol.mms"

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
// The NPC turns to the player every AIM_PERIOD_SECONDS, taking
// AIM_TURN_SECONDS, then fires; it hits if the player is still within
// SHOT_HALF_WIDTH of the aimed line.
let AIM_PERIOD_SECONDS = 5.0
let AIM_TURN_SECONDS = 0.6
let SHOT_HALF_WIDTH = 0.35
// The physics NPC's wander: kick strength (m/s per sqrt second), leak back
// toward a stop (per second) and a speed cap (m/s). Typical speed is about
// WANDER_KICK / sqrt(2 * WANDER_LEAK), here ~0.8 m/s.
let WANDER_KICK = 1.0
let WANDER_LEAK = 0.8
let WANDER_MAX_SPEED = 1.5

fn noise(value) {
    let raw = Math.sin(value * 12.9898 + 78.233) * 43758.5453
    return raw - Math.floor(raw)
}

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

// A rooftop player that wanders (or, without physics, strafes left and
// right), wears an energy shield and takes pistol and rocket damage. It drops the shield when it goes down and
// comes back with a fresh one.
// Required: prefix, avatar, use_xr, use_desktop, position, yaw (the head's
// starting yaw; 0 faces -Z).
// Optional:
//   physics      walk under gravity with a sliding capsule, wandering in a
//                brown-noise pattern; it may wander off the edge (default false)
//   strafe       without physics: metres either side of position (default 2.5)
//   speed        without physics: radians per second of the sweep (default 1.2)
//   player       a transform to aim at: every 5 s the NPC turns to face it
//                and fires a machine pistol held in its right hand
//   on_player_hit()  called when a shot lands within 0.35 m of the player
//   health_view (the bar faces it), on_damage(cue)
export fn practice_target(options) {
    let physics = options["physics"] == true
    let npc = make_rooftop_player({
        prefix = options.prefix local_player = false avatar = options.avatar
        use_xr = options.use_xr use_desktop = options.use_desktop opaque_bucket = "near"
        show_health = true health_view = options["health_view"] shield_mount = true
        // The untracked left arm rests in the relaxed pose.
        physics = physics relaxed_pose = true
    })
    let strafe = 2.5
    if options["strafe"] != null { strafe = options.strafe }
    let speed = 1.2
    if options["speed"] != null { speed = options.speed }
    let on_damage = options["on_damage"]
    let player = options["player"]
    let state = { shield = null time = 0.0 yaw = options.yaw cycle = 0.0 firing = false
        steps = 0 walk_x = 0.0 walk_z = 0.0 }
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

    // Turning: the head target carries the whole rig (body, both arms and
    // the pistol). One update_transform per aim; the Transition tweens it.
    let turn = Transition { duration_beats(AIM_TURN_SECONDS * 2.0) ease_in_out_sine() replace_same_target() }
    npc.head.attach(turn)
    fn face(yaw) {
        // Take the short way round from the current heading.
        if yaw - state.yaw > Math.pi { yaw = yaw - 2.0 * Math.pi }
        if state.yaw - yaw > Math.pi { yaw = yaw + 2.0 * Math.pi }
        state.yaw = yaw
        npc.head.update_transform(npc.head.trs().translation, [0.0, yaw, 0.0], [1.0, 1.0, 1.0])
    }
    face(options.yaw)

    // The right arm is held out with the pistol; the left arm relaxes.
    let gun = null
    if player {
        npc.left_driver.set_pose_valid(false)
        npc.right_driver.set_pose_valid(true)
        npc.right.update_transform([0.2, -0.25, -0.45], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
        gun = ship_machine_pistol(options.prefix + "_pistol", {
            opaque_bucket = "near" ammunition = 1000000 can_fire = fn(user) { return health.alive() }
        })
        // The player can't take it out of the NPC's hand.
        for grab in gun.grip.query_all("grabbable") { grab.remove_subtree() }
        // The grip's axes are a quarter turn off the hand target's: yaw the
        // pistol left so it points where the NPC faces.
        let gun_mount = T.position(0.0, 0.0, -0.105).rotation(0.0, Math.pi / 2.0, 0.0) {
            name = options.prefix + "_gun_mount"
            gun
        }
        npc.right.attach(gun_mount)
    }
    fn heading_to_player() {
        let h = npc.head.world_trs().translation
        let p = player.world_trs().translation
        return Math.atan2(-(p[0] - h[0]), -(p[2] - h[2]))
    }
    // A shot along the aimed heading lands if the player is still near it.
    fn shot_lands() {
        let h = npc.head.world_trs().translation
        let p = player.world_trs().translation
        let dx = p[0] - h[0]
        let dz = p[2] - h[2]
        let fx = -Math.sin(state.yaw)
        let fz = -Math.cos(state.yaw)
        let along = dx * fx + dz * fz
        let lateral = Math.abs(dx * fz - dz * fx)
        return along > 0.0 && lateral < SHOT_HALF_WIDTH
    }

    let p = options.position
    let frame = T.position(p[0], p[1], p[2]) {
        name = options.prefix + "_frame"
        npc.locomotion
    }
    on_global("FrameTick", fn(event) {
        let dt = event.dt_sec
        let alive = health.alive()
        if physics {
            // Brown-noise wander: random kicks integrate into the walking
            // velocity, which leaks back toward a stop so it stays bounded.
            // Hold still while down.
            state.steps = state.steps + 1
            let kick = WANDER_KICK * Math.sqrt(dt)
            let white_x = 2.0 * noise(state.steps * 2.0) - 1.0
            let white_z = 2.0 * noise(state.steps * 2.0 + 1.0) - 1.0
            state.walk_x = state.walk_x + white_x * kick - state.walk_x * WANDER_LEAK * dt
            state.walk_z = state.walk_z + white_z * kick - state.walk_z * WANDER_LEAK * dt
            let walk_speed = Math.sqrt(state.walk_x * state.walk_x + state.walk_z * state.walk_z)
            if walk_speed > WANDER_MAX_SPEED {
                state.walk_x = state.walk_x * WANDER_MAX_SPEED / walk_speed
                state.walk_z = state.walk_z * WANDER_MAX_SPEED / walk_speed
            }
            let vx = 0.0
            let vz = 0.0
            if alive {
                vx = state.walk_x
                vz = state.walk_z
            }
            // translate adds velocity: steer toward the walk, leave Y to gravity.
            let linear = npc.velocity.linear()
            npc.velocity.translate([vx - linear[0], 0.0, vz - linear[2]])
        } else if alive {
            // Strafe back and forth along X.
            state.time = state.time + dt
            npc.root.update_transform([strafe * Math.sin(state.time * speed), 0.0, 0.0])
        }
        // Every five seconds: turn to the player, then fire once.
        if !gun { return }
        if !alive {
            state.cycle = 0.0
            if state.firing { gun.fire_end() state.firing = false }
            return
        }
        let previous = state.cycle
        state.cycle = state.cycle + dt
        if previous < AIM_PERIOD_SECONDS && state.cycle >= AIM_PERIOD_SECONDS {
            state.cycle = 0.0
            face(heading_to_player())
            return
        }
        let fire_at = AIM_TURN_SECONDS + 0.1
        if previous < fire_at && state.cycle >= fire_at {
            gun.fire_start()
            state.firing = true
            if shot_lands() && options["on_player_hit"] { options.on_player_hit() }
        }
        if state.firing && state.cycle >= fire_at + 0.3 {
            gun.fire_end()
            state.firing = false
        }
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
