import { emissive_surface } from "./materials/emissive_surface.mms"

// Independent hitscan projectile slot. The caller owns muzzle, sound and cadence.
// launch({ pose, source_tool, source_user, on_hit }) takes copied world TRS.
// Reuse a slot after its short animation finishes; concurrent shots need separate slots.
// The beam waits for the raycast (about a frame), then runs from the muzzle to
// the hit, or its full length on a miss, so it never shows through cover.
export fn laser(options) {
    let label = "laser"
    if options["name"] != null { label = options.name }
    let beam_name = label + "_beam"
    if options["beam_name"] != null { beam_name = options.beam_name }
    let length = 40.0
    if options["length"] != null { length = options.length }
    let radius = 0.008
    if options["beam_radius"] != null { radius = options.beam_radius }
    let origin_offset = 0.0
    if options["ray_origin_offset"] != null { origin_offset = options.ray_origin_offset }
    let beats_per_second = 2.0
    if options["beats_per_second"] != null { beats_per_second = options.beats_per_second }
    let emission = Emissive.off()
    let raycaster = Raycast.event_driven().parent_forward(true).nearest_hit_only(true).max_distance(length) {
        name = label + "_raycaster"
    }

    let beam = T.position(0.0, 0.0, -origin_offset - length / 2.0).scale(0.0, 0.0, 0.0) {
        name = beam_name
        T.position(0.0, 0.0, length / 2.0).rotation(-1.57079633, 0.0, 0.0) {
            R.cylinder(radius, length, 8) {
                emissive_surface()
                C.rgba(1.0, 0.12, 0.06, 1.0)
                emission
            }
        }
    }

    let animation = Animation.paused().length(0.04 * beats_per_second) {
        name = label + "_shot"
        Keyframe.at(0.0) {}
        Keyframe.at(0.04 * beats_per_second) {
            beam.update_transform([0.0, 0.0, -origin_offset - length / 2.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0])
            emission.off()
        }
    }

    let root = T {
        name = label + "_flight"
        Raycastable.disabled()
        beam
        T.position(0.0, 0.0, -origin_offset) { raycaster }
        animation
    }
    root
    // waiting: a shot whose raycast hasn't answered yet. pending_hit: that
    // shot is a local one, so a hit reaches on_hit.
    let flight_state = { pending_hit = false waiting = false context = null }

    // The beam from the muzzle, `distance` long.
    fn show(distance) {
        beam.update_transform([0.0, 0.0, -origin_offset - distance / 2.0], [0.0, 0.0, 0.0],
            [1.0, 1.0, distance / length])
        emission.set_intensity(12.0)
        animation.play()
    }

    on(raycaster, "RayIntersected", fn(event) {
        if event.raycaster != raycaster || !flight_state.waiting { return }
        flight_state.waiting = false
        show(event.distance)
        if flight_state.pending_hit {
            flight_state.pending_hit = false
            let context = flight_state.context
            let hit = {
                projectile = root raycaster = raycaster renderable = event.renderable
                hit_point = event.hit_point direction = event.direction origin = event.origin distance = event.distance
                source_tool = context.source_tool source_user = context.source_user
                damage = context.damage
            }
            if context.on_hit != null { context.on_hit(hit) }
        }
    })
    on(raycaster, "RayMissed", fn(event) {
        if !flight_state.waiting { return }
        flight_state.waiting = false
        flight_state.pending_hit = false
        show(length)
    })

    fn launch(spec) {
        // Copy the launch context so later tool/user changes cannot retag a hit.
        let damage=1.0
        if spec["damage"]!=null {damage=spec.damage}
        flight_state.context = { source_tool = spec.source_tool source_user = spec.source_user damage=damage on_hit = spec.on_hit }
        flight_state.pending_hit = true
        flight_state.waiting = true
        root.world_trs(spec.pose)
        raycaster.request_raycast()
    }

    // A remote shot: its own raycast sets the beam's length, but no hit is
    // reported, since the shooter's peer owns the hit.
    fn play_visual(pose) {
        flight_state.pending_hit = false
        flight_state.waiting = true
        root.world_trs(pose)
        raycaster.request_raycast()
    }

    return { root = root animation = animation raycaster = raycaster launch = launch play_visual = play_visual }
}
