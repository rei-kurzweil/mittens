import { make_rooftop_player } from "./characters/rooftop_player.mms"
import { energy_shield } from "./energy_sheild.mms"
import { player_death } from "./player_death.mms"
import { ship_machine_pistol } from "./weapons/ship_machine_pistol.mms"
import { pistol_grip } from "./poses/bisket/pistol_grip.mms"

// Local, single-player versions of the arena's combat rules
// (shared_player_health.mms): pistol hits take 1 of 10 points. Rocket
// splash within 2 m takes 4.9 points (49%) and 50 shield, so two close hits
// leave a sliver; out to 6 m it takes 25 shield only.
// Damage cues: 1 health, 2 shield, 3 both.
let FULL = 10.0
let INNER_BLAST = { radius = 2.0 shield = 50.0 health = 4.9 }
let OUTER_BLAST = { radius = 6.0 shield = 25.0 health = 0.0 }
// Animations run in beats; the scenes' Clock.bpm(120) is 2 beats a second.
let BEATS_PER_SECOND = 2.0
// The death burst (player_death at 2 beats per second) lasts six seconds.
let RESPAWN_SECONDS = 6.0
// The NPC alternates PHASE_SECONDS of semi-auto and of full auto. Each
// cycle it turns to the player (AIM_TURN_SECONDS), then pulls the trigger:
//   semi: one shot every SEMI_PERIOD_SECONDS, SEMI_POINTS if it lands
//   auto: AUTO_HOLD_SECONDS bursts AUTO_WAIT_SECONDS apart, AUTO_POINTS per
//         landed shot at the pistol's auto cadence
// A shot lands if the player is still within SHOT_HALF_WIDTH of the aimed
// line (the NPC does not track during a burst).
let PHASE_SECONDS = 30.0
let AIM_TURN_SECONDS = 0.6
// The turn plus a beat before the trigger.
let AIM_SECONDS = AIM_TURN_SECONDS + 0.1
let SEMI_PERIOD_SECONDS = 5.0
// Semi fires on a keyframe shortly after the press; release after this.
let SEMI_HOLD_SECONDS = 0.3
let SEMI_POINTS = 1.0
let AUTO_HOLD_SECONDS = 1.5
let AUTO_WAIT_SECONDS = 6.0
let AUTO_POINTS = 0.5
// laser_machine_pistol's auto cadence.
let AUTO_SHOT_SECONDS = 1.0 / (8.0 * 1.15)
let SHOT_HALF_WIDTH = 0.35
// The NPC's right hand, in head-target space: it pivots on the shoulder,
// a little short of full reach so the elbow bends down, and yaws a quarter
// turn so the palm faces left around the pistol grip.
let SHOULDER = [0.18, -0.24, -0.05]
let HAND_REACH = 0.40
let HAND_DROP = 0.05
let HAND_YAW = Math.pi / 2.0
// The physics NPC's wander: kick strength (m/s per sqrt second), leak back
// toward a stop (per second) and a speed cap (m/s). Typical speed is about
// WANDER_KICK / sqrt(2 * WANDER_LEAK), here ~0.8 m/s.
let WANDER_KICK = 1.0
let WANDER_LEAK = 0.8
let WANDER_MAX_SPEED = 1.5
// Out of its vehicle, the NPC walks back to the driver's door in half-second
// legs at walking pace, and gets in once this close.
let WALK_SPEED = 1.4
let WALK_STEP_SECONDS = 0.5
let BOARD_DISTANCE = 1.4

// A paused one-shot timer: play() it, and `seconds` later it emits `name`
// as a DataEvent on `events`. Timing runs natively, not in per-frame script.
fn after_seconds(events, seconds, name) {
    let beats = seconds * BEATS_PER_SECOND
    let timer = Animation.paused().length(beats) {
        // Animation time is relative to its earliest keyframe.
        Keyframe.at(0.0) {}
        Keyframe.at(beats) { emit_data(events, name) }
    }
    return timer
}

// Rotate v by the unit quaternion q (xyzw).
fn rotate(q, v) {
    let ix = q[3] * v[0] + q[1] * v[2] - q[2] * v[1]
    let iy = q[3] * v[1] + q[2] * v[0] - q[0] * v[2]
    let iz = q[3] * v[2] + q[0] * v[1] - q[1] * v[0]
    let iw = -q[0] * v[0] - q[1] * v[1] - q[2] * v[2]
    return [
        ix * q[3] - iw * q[0] - iy * q[2] + iz * q[1],
        iy * q[3] - iw * q[1] - iz * q[0] + ix * q[2],
        iz * q[3] - iw * q[2] - ix * q[1] + iy * q[0],
    ]
}

fn noise(value) {
    let raw = Math.sin(value * 12.9898 + 78.233) * 43758.5453
    return raw - Math.floor(raw)
}

// bar: a health_bar. center: the transform splash distance is measured from.
// on_damage(cue), on_death() and on_respawn() are optional.
export fn practice_health(bar, center, options) {
    let state = { points = FULL }
    fn callback(name) { if options { return options[name] } return null }
    let events = T {}
    let revive = after_seconds(events, RESPAWN_SECONDS, "Revive")
    let timers = T { events revive }
    timers
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
            revive.play()
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
    on(events, "DataEvent", fn(event) {
        if event != "Revive" { return }
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
//   player       a transform to aim at: the NPC turns to face it and fires
//                a machine pistol held in its right hand, alternating 30 s
//                of single shots every 5 s with 30 s of 1.5 s auto bursts
//   on_player_hit(points)  called for each shot that lands on the player
//   on_down()    called each time the NPC goes down
//   health_view (the bar faces it), on_damage(cue)
//   seated       sit (Mitten's floor-seated pose, no leg IK) instead of
//                standing, e.g. in a vehicle seat; position is then the root
//                in the seat's parent space
//   vehicle      { chassis, seat, door } with seated: the caller attaches
//                root to chassis at seat (chassis space). Going down ejects
//                the NPC beside the door (chassis space; on the ground);
//                after respawning it walks back to the door, which may be
//                moving, and gets back in, aiming and firing throughout
// The NPC's root may ride a moving, turned parent (a truck's chassis): it
// aims in world space and turns relative to that parent.
export fn practice_target(options) {
    let physics = options["physics"] == true
    let npc = make_rooftop_player({
        prefix = options.prefix local_player = false avatar = options.avatar
        use_xr = options.use_xr use_desktop = options.use_desktop opaque_bucket = "near"
        show_health = true health_view = options["health_view"] shield_mount = true
        // The untracked left arm rests in the relaxed pose.
        physics = physics relaxed_pose = true seated_pose = options["seated"] == true
    })
    let strafe = 2.5
    if options["strafe"] != null { strafe = options.strafe }
    let speed = 1.2
    if options["speed"] != null { speed = options.speed }
    let on_damage = options["on_damage"]
    let player = options["player"]
    let vehicle = options["vehicle"]
    // yaw is the head's heading relative to the NPC's parent; aim_yaw is the
    // world heading of the last aim, for the shot check; frame is the NPC's
    // root frame once built, whose parent may turn.
    let state = { shield = null time = 0.0 yaw = options.yaw aim_yaw = options.yaw pitch = 0.0 steps = 0
        walk_x = 0.0 walk_z = 0.0 frame = null seated = false }
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
    let hooks = { death = null respawn = null }
    let health = practice_health(npc.health, npc.shield_mount, {
        on_damage = on_damage
        on_death = fn() {
            drop_shield()
            if hooks.death { hooks.death() }
            if options["on_down"] { options.on_down() }
        }
        on_respawn = fn() {
            wear_shield()
            if hooks.respawn { hooks.respawn() }
        }
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
    // Up/down aim: the right hand swings on the shoulder by `pitch` (positive
    // is up) and tilts with it, tweened like the turn.
    let raise = Transition { duration_beats(AIM_TURN_SECONDS * 2.0) ease_in_out_sine() replace_same_target() }
    npc.right.attach(raise)
    fn aim_hand(pitch) {
        state.pitch = pitch
        let translation = [
            SHOULDER[0],
            SHOULDER[1] + Math.sin(pitch) * HAND_REACH - HAND_DROP,
            SHOULDER[2] - Math.cos(pitch) * HAND_REACH,
        ]
        // Pitch about the head's X after the palm-left yaw: qx(pitch) * qy(yaw).
        let sa = Math.sin(pitch / 2.0)
        let ca = Math.cos(pitch / 2.0)
        let sb = Math.sin(HAND_YAW / 2.0)
        let cb = Math.cos(HAND_YAW / 2.0)
        npc.right.trs({
            translation = translation
            rotation_quat_xyzw = [sa * cb, ca * sb, sa * sb, ca * cb]
            scale = [1.0, 1.0, 1.0]
        })
    }

    // The right arm is held out with the pistol; the left arm relaxes.
    let gun = null
    if player {
        npc.left_driver.set_pose_valid(false)
        npc.right_driver.set_pose_valid(true)
        aim_hand(0.0)
        on(npc.model, "GLTFInitialized", fn(event) { pistol_grip(event.gltf, "Right") })
        gun = ship_machine_pistol(options.prefix + "_pistol", {
            opaque_bucket = "near" ammunition = 1000000 can_fire = fn(user) { return health.alive() }
        })
        // The player can't take it out of the NPC's hand.
        for grab in gun.grip.query_all("grabbable") { grab.remove_subtree() }
        let gun_mount = T.position(0.0, 0.0, -0.105) {
            name = options.prefix + "_gun_mount"
            gun
        }
        npc.right.attach(gun_mount)
    }
    // Shots leave from about shoulder height under the head.
    fn shoulder_world() {
        let h = npc.head.world_trs().translation
        return [h[0], h[1] + SHOULDER[1], h[2]]
    }
    // The heading of the NPC's frame in the world (its parent's turn).
    fn frame_yaw() {
        if !state.frame { return 0.0 }
        let q = state.frame.world_trs().rotation_quat_xyzw
        return 2.0 * Math.atan2(q[1], q[3])
    }
    // Turn and raise the pistol toward the player, once per aim.
    fn aim_at_player() {
        let s = shoulder_world()
        let p = player.world_trs().translation
        let dx = p[0] - s[0]
        let dz = p[2] - s[2]
        state.aim_yaw = Math.atan2(-dx, -dz)
        face(state.aim_yaw - frame_yaw())
        aim_hand(Math.atan2(p[1] - s[1], Math.sqrt(dx * dx + dz * dz)))
    }
    // A shot along the aimed line lands if the player is still near it.
    fn shot_lands() {
        let s = shoulder_world()
        let p = player.world_trs().translation
        let d = [p[0] - s[0], p[1] - s[1], p[2] - s[2]]
        let c = Math.cos(state.pitch)
        let f = [-Math.sin(state.aim_yaw) * c, Math.sin(state.pitch), -Math.cos(state.aim_yaw) * c]
        let along = d[0] * f[0] + d[1] * f[1] + d[2] * f[2]
        let off = [d[0] - f[0] * along, d[1] - f[1] * along, d[2] - f[2] * along]
        let miss = Math.sqrt(off[0] * off[0] + off[1] * off[1] + off[2] * off[2])
        return along > 0.0 && miss < SHOT_HALF_WIDTH
    }

    // Combat activity state machine, timed by animations (no per-frame
    // script): each activity's timer emits a DataEvent when it ends and the
    // machine enters `next`:
    //   wait -> aim -> fire -> wait ...    (down while the NPC is dead)
    // Entering an activity performs its one action: aim turns to the player,
    // fire presses the trigger, leaving fire releases it. A looping 30 s
    // phase animation flips semi/auto; the flip applies when a wait begins,
    // never mid-burst.
    let combat = { activity = "down" next = "wait" mode = "semi" flip = false }
    let events = T {}
    let timers = { wait = null aim = null fire = null }
    let clocks = null
    if gun {
        timers.wait = {
            semi = after_seconds(events, SEMI_PERIOD_SECONDS - AIM_SECONDS - SEMI_HOLD_SECONDS, "WaitDone")
            auto = after_seconds(events, AUTO_WAIT_SECONDS, "WaitDone")
        }
        timers.aim = after_seconds(events, AIM_SECONDS, "AimDone")
        timers.fire = { semi = after_seconds(events, SEMI_HOLD_SECONDS, "FireDone") }
        // An auto burst checks each shot at the pistol's cadence.
        let burst_beats = AUTO_HOLD_SECONDS * BEATS_PER_SECOND
        let shot_beats = AUTO_SHOT_SECONDS * BEATS_PER_SECOND
        let burst = Animation.paused().length(burst_beats) {
            Keyframe.at(0.0) {}
            for shot in range(Math.floor(AUTO_HOLD_SECONDS / AUTO_SHOT_SECONDS)) {
                Keyframe.at((shot + 1) * shot_beats) { emit_data(events, "Shot") }
            }
            Keyframe.at(burst_beats) { emit_data(events, "FireDone") }
        }
        timers.fire.auto = burst
        let phase = Animation.looping().length(PHASE_SECONDS * BEATS_PER_SECOND) {
            Keyframe.at(0.0) {}
            Keyframe.at(PHASE_SECONDS * BEATS_PER_SECOND) { emit_data(events, "PhaseFlip") }
        }
        let timer_root = T {
            name = options.prefix + "_combat_timers"
            events timers.wait.semi timers.wait.auto timers.aim timers.fire.semi burst phase
        }
        clocks = timer_root
    }
    fn check_shot() {
        let points = SEMI_POINTS
        if combat.mode == "auto" { points = AUTO_POINTS }
        if shot_lands() && options["on_player_hit"] { options.on_player_hit(points) }
    }
    fn stop_timers() {
        timers.wait.semi.pause()
        timers.wait.auto.pause()
        timers.aim.pause()
        timers.fire.semi.pause()
        timers.fire.auto.pause()
    }
    fn enter(activity) {
        if combat.activity == "fire" { gun.fire_end() }
        combat.activity = activity
        if activity == "wait" {
            if combat.flip {
                combat.flip = false
                if combat.mode == "semi" { combat.mode = "auto" } else { combat.mode = "semi" }
                gun.set_mode(combat.mode)
            }
            combat.next = "aim"
            timers.wait[combat.mode].play()
        }
        if activity == "aim" {
            aim_at_player()
            combat.next = "fire"
            timers.aim.play()
        }
        if activity == "fire" {
            gun.fire_start()
            combat.next = "wait"
            // The first shot leaves with the press; auto keyframes check the rest.
            check_shot()
            timers.fire[combat.mode].play()
        }
        if activity == "down" {
            stop_timers()
            combat.next = "wait"
        }
    }
    on(events, "DataEvent", fn(event) {
        if combat.activity == "down" { return }
        if event == "PhaseFlip" { combat.flip = true }
        if event == "Shot" && combat.activity == "fire" { check_shot() }
        if event == "WaitDone" || event == "AimDone" || event == "FireDone" { enter(combat.next) }
    })
    if gun {
        hooks.death = fn() { enter("down") }
        hooks.respawn = fn() { enter("wait") }
        enter("wait")
    }

    // Vehicle riding (see the vehicle option): eject on death, walk back to
    // the door after respawning, then get in. Walking tweens the NPC's root
    // inside its frame, so the walking IK steps the legs, and getting in
    // tweens it back into the seat.
    fn ride_vehicle(ride) {
        state.seated = true
        let ground = T { name = options.prefix + "_ground" }
        ground
        let walk = Transition { duration_beats(WALK_STEP_SECONDS * BEATS_PER_SECOND) replace_same_target() }
        npc.root.attach(walk)
        fn chassis_point(local) {
            let pose = ride.chassis.world_trs()
            let r = rotate(pose.rotation_quat_xyzw, local)
            return [pose.translation[0] + r[0], pose.translation[1] + r[1], pose.translation[2] + r[2]]
        }
        fn eject() {
            if !state.seated { return }
            state.seated = false
            let door = chassis_point(ride.door)
            ground.attach(state.frame)
            state.frame.update_transform([door[0], 0.0, door[2]], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
            npc.root.update_transform([0.0, 0.0, 0.0])
            npc.set_legs_enabled(true)
        }
        fn board() {
            state.seated = true
            ride.chassis.attach(state.frame)
            state.frame.update_transform(ride.seat, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
            npc.root.update_transform([0.0, 0.0, 0.0])
            npc.set_legs_enabled(false)
        }
        fn approach() {
            if state.seated || !health.alive() { return }
            let door = chassis_point(ride.door)
            let here = npc.root.world_trs().translation
            let dx = door[0] - here[0]
            let dz = door[2] - here[2]
            let d = Math.sqrt(dx * dx + dz * dz)
            if d < BOARD_DISTANCE {
                board()
                return
            }
            let step = Math.clamp(d, 0.0, WALK_SPEED * WALK_STEP_SECONDS)
            let base = state.frame.world_trs().translation
            npc.root.update_transform([here[0] + dx / d * step - base[0], 0.0, here[2] + dz / d * step - base[2]])
        }
        let walk_events = T { name = options.prefix + "_walk_events" }
        let walking = Animation.looping().length(WALK_STEP_SECONDS * BEATS_PER_SECOND) {
            Keyframe.at(0.0) {}
            Keyframe.at(WALK_STEP_SECONDS * BEATS_PER_SECOND) { emit_data(walk_events, "Approach") }
        }
        ground.attach(walk_events)
        ground.attach(walking)
        on(walk_events, "DataEvent", fn(event) { if event == "Approach" { approach() } })
        let on_death = hooks.death
        hooks.death = fn() {
            if on_death { on_death() }
            eject()
        }
    }

    let p = options.position
    let frame = T.position(p[0], p[1], p[2]) {
        name = options.prefix + "_frame"
        npc.locomotion
        if clocks { clocks }
    }
    state.frame = frame
    if vehicle { ride_vehicle(vehicle) }
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
        } else if alive && !vehicle {
            // Strafe back and forth along X.
            state.time = state.time + dt
            npc.root.update_transform([strafe * Math.sin(state.time * speed), 0.0, 0.0])
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
