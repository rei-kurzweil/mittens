// Per-component target API: can.on_hit(hit) reports whether this can received it.
// This is visual feedback and a hit counter, without damage or destruction.
import { hit_number } from "../hit-number.mms"
export fn metal_can(options) {
    let label = options.name
    let material = options.material
    let radius = 0.045
    let height = 0.18
    let state = { hits = 0 last_hit = null }
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
    let wobble = T {
        name = label + "_wobble"
        Raycastable.enabled()
        body
        T.position(0.0, height, 0.0) { top }
        T.position(0.0, -0.006, 0.0) { bottom }
    }
    let reaction = Animation.paused().length(0.32) {
        name = label + "_hit_reaction"
        Keyframe.at(0.0) {
            wobble.update_transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.16], [1.0, 1.0, 1.0])
            body.set_color([1.0, 0.95, 0.55, 1.0])
        }
        Keyframe.at(0.12) {
            wobble.update_transform([0.0, 0.0, 0.0], [0.0, 0.0, -0.10], [1.0, 1.0, 1.0])
        }
        Keyframe.at(0.24) {
            wobble.update_transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
            body.set_color(green)
        }
    }
    let root = T.position(options.position[0], options.position[1], options.position[2]) {
        name = label
        wobble
        reaction
    }
    fn handle_hit(hit) {
        if hit.renderable != body && hit.renderable != top && hit.renderable != bottom { return false }
        state.hits = state.hits + 1
        state.last_hit = hit
        let number = hit_number(hit)
        number.root
        number.animation.play()
        reaction.play()
        if options["on_hit"] != null { options.on_hit(hit, state) }
        return true
    }
    root.on_hit = handle_hit
    root.hit_count = fn() { return state.hits }
    // Each raycastable surface exposes the same handler and private state.
    body.on_hit = handle_hit
    top.on_hit = handle_hit
    bottom.on_hit = handle_hit
    return root
}
