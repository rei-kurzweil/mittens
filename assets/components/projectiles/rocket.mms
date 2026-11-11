import { rocket_explosion } from "./rocket_explosion.mms"

// All flight acceleration and collision checks run in native fixed steps.
export fn rocket(spec) {
    let state = { alive = true }
    let zone = Zone.sphere(0.055) { ProjectileImpact {} }
    let body = T {
        name = "sta_rocket_body"
        zone
        T.position(0.191356, -1.507589, -0.111893) {
            GLTF.new("assets/models/STA_rocket_bullet.glb") {}
        }
    }
    let motion = Velocity { body }
    let root = T {
        name = "sta_rocket_flight"
        Raycastable.disabled()
        Acceleration.local([0.0, 0.0, -12.0]).max_speed(18.0) { motion }
    }
    root
    root.world_trs({ translation = spec.pose.translation rotation_quat_xyzw = spec.pose.rotation_quat_xyzw scale = [1.0, 1.0, 1.0] })
    motion.translate([0.0, 0.0, -2.0])
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
        Damage.at(point[0], point[1], point[2], 1.5, 8.0)
        let effect = rocket_explosion(point)
        effect.root
        effect.animation.play()
        if spec["on_hit"] != null {
            spec.on_hit({ hit_point = point normal = event.normal other_zone = event.other_zone source_tool = spec.source_tool source_user = spec.source_user projectile = root damage = 8.0 })
        }
        expiry.pause()
        root.remove_subtree()
    })
    root.attach(expiry)
    expiry.play()
    return root
}
