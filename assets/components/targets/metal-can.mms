// Per-component target API: can.on_hit(hit) reports whether this can received it.
// Velocity owns motion/spin. Hits and nearby blasts reset a five-second respawn timer.
import { hit_number } from "../hit-number.mms"
export fn metal_can(options) {
    let label = options.name
    let material = options.material
    let radius = 0.045
    let height = 0.18
    let state = { hits = 0 last_hit = null remaining = 0.0 }
    let green = [0.52, 1.0, 0.035, 1.0]
    let body = R.cylinder(radius, height, 24) {
        name = label + "_body"
        Bounds.aabb([-radius, 0.0, -radius], [radius, height, radius])
        material
        C.rgba(green[0], green[1], green[2], green[3])
        Texture.with_uri("assets/textures/pool/chrome-matcap.png")
        TextureFiltering.linear()
    }
    let top = R.cylinder(radius + 0.002, 0.006, 24) {
        name = label + "_top"
        Bounds.aabb([-radius - 0.002, 0.0, -radius - 0.002], [radius + 0.002, 0.006, radius + 0.002])
        material
        C.rgba(0.96, 0.98, 1.0, 1.0)
        Texture.with_uri("assets/textures/pool/chrome-matcap.png")
        TextureFiltering.linear()
    }
    let bottom = R.cylinder(radius + 0.002, 0.006, 24) {
        name = label + "_bottom"
        Bounds.aabb([-radius - 0.002, 0.0, -radius - 0.002], [radius + 0.002, 0.006, radius + 0.002])
        material
        C.rgba(0.96, 0.98, 1.0, 1.0)
        Texture.with_uri("assets/textures/pool/chrome-matcap.png")
        TextureFiltering.linear()
    }
    let wobble = T.position(0.0, -height / 2.0, 0.0) {
        name = label + "_wobble"
        Raycastable.enabled()
        body
        T.position(0.0, height, 0.0) { top }
        T.position(0.0, -0.006, 0.0) { bottom }
    }
    let reaction = Animation.paused().length(0.32) {
        name = label + "_hit_reaction"
        Keyframe.at(0.0) {
            body.set_color([1.0, 0.95, 0.55, 1.0])
        }
        Keyframe.at(0.12) {
        }
        Keyframe.at(0.24) {
            body.set_color(green)
        }
    }
    let body_pose = T.position(0.0, height / 2.0, 0.0) {
        name = label + "_motion_pose"
        Zone.sphere(height / 2.0) { Collidable.slide() {} }
        wobble
    }
    let motion = Velocity { name = label + "_velocity" body_pose }
    let gravity = Gravity.enabled(false) { name = label + "_gravity" motion }
    let root = T.position(options.position[0], options.position[1], options.position[2]) {
        name = label
        gravity
        reaction
    }
    fn kick(impulse, point) {
        let center = body_pose.world_trs().translation
        let r = [point[0] - center[0], point[1] - center[1], point[2] - center[2]]
        let spin = [
            (r[1] * impulse[2] - r[2] * impulse[1]) * 30.0,
            (r[2] * impulse[0] - r[0] * impulse[2]) * 30.0,
            (r[0] * impulse[1] - r[1] * impulse[0]) * 30.0,
        ]
        motion.translate_world(impulse)
        motion.rotate_world(spin)
        gravity.set_enabled(true)
        state.remaining = 5.0
    }
    fn handle_hit(hit) {
        if hit.renderable != body && hit.renderable != top && hit.renderable != bottom { return false }
        state.hits = state.hits + 1
        state.last_hit = hit
        let direction = hit["direction"]
        if direction == null { direction = [0.0, 0.0, -1.0] }
        let length = Math.sqrt(direction[0] * direction[0] + direction[1] * direction[1] + direction[2] * direction[2])
        if length < 0.0001 { length = 1.0 }
        kick([direction[0] * 3.0 / length, direction[1] * 3.0 / length + 1.5, direction[2] * 3.0 / length], hit.hit_point)
        let number = hit_number(hit)
        number.root
        number.animation.play()
        reaction.play()
        if options["on_hit"] != null { options.on_hit(hit, state) }
        return true
    }
    fn handle_blast(hit) {
        if hit.renderable == body || hit.renderable == top || hit.renderable == bottom { return false }
        let center = body_pose.world_trs().translation
        let delta = [center[0] - hit.hit_point[0], center[1] - hit.hit_point[1], center[2] - hit.hit_point[2]]
        let distance = Math.sqrt(delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2])
        if distance >= 0.65 || distance < 0.0001 { return false }
        let strength = (1.0 - distance / 0.65) * 2.0
        kick([delta[0] / distance * strength, delta[1] / distance * strength + strength, delta[2] / distance * strength], hit.hit_point)
        return true
    }
    on_global("FrameTick", fn(event) {
        if state.remaining > 0.0 {
            state.remaining = state.remaining - event.dt_sec
            if state.remaining <= 0.0 {
                motion.reset()
                gravity.set_enabled(false)
                body_pose.update_transform([0.0, height / 2.0, 0.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
                body.set_color(green)
                state.remaining = 0.0
            }
        }
    })
    root.on_hit = handle_hit
    root.on_blast = handle_blast
    root.hit_count = fn() { return state.hits }
    body.on_hit = handle_hit
    top.on_hit = handle_hit
    bottom.on_hit = handle_hit
    return root
}
