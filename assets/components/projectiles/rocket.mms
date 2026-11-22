import { rocket_explosion } from "./rocket_explosion.mms"
import { sta_rocket_body } from "./sta_rocket_ammo.mms"
import { emissive_surface } from "../materials/emissive_surface.mms"
import { toon_rocket_exhaust } from "../materials/toon_rocket_exhaust.mms"

let trail_length = 3.0
let trail_width = 0.9
// The recentered GLB's rear face ends at z=0.17875 with a diameter of 0.0385.
let nozzle_z = 0.18
// Exhaust cone length behind the rear face.
let flame_length = 0.16

// World direction of local -Z, the rocket's flight axis.
fn forward_of(q) {
    let x = q[0]
    let y = q[1]
    let z = q[2]
    let w = q[3]
    return [-2.0 * (x * z + w * y), -2.0 * (y * z - w * x), -(1.0 - 2.0 * (x * x + y * y))]
}

// Two crossed planes so the ribbon stays visible edge-on. Each runs from the
// nozzle (UV y=0) back along local +Z; the shader clips it at the launch point.
fn exhaust_trail(material) {
    return T.position(0.0, 0.0, nozzle_z + trail_length * 0.5) {
        name = "sta_rocket_trail"
        T.rotation(-Math.pi / 2.0, 0.0, 0.0) {
            T.scale(trail_width, trail_length, 1.0) {
                R.plane() { material C.rgba(1.0, 1.0, 1.0, 0.95) }
            }
            T.rotation(0.0, Math.pi / 2.0, 0.0) {
                T.scale(trail_width, trail_length, 1.0) {
                    R.plane() { material C.rgba(1.0, 1.0, 1.0, 0.95) }
                }
            }
        }
    }
}

export fn apply_rocket_impact(point, amount, rotation) {
    Damage.at(point[0], point[1], point[2], 1.5, amount)
    rocket_explosion(point, rotation)
}

// All flight acceleration and collision checks run in native fixed steps.
export fn launch_rocket(round, spec) {
    let state = { alive = true }
    let zone = Zone.sphere(0.055) { ProjectileImpact {} }
    let model = round.query("[name='sta_rocket_model']")
    // A glowing exhaust cone: its base fills the rear face and it narrows
    // to a point behind the rocket (the unit cone's tip is local +Z, away
    // from the -Z flight direction).
    let nozzle = T.position(0.0, 0.0, nozzle_z + flame_length * 0.5) {
        name = "sta_rocket_nozzle"
        emissive_surface()
        Emissive.on().intensity(3.0)
        T.scale(0.038, 0.038, flame_length) {
            R.cone() { C.rgba(1.0, 0.45, 0.04, 1.0) }
        }
    }
    let launch = spec.pose.translation
    let trail = exhaust_trail(toon_rocket_exhaust(
        [launch[0], launch[1], launch[2]],
        forward_of(spec.pose.rotation_quat_xyzw),
        trail_length,
    ))
    let motion = Velocity {}
    let root = T {
        name = "sta_rocket_flight"
        Raycastable.disabled()
        Acceleration.local([0.0, 0.0, -12.0]).max_speed(18.0) { motion }
    }
    root
    model.attach(nozzle)
    round.attach(zone)
    round.attach(trail)
    motion.attach(round)
    root.world_trs({ translation = spec.pose.translation rotation_quat_xyzw = spec.pose.rotation_quat_xyzw scale = [1.0, 1.0, 1.0] })
    motion.translate([0.0, 0.0, -2.5])
    let expiry = Animation.paused().length(12.0) {
        Keyframe.at(0.0) {}
        Keyframe.at(12.0) {
            state.alive = false
            root.remove_subtree()
        }
    }
    on(zone, "ProjectileHit", fn(event) {
        if !state.alive { return }
        state.alive = false
        let point = event.point
        let rotation = root.world_trs().rotation_quat_xyzw
        if spec["impact"] { spec.impact(point, 8.0, rotation) }
        else { apply_rocket_impact(point, 8.0, rotation) }
        if spec["on_hit"] != null {
            spec.on_hit({ hit_point = point normal = event.normal other_zone = event.other_zone source_tool = spec.source_tool source_user = spec.source_user projectile = root damage = 8.0 })
        }
        expiry.pause()
        // Velocity stopped at the contact, so the trail stays where it was laid
        // down. Hide the rocket and fade the smoke out over half a second.
        model.remove_subtree()
        let fade = Animation.paused().length(1.0) {
            Keyframe.at(0.25) { trail.set_color([1.0, 1.0, 1.0, 0.7]) }
            Keyframe.at(0.5) { trail.set_color([1.0, 1.0, 1.0, 0.45]) }
            Keyframe.at(0.75) { trail.set_color([1.0, 1.0, 1.0, 0.2]) }
            Keyframe.at(1.0) { root.remove_subtree() }
        }
        root.attach(fade)
        fade.play()
    })
    root.attach(expiry)
    expiry.play()
    return root
}

export fn rocket(spec) {
    return launch_rocket(sta_rocket_body("sta_rocket_body"), spec)
}
