import { ship_environment_material } from "../materials/ship_anime_materials.mms"
import { laser_machine_pistol } from "../laser-machine-pistol.mms"
import { mini_explosion } from "../mini-explosion.mms"
import { emissive_surface } from "../materials/emissive_surface.mms"

// Reuses the existing weapon's muzzle, selector, sounds and firing cadence.
// Unique labels keep the three rack instances' controls independent.
export fn ship_machine_pistol(label) {
    let scale = 0.085
    let hand_offset = 0.05
    let material = ship_environment_material()
    fn impact(hit) {
        Damage.at(hit.hit_point[0], hit.hit_point[1], hit.hit_point[2], 0.28, 1.0)
        let effect = mini_explosion(hit.hit_point, 2.0)
        effect.root
        effect.animation.play()
    }
    let tool = laser_machine_pistol({
        name = label + "_laser" length = 60.0 flash_size = 0.22 beam_radius = 0.012
        ray_origin_offset = 0.18 on_hit = impact ammunition = 75
    })
    let ammo_count = Text {
        name = label + "_ammo_count"
        "75"
        C.rgba(1.0,0.35,0.65,1.0)
        emissive_surface()
        EM.on().intensity(3.0)
    }
    let ammo_display = { pending = false shown = 75 }
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
            let remaining = tool.device_state.ammunition
            if remaining != ammo_display.shown {
                ammo_count.set_text("" + remaining)
                ammo_display.shown = remaining
            }
            ammo_display.pending = false
        }
        if event == "ShotFired" && !ammo_display.pending {
            ammo_display.pending = true
            ammo_refresh.play()
        }
    })
    let held = { active = false hand = "" pointer = null }
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
            if event.pointer == query("[name='ship_left_hand_pointer']") { held.hand = "Left" }
            if event.pointer == query("[name='ship_right_hand_pointer']") { held.hand = "Right" }
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
        if !held.active {return}
        if kind=="XrButtonDown" || kind=="XrButtonUp" {
            if event.hand==held.hand && (event.control=="LeftTrigger" || event.control=="RightTrigger") {
                if kind=="XrButtonDown" {tool.use_start({user=query("#ship_avatar_control")})}
                else {tool.use_end()}
            }
        }
        if held.hand=="Desktop" && (kind=="KeyDown" || kind=="KeyUp") && event.code=="Space" {
            if kind=="KeyDown" {tool.use_start({user=held.pointer})}
            else {tool.use_end()}
        }
    }
    return T {
        name = label + "_assembly"
        grip
        for projectile in tool.projectiles { projectile }
    }
}
