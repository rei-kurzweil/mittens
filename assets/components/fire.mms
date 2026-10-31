import { smoke } from "smoke.mms"
import { burning_material } from "materials/burning.mms"

fn setting(options, key, fallback) {
    if options[key] != null { return options[key] }
    return fallback
}

// A burning source owns its burn progression and one reusable smoke pool.
// Attach root to the world, source beneath the fire/ember tip, and material to
// the fuel mesh. The caller supplies update(dt_sec); no global handler is added.
export fn fire(options) {
    if options == null { options = {} }
    let label = setting(options, "name", "fire")
    let smoke_options = setting(options, "smoke_options", { name = label + "_smoke" })
    let effect = smoke(smoke_options)
    let base = effect.emission()
    let material = options["material"]
    if material == null { material = burning_material() }
    let burn_rate = Math.clamp(setting(options, "burn_rate", 0.025), 0.0, 10.0)
    let small_size = Math.clamp(setting(options, "smoke_size_low", base.size * 0.7), 0.0001, 10.0)
    let large_size = Math.clamp(setting(options, "smoke_size_high", base.size * 1.6), small_size, 10.0)
    let state = {
        lit = false disposed = false seconds = 0.0
        airflow = Math.clamp(setting(options, "airflow", 0.35), 0.0, 1.0)
        burn_progress = Math.clamp(setting(options, "burn_progress", 0.0), 0.0, 1.0)
    }

    fn apply_airflow() {
        let air = state.airflow
        // These are authored cartoon response curves, not a fluid simulation.
        // Only future births use them; existing cubes retain their own size,
        // velocity, growth and opacity snapshots inside the smoke component.
        effect.set_emission({
            size = small_size + (large_size - small_size) * air
            interval = base.interval / (0.65 + 0.7 * air)
            velocity = [base.velocity[0], base.velocity[1] * (0.7 + 0.9 * air), base.velocity[2]]
            spread = base.spread * (0.5 + air)
        })
    }

    fn set_airflow(amount) {
        if state.disposed { return }
        let air = Math.clamp(amount, 0.0, 1.0)
        if air == state.airflow { return }
        state.airflow = air
        apply_airflow()
    }

    fn ignite() {
        if state.disposed || state.lit || state.burn_progress >= 1.0 { return false }
        state.lit = true
        effect.start()
        return true
    }

    fn extinguish() {
        if state.disposed { return }
        state.lit = false
        effect.stop()
    }

    fn set_burn_progress(progress) {
        if state.disposed { return }
        state.burn_progress = Math.clamp(progress, 0.0, 1.0)
        material.set_input("burn_progress", state.burn_progress)
        if state.burn_progress >= 1.0 { extinguish() }
    }

    fn update(dt_sec) {
        if state.disposed || dt_sec <= 0.0 { return }
        // Whole pulse periods keep the visible ember animation continuous when
        // wrapping the time value supplied to the fragment shader.
        state.seconds = (state.seconds + dt_sec) % 628.318530718
        if state.lit {
            material.set_input("time_seconds", state.seconds)
            let remaining = 1.0 - state.burn_progress
            let rate = burn_rate * (0.5 + 1.5 * state.airflow)
            let burning_time = dt_sec
            if rate > 0.0 { burning_time = Math.clamp(remaining / rate, 0.0, dt_sec) }
            state.burn_progress = Math.clamp(state.burn_progress + burning_time * rate, 0.0, 1.0)
            material.set_input("burn_progress", state.burn_progress)
            // Age smoke while the fire was burning, then stop births at fuel
            // exhaustion and age the remainder without creating new smoke.
            effect.update(burning_time)
            if state.burn_progress >= 1.0 { extinguish() }
            if burning_time < dt_sec { effect.update(dt_sec - burning_time) }
        } else {
            effect.update(dt_sec)
        }
    }

    fn dispose() {
        if state.disposed { return }
        extinguish()
        effect.dispose()
        state.disposed = true
    }

    fn stats() {
        return {
            lit = state.lit disposed = state.disposed airflow = state.airflow
            burn_progress = state.burn_progress smoke = effect.stats()
        }
    }

    apply_airflow()
    material.set_input("burn_progress", state.burn_progress)
    return {
        root = effect.root source = effect.source material = material smoke = effect
        ignite = ignite extinguish = extinguish set_airflow = set_airflow
        set_burn_progress = set_burn_progress update = update dispose = dispose stats = stats
    }
}
