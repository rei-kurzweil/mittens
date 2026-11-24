import { ship_environment_material } from "../materials/ship_anime_materials.mms"
import { laser_machine_pistol } from "../laser-machine-pistol.mms"
import { mini_explosion } from "../mini-explosion.mms"
import { emissive_surface } from "../materials/emissive_surface.mms"
import { shared_grabbable } from "../network/shared_grabbable.mms"
import { shared_impact } from "../network/shared_impact.mms"
import { weapon_restock } from "./weapon_restock.mms"

export fn apply_machine_pistol_impact(point, amount) {
    Damage.at(point[0], point[1], point[2], 0.28, amount)
    let effect = mini_explosion(point, 2.0)
    effect.root
    effect.animation.play()
}

// Reuses the existing weapon's muzzle, selector, sounds and firing cadence.
// Unique labels keep rack instances' controls independent.
// For a shared pistol pass { socket, weapon_id, rank, mounts? }, bind the local pointers
// and connect/disconnect the assembly with the scene's peer lifecycle.
// Optional can_fire(user) and on_hit(hit) supply the scene's gameplay rules.
export fn ship_machine_pistol(label, options) {
    let scale = 0.085
    let hand_offset = 0.05
    // options.opaque_bucket draws a hand-held pistol before the scenery.
    let bucket = null
    if options { bucket = options["opaque_bucket"] }
    let material = ship_environment_material(bucket)
    let sharing = { grab = null socket = null id = label }
    let impacts = null
    let fire_gate = null
    let on_hit = null
    let ammunition = 75
    let left_pointer = "#ship_left_hand_pointer"
    let right_pointer = "#ship_right_hand_pointer"
    let user = "#ship_avatar_control"
    if options {
        sharing.socket = options["socket"]
        impacts = options["shared_impacts"]
        fire_gate = options["can_fire"]
        on_hit = options["on_hit"]
        if options["ammunition"] != null {
            ammunition = Math.floor(options.ammunition)
            if ammunition < 0 { ammunition = 0 }
        }
        if options["weapon_id"] { sharing.id = options.weapon_id }
        if options["left_pointer"] { left_pointer = options.left_pointer }
        if options["right_pointer"] { right_pointer = options.right_pointer }
        if options["user"] { user = options.user }
    }
    fn locally_owned() { return !sharing.grab || sharing.grab.state.owner == options.rank }
    fn can_fire(user) { return locally_owned() && (!fire_gate || fire_gate(user)) }
    let own_impacts = sharing.socket && !impacts
    if own_impacts {
        impacts = shared_impact({ socket = sharing.socket name = sharing.id apply = apply_machine_pistol_impact })
    }
    fn impact(hit) {
        if impacts { impacts.hit(hit.hit_point, hit.damage) }
        else { apply_machine_pistol_impact(hit.hit_point, hit.damage) }
        if on_hit { on_hit(hit) }
    }
    let tool = laser_machine_pistol({
        name = label + "_laser" length = 60.0 flash_size = 0.22 beam_radius = 0.012
        ray_origin_offset = 0.18 on_hit = impact ammunition = ammunition
        socket = sharing.socket weapon_id = sharing.id can_fire = can_fire
    })
    let ammo_count = Text {
        name = label + "_ammo_count"
        "" + ammunition
        C.rgba(1.0,0.35,0.65,1.0)
        emissive_surface()
        EM.on().intensity(3.0)
    }
    let ammo_display = { pending = false shown = ammunition }
    let restock = { timer = null }
    fn show_ammo(amount) {
        if amount == ammo_display.shown { return }
        ammo_count.set_text("" + amount)
        ammo_display.shown = amount
    }
    // Start once per dirty interval, without restarting on subsequent shots.
    // This trailing throttle updates during continuous fire and after release.
    let ammo_refresh = Animation.paused().length(2.0) {
        name = label + "_ammo_refresh"
        // Animation time is relative to its earliest keyframe.
        Keyframe.at(0.0) {}
        Keyframe.at(2.0) { emit_data(tool.completion,"AmmoRefreshDue") }
    }
    on(tool.completion,"DataEvent",fn(event) {
        if event == "AmmoRefreshDue" {
            show_ammo(tool.device_state.ammunition)
            ammo_display.pending = false
        }
        if event == "ShotFired" && restock.timer {
            let amount = tool.device_state.ammunition
            sharing.grab.set_ammo(amount)
            if amount == 0 { restock.timer.start() }
        }
        if event == "ShotFired" && !ammo_display.pending {
            ammo_display.pending = true
            ammo_refresh.play()
        }
    })
    let held = { active = false hand = "" pointer = null }
    fn apply_ammo(amount, returned) {
        if returned {
            held.active = false
            held.pointer = null
            tool.use_end()
            ammo_refresh.pause()
            ammo_display.pending = false
            restock.timer.cancel()
        }
        tool.device_state.ammunition = amount
        show_ammo(amount)
    }
    let grip = T.rotation_quat(0.5, -0.5, -0.5, 0.5) {
        name = label + "_grip"
        Selectable.off()
        Grabbable.to_hand(true).hand_offset([0.0, 0.0, -0.105]) {}
        // Above the scope, facing back toward the person holding the weapon.
        T.position(-0.018,0.185,0.060).scale(0.018,0.018,0.018) {
            Raycastable.disabled()
            ammo_count
        }
        ammo_refresh
        T.position(0.028, 0.085, 0.0402).rotation(0.0, Math.pi / 2.0, 0.0) { tool.mode_selector }
        T.position(0.0, 1.048425 * scale, hand_offset - 3.342659 * scale - 0.015) { tool.root }
        T.position(0.0, 0.0, hand_offset).rotation(0.0, Math.pi, 0.0).scale(scale, scale, scale) {
            GLTF.new("assets/models/sakura-machine-pistol.glb") { name = label material }
        }
    }
    on(grip, "GrabStart", fn(event) {
        if event.target == grip {
            held.active = true
            held.pointer = event.pointer
            held.hand = "Desktop"
            if event.pointer == query(left_pointer) { held.hand = "Left" }
            if event.pointer == query(right_pointer) { held.hand = "Right" }
        }
    })
    on(grip, "GrabEnd", fn(event) {
        if event.target == grip && event.pointer == held.pointer {
            held.active = false
            held.pointer = null
            tool.use_end()
        }
    })
    // Input is forwarded by the ship controller, never globally subscribed here.
    grip.ship_input=fn(kind,event) {
        if !held.active || !locally_owned() {return}
        if kind=="XrButtonDown" || kind=="XrButtonUp" {
            if event.hand==held.hand && (event.control=="LeftTrigger" || event.control=="RightTrigger") {
                if kind=="XrButtonDown" {tool.use_start({user=query(user)})}
                else {tool.use_end()}
            }
        }
        if held.hand=="Desktop" && (kind=="KeyDown" || kind=="KeyUp") && event.code=="Space" {
            if kind=="KeyDown" {tool.use_start({user=held.pointer})}
            else {tool.use_end()}
        }
    }
    let grip_mount = grip
    if sharing.socket {
        let stock = null
        let restock_rate = options["restock_beats_per_second"]
        if restock_rate != null { stock = { capacity = ammunition apply = apply_ammo } }
        let grab = shared_grabbable({ socket = sharing.socket name = sharing.id + ".grip" target = grip rank = options.rank mounts = options["mounts"] stock = stock })
        sharing.grab = grab
        grip_mount = grab.root
        if stock { restock.timer = weapon_restock(label, restock_rate, grab.restock) }
    }
    let assembly = T {
        name = label + "_assembly"
        grip_mount
        if restock.timer { restock.timer.root }
        for projectile in tool.projectiles { projectile }
    }
    assembly.grip = grip
    if sharing.socket {
        assembly.bind_pointer = sharing.grab.bind_pointer
        assembly.connect = fn(group, address) {
            sharing.grab.connect(group, address)
            tool.network.connect(group, address)
            if own_impacts { impacts.connect(group, address) }
        }
        assembly.disconnect = fn() {
            tool.use_end()
            sharing.grab.disconnect()
            tool.network.disconnect()
            if own_impacts { impacts.disconnect() }
            if restock.timer && tool.device_state.ammunition == 0 { restock.timer.start() }
        }
    }
    return assembly
}
