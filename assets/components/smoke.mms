import { smoke_material } from "materials/smoke.mms"

fn setting(options, key, fallback) {
    if options[key] != null { return options[key] }
    return fallback
}

fn eased(value) {
    let t = Math.clamp(value, 0.0, 1.0)
    return t * t * (3.0 - 2.0 * t)
}

fn hash01(value) {
    let x = Math.sin(value * 12.9898 + 78.233) * 43758.5453
    return x - Math.floor(x)
}

// Bounded world-space smoke pool. Attach root to the scene, source to the prop,
// then call update(dt_sec) from the scene's existing FrameTick handler.
// Each section is a cached 300-triangle rounded cube, not an implicit surface.
export fn smoke(options) {
    if options == null { options = {} }
    let label = setting(options, "name", "smoke")
    let capacity = Math.floor(Math.clamp(setting(options, "capacity", 12), 1.0, 64.0))
    let lifetime = Math.clamp(setting(options, "lifetime", 1.6), 0.05, 20.0)
    let interval = Math.clamp(setting(options, "interval", 0.12), 0.02, 10.0)
    let size = Math.clamp(setting(options, "size", 0.065), 0.0001, 10.0)
    let growth = Math.clamp(setting(options, "growth", 2.6), 1.0, 10.0)
    let velocity = setting(options, "velocity", [0.0, 0.16, 0.0])
    let acceleration = setting(options, "acceleration", [0.0, 0.035, 0.0])
    let spread = Math.clamp(setting(options, "spread", 0.025), 0.0, 10.0)
    let inherit_velocity = Math.clamp(setting(options, "inherit_velocity", 0.15), 0.0, 1.0)
    let wobble = Math.clamp(setting(options, "wobble", 0.025), 0.0, 10.0)
    let wave_speed = Math.clamp(setting(options, "wave_speed", 4.0), 0.0, 30.0)
    let step_rate = Math.clamp(setting(options, "step_rate", 0.0), 0.0, 60.0)
    let fade_in = Math.clamp(setting(options, "fade_in", Math.clamp(lifetime * 0.04, 0.001, 0.06)), 0.001, lifetime)
    let fade_out = Math.clamp(setting(options, "fade_out", lifetime * 0.8), 0.001, lifetime)
    // Keep every live section in the ordinary alpha-blended stream, even at
    // its peak. A shader uniform alone cannot choose the renderer's phase.
    let max_opacity = Math.clamp(setting(options, "max_opacity", 0.8), 0.0, 0.98)
    // Controllers such as fire can change these birth settings. Every live
    // cube keeps a snapshot, so new airflow cannot resize old smoke.
    let emission_config = {
        size = size growth = growth interval = interval velocity = velocity
        spread = spread max_opacity = max_opacity
    }
    let seed = setting(options, "seed", 1.0)
    let material = options["material"]
    if material == null { material = smoke_material() }
    let owns_source = options["source"] == null
    let source = options["source"]
    if source == null { source = T { name = label + "_source" } }
    let root = T {
        name = label + "_pool"
        Raycastable.disabled()
    }
    // The current MMS runtime has mutable tables and value arrays. Keep pool
    // slots in a table; do not depend on array push or indexed array mutation.
    let section_indices = range(capacity)
    let sections = {}
    let state = {
        emitting = false disposed = false accumulator = 0.0 cursor = 0
        serial = 0 active = 0 emitted = 0 previous = null
    }
    for index in section_indices {
        let node = T.scale(0.00001, 0.00001, 0.00001) {
            name = label + "_section_" + index
            GLTF.new("assets/models/smoke-rounded-box.glb") { material }
        }
        root.attach(node)
        sections["" + index] = {
            node = node alive = false age = 0.0 phase = 0.0 variation = 1.0
            birth_size = size growth = growth max_opacity = max_opacity
            origin = [0.0, 0.0, 0.0] velocity = [0.0, 0.0, 0.0]
            pose = {
                translation = [0.0, 0.0, 0.0]
                rotation_quat_xyzw = [0.0, 0.0, 0.0, 1.0]
                scale = [0.00001, 0.00001, 0.00001]
            }
        }
    }

    fn hide(section) {
        section.node.set_color([1.0, 1.0, 1.0, 0.0])
        // Nonzero hidden scale keeps the standard vertex normal matrix valid.
        section.node.update_transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.00001, 0.00001, 0.00001])
    }

    fn draw_section(section) {
        let age = section.age
        let u = age / lifetime
        let shape_age = age
        if step_rate > 0.0 { shape_age = Math.floor(age * step_rate) / step_rate }
        let phase = section.phase
        let wave = shape_age * wave_speed + phase
        let diameter = section.birth_size * (1.0 + (section.growth - 1.0) * eased(u))
        section.pose.translation = [
            section.origin[0] + section.velocity[0] * age + 0.5 * acceleration[0] * age * age + wobble * u * (Math.sin(wave) - Math.sin(phase)),
            section.origin[1] + section.velocity[1] * age + 0.5 * acceleration[1] * age * age,
            section.origin[2] + section.velocity[2] * age + 0.5 * acceleration[2] * age * age + wobble * u * (Math.cos(wave * 0.7) - Math.cos(phase * 0.7)),
        ]
        section.pose.scale = [diameter, diameter, diameter]
        let half_yaw = phase * 0.15 + shape_age * 0.12
        section.pose.rotation_quat_xyzw = [0.0, Math.sin(half_yaw), 0.0, Math.cos(half_yaw)]
        section.node.world_trs(section.pose)
        let alpha = section.max_opacity * eased(age / fade_in) * eased((lifetime - age) / fade_out)
        section.node.set_color([1.0, 1.0, 1.0, alpha])
    }

    fn emit_one(position, age, inherited) {
        // Live sections are never recycled early. Excess births are dropped
        // when the pool is full; repeated shots cannot increase its capacity.
        for offset in section_indices {
            let index = (state.cursor + offset) % capacity
            let section = sections["" + index]
            if !section.alive {
                state.cursor = (index + 1) % capacity
                state.serial = state.serial + 1
                state.active = state.active + 1
                state.emitted = state.emitted + 1
                let random = seed + state.serial * 7.0
                section.alive = true
                section.age = age
                section.phase = hash01(random) * 6.283185307
                section.variation = 0.82 + hash01(random + 1.0) * 0.36
                section.birth_size = emission_config.size * section.variation
                section.growth = emission_config.growth
                section.max_opacity = emission_config.max_opacity
                section.origin = [position[0], position[1], position[2]]
                section.velocity = [
                    emission_config.velocity[0] + (hash01(random + 2.0) * 2.0 - 1.0) * emission_config.spread + inherited[0],
                    emission_config.velocity[1] + (hash01(random + 3.0) * 2.0 - 1.0) * emission_config.spread + inherited[1],
                    emission_config.velocity[2] + (hash01(random + 4.0) * 2.0 - 1.0) * emission_config.spread + inherited[2],
                ]
                draw_section(section)
                return true
            }
        }
        return false
    }

    fn start() {
        if state.disposed || state.emitting { return }
        state.emitting = true
        state.accumulator = emission_config.interval
        state.previous = null
    }

    fn stop() {
        state.emitting = false
        state.accumulator = 0.0
        state.previous = null
    }

    fn burst_at(position, count) {
        if state.disposed { return 0 }
        let requested = Math.floor(Math.clamp(count, 0.0, capacity))
        let spawned = 0
        for index in range(requested) {
            if emit_one(position, 0.0, [0.0, 0.0, 0.0]) { spawned = spawned + 1 }
        }
        return spawned
    }

    fn burst(count) {
        if state.disposed { return 0 }
        return burst_at(source.world_trs().translation, count)
    }

    // Call after teleporting the source to avoid interpolating a trail across
    // the jump or inheriting its apparent velocity. Existing smoke stays put.
    fn reset_source() { state.previous = null }

    fn set_emission(parameters) {
        if state.disposed || parameters == null { return false }
        let fraction = Math.clamp(state.accumulator / emission_config.interval, 0.0, 1.0)
        emission_config.size = Math.clamp(setting(parameters, "size", emission_config.size), 0.0001, 10.0)
        emission_config.growth = Math.clamp(setting(parameters, "growth", emission_config.growth), 1.0, 10.0)
        emission_config.interval = Math.clamp(setting(parameters, "interval", emission_config.interval), 0.02, 10.0)
        emission_config.velocity = setting(parameters, "velocity", emission_config.velocity)
        emission_config.spread = Math.clamp(setting(parameters, "spread", emission_config.spread), 0.0, 10.0)
        emission_config.max_opacity = Math.clamp(setting(parameters, "max_opacity", emission_config.max_opacity), 0.0, 0.98)
        state.accumulator = fraction * emission_config.interval
        return true
    }

    fn emission() {
        return {
            size = emission_config.size growth = emission_config.growth
            interval = emission_config.interval velocity = emission_config.velocity
            spread = emission_config.spread max_opacity = emission_config.max_opacity
        }
    }

    fn update(dt_sec) {
        if state.disposed || dt_sec <= 0.0 { return }
        for index in section_indices {
            let section = sections["" + index]
            if section.alive {
                section.age = section.age + dt_sec
                if section.age >= lifetime {
                    section.alive = false
                    state.active = state.active - 1
                    hide(section)
                } else {
                    draw_section(section)
                }
            }
        }
        if !state.emitting { return }
        let current = source.world_trs().translation
        let previous = state.previous
        if previous == null { previous = current }
        let inherited = [
            (current[0] - previous[0]) / dt_sec * inherit_velocity,
            (current[1] - previous[1]) / dt_sec * inherit_velocity,
            (current[2] - previous[2]) / dt_sec * inherit_velocity,
        ]
        let total = state.accumulator + dt_sec
        let due = Math.floor(total / emission_config.interval)
        state.accumulator = total - due * emission_config.interval
        // A long frame does not produce an unbounded catch-up loop. Keep only
        // the newest capacity births and skip those already past their life.
        let count = Math.clamp(due, 0.0, capacity)
        for index in range(count) {
            let age = state.accumulator + (count - 1.0 - index) * emission_config.interval
            if age < lifetime {
                let fraction = Math.clamp(1.0 - age / dt_sec, 0.0, 1.0)
                let position = [
                    previous[0] + (current[0] - previous[0]) * fraction,
                    previous[1] + (current[1] - previous[1]) * fraction,
                    previous[2] + (current[2] - previous[2]) * fraction,
                ]
                emit_one(position, age, inherited)
            }
        }
        state.previous = current
    }

    fn clear() {
        if state.disposed { return }
        stop()
        for index in section_indices {
            let section = sections["" + index]
            if section.alive { hide(section) }
            section.alive = false
        }
        state.active = 0
    }

    fn dispose() {
        if state.disposed { return }
        clear()
        state.disposed = true
        root.remove_subtree()
        if owns_source { source.remove_subtree() }
    }

    fn stats() {
        return {
            capacity = capacity active = state.active emitted = state.emitted
            emitting = state.emitting disposed = state.disposed
        }
    }

    return {
        root = root source = source material = material
        start = start stop = stop burst = burst burst_at = burst_at
        set_emission = set_emission emission = emission
        update = update clear = clear dispose = dispose stats = stats reset_source = reset_source
    }
}
