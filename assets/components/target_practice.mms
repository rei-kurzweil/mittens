import { make_rooftop_player } from "./characters/rooftop_player.mms"
import { energy_shield } from "./energy_sheild.mms"
import { player_death } from "./player_death.mms"
import { ship_machine_pistol } from "./weapons/ship_machine_pistol.mms"
import { after_seconds, practice_combat } from "./characters/target_combat.mms"
import { mitten_weapon_slot } from "./characters/mitten_equipment.mms"
import { mitten_driver } from "./characters/mitten_driver.mms"
import { mitten_right_wrist_basis, pistol_grip } from "./poses/bisket/pistol_grip.mms"
import { seated_lower_body_pose } from "./poses/mitten/floor-seated.pose.mms"
import { lie_down, get_up } from "./characters/downed.mms"
import { secondary_motion_visibility } from "./characters/secondary_motion_visibility.mms"

// Local, single-player versions of the arena's combat rules
// (shared_player_health.mms): pistol hits take 1 of 10 points. Rocket
// splash within 2 m takes 4.9 points (49%) and 50 shield, so two close hits
// leave a sliver; out to 6 m it takes 25 shield only.
// Damage cues: 1 health, 2 shield, 3 both.
let FULL = 10.0
let INNER_BLAST = { radius = 2.0 shield = 50.0 health = 4.9 }
let OUTER_BLAST = { radius = 6.0 shield = 25.0 health = 0.0 }
// A pistol point's worth of shield, as on Mitten's own shield.
let SHIELD_PER_POINT = 10.0
// Players are down for 30 s and NPCs for a minute (the death burst,
// player_death at 2 beats per second, lasts the first six), lying on the
// ground (downed.mms). practice_health's respawn_seconds option overrides it.
let RESPAWN_SECONDS = 30.0
let NPC_RESPAWN_SECONDS = 60.0
let AIM_TURN_SECONDS = 0.6
let SHOT_HALF_WIDTH = 0.35
// A solid this close in front of the player's chest is the player's own
// body or hitbox, not cover.
let COVER_MARGIN = 0.6
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
// Distance from the driver's door at which the NPC can get back in.
let BOARD_DISTANCE = 1.4
// How close an unarmed NPC gets to the player before taking their weapon.
let TAKE_REACH = 0.9
let RUN_SPEED = 2.28

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
// on_damage(cue), on_death(), on_respawn() and respawn_seconds are optional.
// hit(points) is a shot that lands and blast(point) a rocket explosion; with
// a charged energy shield worn (wear_shields(source), source() returning the
// worn shield or null) shots drain the shield instead of health, and a blast
// takes its shield share from the shield while its health share always gets
// through. damage(amount, cue) is health damage itself.
export fn practice_health(bar, center, options) {
    let state = { points = FULL shields = null }
    fn callback(name) { if options { return options[name] } return null }
    fn cue(value) {
        let on_damage = callback("on_damage")
        if on_damage { on_damage(value) }
    }
    fn worn_shield() {
        if !state.shields { return null }
        let shield = state.shields()
        if shield && shield.integrity() > 0.0 { return shield }
        return null
    }
    let events = T {}
    let respawn_seconds = RESPAWN_SECONDS
    if options && options["respawn_seconds"] != null { respawn_seconds = options.respawn_seconds }
    let revive = after_seconds(events, respawn_seconds, "Revive")
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
    fn hit(points) {
        if !alive() || points <= 0.0 { return }
        let shield = worn_shield()
        if shield {
            shield.take_damage(points * SHIELD_PER_POINT)
            cue(2.0)
            return
        }
        damage(points, 1.0)
    }
    fn blast(point) {
        let ring = blast_at(point)
        if !ring || !alive() { return }
        let shield = worn_shield()
        if shield && ring.shield > 0.0 {
            shield.take_damage(ring.shield)
            if ring.health <= 0.0 { cue(2.0) }
        }
        if ring.health > 0.0 { damage(ring.health, 1.0) }
    }
    on(events, "DataEvent", fn(event) {
        if event != "Revive" { return }
        state.points = FULL
        bar.set_health(1.0)
        let on_respawn = callback("on_respawn")
        if on_respawn { on_respawn() }
    })
    return { root = timers alive = alive damage = damage blast_at = blast_at hit = hit blast = blast
        fraction = fn() { return state.points / FULL }
        wear_shields = fn(source) { state.shields = source } }
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
//   combat      optional controller factory such as mitten_antagonist
//   player_alive() optional gate for the antagonist
//   armory      an optional mitten_armory. Players can take the NPC's weapon,
//               and the NPC follows the shortest available route to a loaded gun.
//               With none to collect it rides its vehicle until one turns up, or
//               without a vehicle walks up to the player and takes theirs.
//   retreat(here, player) optional route to cover after a shield or body hit.
//   on_player_hit(points)  called for each shot that lands on the player
//   on_down()    called each time the NPC goes down
//   on_lie_down(kind) called once the downed body lies "back" or "side"
//   on_respawn() called each time the NPC gets back up
//   on_shot(pose, mode) called for each shot the NPC fires (muzzle pose,
//                "semi"/"burst"/"auto"), e.g. to replay it on a peer
//   health_view (the bar faces it), on_damage(cue)
//   seated       sit (Mitten's floor-seated pose, no leg IK) instead of
//                standing, e.g. in a vehicle seat; position is then the root
//                in the seat's parent space
//   vehicle      { chassis, seat, door, ground_y } with seated: the caller attaches
//                root to chassis at seat (chassis space). Going down ejects
//                the NPC beside the door (chassis space; on the ground);
//                after respawning it walks back to the door, which may be
//                moving, and gets back in, aiming and firing throughout.
//                ground_y (default 0) is her root's height off the truck.
//                truck is the optional drive input target. turn_bounds marks
//                the limits of the driving area for repositioning.
//                set_vehicle_chassis(chassis) moves her to another vehicle
// The NPC's root may ride a moving, turned parent (a truck's chassis): it
// aims in world space and turns relative to that parent.
export fn practice_target(options) {
    let physics = options["physics"] == true
    let armory = options["armory"]
    let gait = null
    if armory { gait = { stride = 0.37 step_scale = 1.60 step_height = 0.075 } }
    let npc = make_rooftop_player({
        prefix = options.prefix local_player = false avatar = options.avatar
        use_xr = options.use_xr use_desktop = options.use_desktop opaque_bucket = "near"
        show_health = true health_view = options["health_view"] shield_mount = true
        // The untracked left arm rests in the relaxed pose.
        physics = physics relaxed_pose = true seated_pose = options["seated"] == true walking_ik = gait
    })
    let strafe = 2.5
    if options["strafe"] != null { strafe = options.strafe }
    let speed = 1.2
    if options["speed"] != null { speed = options.speed }
    let on_damage = options["on_damage"]
    let player = options["player"]
    let vehicle = options["vehicle"]
    let driver = null
    if vehicle && vehicle["truck"] && player { driver = mitten_driver(vehicle, player) }
    let activity = { tick = null moving_to_destination = false attacking_target = false }
    fn set_activity(name, active) {
        if activity[name] == active { return }
        let was_active = activity.moving_to_destination || activity.attacking_target
        activity[name] = active
        let is_active = activity.moving_to_destination || activity.attacking_target
        if was_active != is_active { activity.tick.bypass(!is_active) }
    }
    // chase: unarmed with nothing to collect and no vehicle, so going for the
    // player's weapon (route then leads to the player).
    let rearm = { phase = null waypoint = 0 route = null pickup = null chase = false }
    let retreat = options["retreat"]
    let patrol_options = options["patrol"]
    let patrol = { route = null waypoint = 0 corner = 0 engaged = false retry = 0.0 }
    let cover = { route = null waypoint = 0 hold = false cooldown = false }
    let events = T {}
    let retry = null
    if armory { retry = after_seconds(events, 0.5, "FindWeapon") }
    let cover_hold = null
    let cover_cooldown = null
    if retreat {
        cover_hold = after_seconds(events, 2.5, "CoverHoldDone")
        cover_cooldown = after_seconds(events, 8.0, "CoverReady")
    }
    let bearing = null
    if driver { bearing = after_seconds(events, 0.2, "CheckBearing") }
    // yaw is the head's heading relative to the NPC's parent; aim_yaw is the
    // world heading of the last aim, for the shot check; frame is the NPC's
    // root frame once built, whose parent may turn.
    let state = { shield = null time = 0.0 yaw = options.yaw aim_yaw = options.yaw pitch = 0.0 steps = 0
        walk_x = 0.0 walk_z = 0.0 frame = null seated = false waiting_for_seat = false }
    fn drop_shield() {
        if state.shield { state.shield.remove_subtree() }
        state.shield = null
    }
    fn wear_shield() {
        drop_shield()
        let shield = energy_shield(options.prefix + "_shield", 2.0, { worn = true })
        npc.shield_mount.attach(shield)
        shield.set_worn(true, true)
        state.shield = shield
    }
    let hooks = { death = null respawn = null set_chassis = null remove_ground = null eject = null approach = null
        refresh_motion = null resume_motion = null attacked = null }
    // Each fall alternates lying on the back and curling on the side. The
    // body lies down a moment after going down, once a vehicle has let go.
    let fall = { count = 0 kind = null }
    let lie_timer = after_seconds(events, 0.1, "LieDown")
    let health = practice_health(npc.health, npc.shield_mount, {
        on_damage = on_damage respawn_seconds = NPC_RESPAWN_SECONDS
        on_death = fn() {
            drop_shield()
            if hooks.death { hooks.death() }
            if physics {
                let linear = npc.velocity.linear()
                npc.velocity.translate([-linear[0], 0.0, -linear[2]])
            }
            hooks.refresh_motion()
            lie_timer.play()
            if options["on_down"] { options.on_down() }
        }
        on_respawn = fn() {
            lie_timer.pause()
            get_up(npc)
            fall.kind = null
            patrol.route = null
            patrol.engaged = false
            patrol.retry = 0.0
            wear_shield()
            if hooks.respawn { hooks.respawn() }
            hooks.resume_motion()
            if options["on_respawn"] { options.on_respawn() }
        }
    })
    wear_shield()

    fn seek_cover() {
        if !retreat || !player || !health.alive() || rearm.phase || cover.route || cover.cooldown { return }
        let shield = 0.0
        if state.shield { shield = state.shield.integrity() }
        if health.fraction() >= 0.5 && shield >= 50.0 { return }
        let route = retreat(npc.root.world_trs().translation, player.world_trs().translation)
        if !route || route.length < 0.3 { return }
        cover.route = route
        cover.waypoint = 0
        cover.cooldown = true
        cover_cooldown.play()
        hooks.refresh_motion()
    }

    fn pistol_hit(hit) {
        if !health.alive() { return }
        let shield = hit.renderable.energy_shield
        if shield && shield == state.shield {
            shield.take_damage(10.0)
            if on_damage { on_damage(2.0) }
            seek_cover()
            if hooks.attacked { hooks.attacked() }
            return
        }
        if hit.renderable.player_hit_zone == npc.hit_zone {
            health.damage(1.0, 1.0)
            seek_cover()
            if hooks.attacked { hooks.attacked() }
        }
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
        if blast.health > 0.0 || shield_hit {
            seek_cover()
            if hooks.attacked { hooks.attacked() }
        }
    }
    // Damage decided on another machine, such as a peer's shots at a replica
    // of this NPC: shield takes shield_amount while charged (once it has
    // broken here, a point per SHIELD_PER_POINT reaches health instead), and
    // health takes points.
    fn take_damage(points, shield_amount) {
        if !health.alive() { return }
        let shield = state.shield
        if shield_amount > 0.0 {
            if shield && shield.integrity() > 0.0 {
                shield.take_damage(shield_amount)
                if on_damage { on_damage(2.0) }
            } else {
                points = points + shield_amount / SHIELD_PER_POINT
            }
        }
        if points > 0.0 { health.damage(points, 1.0) }
        seek_cover()
        if hooks.attacked { hooks.attacked() }
    }

    // Turning: the head target carries the whole rig (body, both arms and
    // the pistol). One update_transform per aim; the Transition tweens it.
    let turn = Transition { duration_beats(AIM_TURN_SECONDS * 2.0) ease_in_out_sine() replace_same_target() }
    if !options["combat"] { npc.head.attach(turn) }
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
    if !options["combat"] { npc.right.attach(raise) }
    let wrist_basis = null
    if options.avatar == "mitten" { wrist_basis = mitten_right_wrist_basis() }
    fn aim_hand(pitch) {
        state.pitch = pitch
        let translation = [
            SHOULDER[0],
            SHOULDER[1] + Math.sin(pitch) * HAND_REACH - HAND_DROP,
            SHOULDER[2] - Math.cos(pitch) * HAND_REACH,
        ]
        let sa = Math.sin(pitch / 2.0)
        let ca = Math.cos(pitch / 2.0)
        let sb = Math.sin(HAND_YAW / 2.0)
        let cb = Math.cos(HAND_YAW / 2.0)
        let rotation = [sa * cb, ca * sb, sa * sb, ca * cb]
        if wrist_basis {
            let q = wrist_basis
            rotation = [ca * q[0] + sa * q[3], ca * q[1] - sa * q[2], ca * q[2] + sa * q[1], ca * q[3] - sa * q[0]]
        }
        npc.right.trs({
            translation = translation
            rotation_quat_xyzw = rotation
            scale = [1.0, 1.0, 1.0]
        })
    }

    // The right arm is held out with the pistol; the left arm relaxes.
    let combat = { controller = null gun = null }
    fn shot(pose, mode) {
        if options["on_shot"] { options.on_shot(pose, mode) }
        if combat.controller && combat.controller["shot"] { combat.controller.shot(pose, mode) }
        if armory && !combat.gun.has_ammo() { emit_data(events, "GunEmpty") }
    }
    fn disarmed() {
        if driver { driver.stop() }
        if hooks.eject { hooks.eject() }
        if combat.controller { combat.controller.stop() }
        npc.right_driver.set_pose_valid(false)
        rearm.phase = "seek"
        rearm.pickup = null
        rearm.route = null
        rearm.waypoint = 0
        rearm.chase = false
        cover.route = null
        cover.hold = false
        if cover_hold { cover_hold.pause() }
        hooks.refresh_motion()
        emit_data(events, "FindWeapon")
    }
    let sight = { elapsed = 2.0 visible = false }
    let motion_visibility = options["secondary_motion_observer"]
    let visibility = null
    if motion_visibility { visibility = secondary_motion_visibility(npc.model, npc.shield_mount, motion_visibility) }
    let sight_tick = on_global("FrameTick", fn(event) { sight.elapsed = sight.elapsed + event.dt_sec })
    fn can_attack() {
        if !health.alive() || !combat.gun.has_ammo() || rearm.phase { return false }
        if (cover.route || cover.hold) && health.fraction() <= 0.5 { return false }
        if player && sight.elapsed >= 2.0 {
            sight.elapsed = 0.0
            let h = npc.head.world_trs().translation
            let origin = [h[0], h[1] + SHOULDER[1], h[2]]
            let p = player.world_trs().translation
            let d = [p[0] - origin[0], p[1] - origin[1], p[2] - origin[2]]
            let distance = Math.sqrt(d[0] * d[0] + d[1] * d[1] + d[2] * d[2])
            let hit = Physics.raycast(origin, d, distance)
            sight.visible = hit == null || hit.distance > distance - COVER_MARGIN
        }
        if player && !sight.visible { return false }
        return !state.seated || !driver || driver.can_fire()
    }
    if player {
        npc.left_driver.set_pose_valid(false)
        npc.right_driver.set_pose_valid(true)
        aim_hand(0.0)
        let weapon_name = options["weapon_name"]
        if !weapon_name { weapon_name = options.prefix + "_pistol" }
        let weapon = ship_machine_pistol(weapon_name, {
            opaque_bucket = "near" ammunition = 128
            on_hit = fn(hit) { if hit.source_user != null { pistol_hit(hit) } }
            world_damage = options["world_damage"]
            shared_impacts = options["shared_impacts"]
            left_pointer = "#local_left_pointer" right_pointer = "#local_right_pointer" user = "#local_avatar"
        })
        if !armory {
            for grab in weapon.grip.query_all("grabbable") { grab.remove_subtree() }
        }
        let grip_offset = [0.0, 0.0, -0.105]
        if wrist_basis { grip_offset = [-0.018, 0.0, -0.105] }
        let gun_mount = T.position(grip_offset[0], grip_offset[1], grip_offset[2]) {
            name = options.prefix + "_gun_mount"
        }
        let mount = gun_mount
        if wrist_basis {
            // Cancel the bone basis for the prop, and follow the solved wrist.
            let q = wrist_basis
            mount = T.rotation_quat(-q[0], -q[1], -q[2], q[3]) { gun_mount }
        }
        npc.right.attach(mount)
        on(npc.model, "GLTFInitialized", fn(event) {
            pistol_grip(event.gltf, "Right")
            if wrist_basis {
                let wrist = event.gltf.query("#J_Bip_R_Hand")
                if wrist { wrist.attach(mount) }
            }
        })
        combat.gun = mitten_weapon_slot(gun_mount, armory, can_attack, shot, disarmed, wrist_basis != null)
        combat.gun.equip(weapon, false)
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
    // The antagonist moves the real muzzle toward the target. The legacy
    // practice profile takes a single aim sample and lets its rig tween to it.
    fn aim_at_player(max_step) {
        let origin = shoulder_world()
        if max_step != null { origin = combat.gun.muzzle_pose().translation }
        let p = player.world_trs().translation
        let dx = p[0] - origin[0]
        let dz = p[2] - origin[2]
        state.aim_yaw = Math.atan2(-dx, -dz)
        let yaw = state.aim_yaw - frame_yaw()
        let pitch = Math.atan2(p[1] - origin[1], Math.sqrt(dx * dx + dz * dz))
        if max_step != null {
            let delta = yaw - state.yaw
            yaw = state.yaw + Math.clamp(Math.atan2(Math.sin(delta), Math.cos(delta)), -max_step, max_step)
            pitch = state.pitch + Math.clamp(pitch - state.pitch, -max_step, max_step)
        }
        face(yaw)
        aim_hand(pitch)
    }
    fn shot_lands(pose) {
        let s = shoulder_world()
        let c = Math.cos(state.pitch)
        let f = [-Math.sin(state.aim_yaw) * c, Math.sin(state.pitch), -Math.cos(state.aim_yaw) * c]
        if pose {
            s = pose.translation
            f = rotate(pose.rotation_quat_xyzw, [0.0, 0.0, -1.0])
        }
        let p = player.world_trs().translation
        let d = [p[0] - s[0], p[1] - s[1], p[2] - s[2]]
        let along = d[0] * f[0] + d[1] * f[1] + d[2] * f[2]
        let off = [d[0] - f[0] * along, d[1] - f[1] * along, d[2] - f[2] * along]
        let miss = Math.sqrt(off[0] * off[0] + off[1] * off[1] + off[2] * off[2])
        if along <= 0.0 || miss >= SHOT_HALF_WIDTH || (pose && along > 60.0) { return false }
        // Only check solid cover on a shot that can actually reach the player.
        let distance = Math.sqrt(d[0] * d[0] + d[1] * d[1] + d[2] * d[2])
        if pose { d = f distance = along }
        let hit = Physics.raycast(s, d, distance)
        return hit == null || hit.distance > distance - COVER_MARGIN
    }

    if combat.gun {
        let controller = options["combat"]
        if !controller { controller = practice_combat }
        combat.controller = controller({
            name = options.prefix gun = combat.gun aim = aim_at_player shot_lands = shot_lands
            on_player_hit = options["on_player_hit"] player_alive = options["player_alive"]
            can_attack = can_attack
            aiming = fn(active) { set_activity("attacking_target", active) }
        })
        if combat.controller["attacked"] {
            hooks.attacked = fn() {
                sight.elapsed = 2.0
                combat.controller.attacked()
            }
        }
        hooks.death = fn() {
            combat.controller.stop()
            cover.route = null
            cover.hold = false
            if cover_hold { cover_hold.pause() }
        }
        hooks.respawn = fn() { if !rearm.phase { combat.controller.start() } }
    }

    fn walk_to(destination, dt) {
        let here = npc.root.world_trs().translation
        let dx = destination[0] - here[0]
        let dz = destination[2] - here[2]
        let distance = Math.sqrt(dx * dx + dz * dz)
        if distance < 0.08 { return true }
        let step = Math.clamp(RUN_SPEED * dt / distance, 0.0, 1.0)
        face(Math.atan2(-dx, -dz) - frame_yaw())
        npc.root.set_translation([here[0] + dx * step, here[1], here[2] + dz * step], "world")
        return false
    }
    // Death and disarming both take Mitten out through the driver's door.
    fn ride_vehicle(ride) {
        state.seated = true
        if driver { ride.truck.set_remote_occupied(true) }
        let ground = T { name = options.prefix + "_ground" }
        ground
        let seat_pose = null
        if options.avatar == "mitten" && options["seated"] == true {
            seat_pose = seated_lower_body_pose()
            ground.attach(seat_pose)
            on(events, "DataEvent", fn(event) {
                if event == "ApplySeatPose" && state.seated && npc.model.query("#J_Bip_C_Hips") {
                    seat_pose.overlay(npc.model)
                }
            })
        }
        fn chassis_point(local) {
            let pose = ride.chassis.world_trs()
            let r = rotate(pose.rotation_quat_xyzw, local)
            return [pose.translation[0] + r[0], pose.translation[1] + r[1], pose.translation[2] + r[2]]
        }
        fn eject() {
            if !state.seated { return }
            if driver { driver.stop() }
            state.seated = false
            if driver {
                bearing.pause()
                ride.truck.set_remote_occupied(false)
            }
            let door = chassis_point(ride.door)
            let ground_y = 0.0
            if ride["ground_y"] != null { ground_y = ride.ground_y }
            ground.attach(state.frame)
            state.frame.update_transform([door[0], ground_y, door[2]], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
            npc.root.update_transform([0.0, 0.0, 0.0])
            npc.set_legs_enabled(true)
        }
        fn board() {
            state.seated = true
            state.waiting_for_seat = false
            ride.chassis.attach(state.frame)
            state.frame.update_transform(ride.seat, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
            npc.root.update_transform([0.0, 0.0, 0.0])
            npc.set_legs_enabled(false)
            // Apply after the leg solver has released its walking pose.
            if seat_pose { emit_data(events, "ApplySeatPose") }
            if driver {
                driver.stop()
                ride.truck.set_remote_occupied(true)
                bearing.play()
            }
            hooks.refresh_motion()
        }
        hooks.approach = fn(dt) {
            let door = chassis_point(ride.door)
            let here = npc.root.world_trs().translation
            let dx = door[0] - here[0]
            let dz = door[2] - here[2]
            if dx * dx + dz * dz > BOARD_DISTANCE * BOARD_DISTANCE {
                walk_to(door, dt)
                return false
            }
            if driver && ride.truck.drive_status().mounted {
                state.waiting_for_seat = true
                bearing.play()
                hooks.refresh_motion()
                return false
            }
            board()
            return true
        }
        hooks.remove_ground = fn() { ground.remove_subtree() }
        hooks.eject = eject
        hooks.set_chassis = fn(chassis) {
            ride.chassis = chassis
            if state.seated { board() }
        }
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
        health.root
        events
        lie_timer
        if retry { retry }
        if cover_hold { cover_hold cover_cooldown }
        if bearing { bearing }
        if combat.controller && combat.controller.root { combat.controller.root }
    }
    state.frame = frame
    if vehicle { ride_vehicle(vehicle) }
    fn follow(path, route, dt) {
        if walk_to(route.point(path.waypoint), dt) { path.waypoint = path.waypoint + 1 }
        return path.waypoint == route.count
    }

    fn refresh_motion() {
        let traveling = cover.route || rearm.pickup || rearm.phase == "return" || rearm.chase
        traveling = traveling || patrol_options
        traveling = traveling || (vehicle && !state.seated && !state.waiting_for_seat)
        let turning = driver && state.seated && driver.turning()
        let wandering = physics || (!vehicle && strafe != 0.0)
        set_activity("moving_to_destination", health.alive() && (traveling || turning || wandering))
    }
    hooks.refresh_motion = refresh_motion

    fn player_alive() { return !options["player_alive"] || options.player_alive() }

    // In order: a loaded weapon to collect; else ride the vehicle; else take
    // the player's weapon. Rechecked every retry, so a weapon that turns up
    // later still comes first.
    fn find_weapon() {
        if rearm.phase != "seek" || rearm.pickup || !health.alive() { return }
        if state.seated {
            // Weapons are reached on foot, so measure height from the ground.
            let seat = npc.root.world_trs().translation
            let ground_y = 0.0
            if vehicle["ground_y"] != null { ground_y = vehicle.ground_y }
            if armory.nearest([seat[0], ground_y, seat[2]]) { hooks.eject() }
        }
        rearm.pickup = armory.nearest(npc.root.world_trs().translation)
        rearm.waypoint = 0
        rearm.chase = false
        if !rearm.pickup {
            rearm.chase = !vehicle && player && player_alive() && armory.held_by_player() != null
            rearm.route = null
            retry.play()
        }
        refresh_motion()
    }

    fn finish_rearm() {
        rearm.phase = null
        combat.controller.start()
        refresh_motion()
    }

    // Holding a weapon again: back to the vehicle, or to the NPC's post.
    fn armed(weapon, rack_gun) {
        combat.gun.equip(weapon, rack_gun)
        npc.right_driver.set_pose_valid(true)
        rearm.pickup = null
        rearm.chase = false
        if vehicle {
            rearm.phase = "vehicle"
        } else {
            rearm.phase = "return"
            rearm.waypoint = 0
            rearm.route = armory.path(npc.root.world_trs().translation, options.position)
            if !rearm.route { finish_rearm() }
        }
        refresh_motion()
    }

    fn take_player_weapon() {
        let weapon = armory.held_by_player()
        rearm.route = null
        if weapon && weapon.take_from_player() { armed(weapon, false) }
    }

    // Walk up to the player, replanning when the route runs out or a retry
    // clears it (every half second), and take their weapon within reach.
    fn chase_player(dt) {
        let here = npc.root.world_trs().translation
        let target = player.world_trs().translation
        let dx = target[0] - here[0]
        let dz = target[2] - here[2]
        if dx * dx + dz * dz < TAKE_REACH * TAKE_REACH {
            take_player_weapon()
            return
        }
        if !rearm.route {
            rearm.route = armory.path(here, [target[0], here[1], target[2]])
            rearm.waypoint = 0
        }
        if rearm.route && follow(rearm, rearm.route, dt) { rearm.route = null }
    }

    fn advance_rearm(dt) {
        if rearm.phase == "vehicle" {
            if hooks.approach(dt) { finish_rearm() }
            return
        }
        if rearm.phase == "return" {
            if follow(rearm, rearm.route, dt) { finish_rearm() }
            return
        }
        if !rearm.pickup {
            // Nothing to collect: ride the vehicle, or go for the player's weapon.
            if vehicle && !state.seated { hooks.approach(dt) }
            if rearm.chase { chase_player(dt) }
            return
        }
        if !armory.available(rearm.pickup) {
            rearm.pickup = null
            find_weapon()
            return
        }
        if !follow(rearm, rearm.pickup.route, dt) { return }
        armed(rearm.pickup.weapon, true)
    }

    fn check_bearing() {
        if !health.alive() { return }
        if state.waiting_for_seat {
            state.waiting_for_seat = false
            refresh_motion()
            return
        }
        if !state.seated { return }
        if !options["player_alive"] || options.player_alive() { driver.update() }
        else { driver.stop() }
        if !driver.turning() { bearing.play() }
        refresh_motion()
    }

    fn discard_empty_gun() {
        if health.alive() && combat.gun.ready() && !combat.gun.has_ammo() {
            combat.gun.discard(npc.root.world_trs().translation[1])
        }
    }

    on(events, "DataEvent", fn(event) {
        if event == "FindWeapon" { find_weapon() }
        if event == "GunEmpty" { discard_empty_gun() }
        if event == "CoverReady" { cover.cooldown = false }
        if event == "CoverHoldDone" { cover.hold = false }
        if event == "CheckBearing" { check_bearing() }
        if event == "LieDown" && !health.alive() {
            let kind = "back"
            if fall.count % 2 == 1 { kind = "side" }
            fall.count = fall.count + 1
            if lie_down(npc, kind, null) {
                fall.kind = kind
                if options["on_lie_down"] { options.on_lie_down(kind) }
            }
        }
    })

    fn resume_motion() {
        if rearm.phase == "seek" && !rearm.pickup { emit_data(events, "FindWeapon") }
        if armory && combat.gun && combat.gun.ready() && !combat.gun.has_ammo() { emit_data(events, "GunEmpty") }
        if bearing && (state.seated || state.waiting_for_seat) { bearing.play() }
        refresh_motion()
    }
    hooks.resume_motion = resume_motion

    fn advance_movement(dt) {
        let alive = health.alive()
        if rearm.phase {
            if alive { advance_rearm(dt) }
            return
        }
        if alive && cover.route {
            if follow(cover, cover.route, dt) {
                cover.route = null
                cover.hold = true
                cover_hold.play()
                refresh_motion()
            }
            return
        }
        if alive && patrol_options && !cover.hold {
            let visible = player_alive() && can_attack()
            if visible {
                patrol.route = null
                if !patrol.engaged && hooks.attacked { hooks.attacked() }
                patrol.engaged = true
                return
            }
            patrol.engaged = false
            if !patrol.route {
                if patrol.retry > 0.0 {
                    patrol.retry = patrol.retry - dt
                    return
                }
                patrol.route = patrol_options.plan(npc.root.world_trs().translation, patrol_options.points[patrol.corner])
                patrol.waypoint = 0
                if !patrol.route {
                    patrol.corner = (patrol.corner + 1) % len(patrol_options.points)
                    patrol.retry = 1.0
                    return
                }
            }
            if follow(patrol, patrol.route, dt) {
                patrol.route = null
                patrol.corner = (patrol.corner + 1) % len(patrol_options.points)
            }
            return
        }
        if vehicle && !state.seated && alive { hooks.approach(dt) }
        if driver && state.seated && driver.turning() { check_bearing() }
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
        } else if alive && !vehicle && strafe != 0.0 {
            // Strafe back and forth along X.
            state.time = state.time + dt
            npc.root.update_transform([strafe * Math.sin(state.time * speed), 0.0, 0.0])
        }
    }
    activity.tick = on(world, "FrameTick", fn(event) {
        if activity.moving_to_destination { advance_movement(event.dt_sec) }
        if activity.attacking_target { combat.controller.advance(event.dt_sec) }
    })
    activity.tick.bypass(true)
    if combat.controller { combat.controller.start() }
    resume_motion()
    return {
        remove = fn() {
            if visibility { visibility.remove() }
            sight_tick.remove()
            if combat.controller { combat.controller.stop() }
            activity.tick.remove()
            if driver { driver.stop() }
            if combat.gun { combat.gun.dispose() }
            npc.dispose()
            frame.remove_subtree()
            if hooks.remove_ground { hooks.remove_ground() }
        }
        root = frame npc = npc pistol_hit = pistol_hit rocket_splash = rocket_splash alive = health.alive
        take_damage = take_damage health_fraction = health.fraction
        // "back" or "side" while lying down, else null.
        lying = fn() { return fall.kind }
        armed = fn() { return combat.gun != null && combat.gun.ready() }
        set_vehicle_chassis = fn(chassis) { if hooks.set_chassis { hooks.set_chassis(chassis) } }
        center = fn() { return npc.shield_mount.world_trs().translation }
        shield_integrity = fn() {
            if state.shield { return state.shield.integrity() }
            return 0.0
        }
    }
}
