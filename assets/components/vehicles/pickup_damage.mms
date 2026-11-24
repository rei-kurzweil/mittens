import { smoke } from "../smoke.mms"
import { player_death } from "../player_death.mms"

// The pickup's damage state: it owns operability, read from its parts'
// Integrity (damage received, whether or not the part can be cut).
//   engine block: smoking at 50%, burning at 25%, wrecked at 0 (explodes,
//                 stops driving)
//   each wheel:   inoperable at 30%; every dead wheel costs a quarter of
//                 the truck's traction
// Chassis space. Attach `parts` to the chassis frame; the truck's
// set_control_input scales throttle by traction() and brakes when wrecked().
// on_wrecked(callback) lets a scene respawn or remove the truck; repair()
// restores everything and stops the smoke and fire.
let WHEELS = [[-0.80, 0.38, -1.25], [0.80, 0.38, -1.25], [-0.80, 0.38, 1.15], [0.80, 0.38, 1.15]]

fn flame() {
    return T.position(0.0, 0.15, 0.0) {
        name = "pickup_engine_fire"
        Raycastable.disabled()
        T.scale(0.45, 0.30, 0.45) {
            R.sphere() {
                C.rgba(1.0, 0.45, 0.12, 1.0)
                Material(MaterialDefinition.toon())
                EM.on().intensity(2.4)
            }
        }
        PL.color(1.0, 0.55, 0.20).intensity(3.0).distance(6.0)
    }
}

fn wheel_part(index, state, integrities) {
    let integrity = Integrity.max(30.0).reach(0.45).threshold("inoperable", 0.3) {
        name = "pickup_wheel_integrity_" + index
    }
    on(integrity, "DataEvent", fn(event) {
        if event == "inoperable" { state.dead_wheels = state.dead_wheels + 1 }
    })
    integrities["" + index] = integrity
    let p = WHEELS[index]
    return T.position(p[0], p[1], p[2]) { name = "pickup_wheel_part_" + index integrity }
}

export fn pickup_damage(options) {
    let state = { dead_wheels = 0 wrecked = false smoke = null tick = null fire = null on_wrecked = null }
    let integrities = {}
    let engine = Integrity.max(60.0).reach(0.7)
        .threshold("smoking", 0.5).threshold("burning", 0.25).threshold("wrecked", 0.0) {
        name = "pickup_engine_integrity"
    }
    let source = T.position(0.0, 0.35, 0.0) { name = "pickup_engine_smoke_source" }
    let engine_part = T.position(0.0, 0.95, -1.45) { name = "pickup_engine" engine source }
    let wheels = [wheel_part(0, state, integrities), wheel_part(1, state, integrities),
        wheel_part(2, state, integrities), wheel_part(3, state, integrities)]
    let parts = T {
        name = "pickup_damage_parts"
        engine_part
        for wheel in wheels { wheel }
    }

    fn start_smoke() {
        if state.smoke { return }
        let effect = smoke({
            name = "pickup_engine_smoke" capacity = 24 size = 0.30 lifetime = 2.4 growth = 2.4
            interval = 0.10 velocity = [0.0, 0.9, 0.0] max_opacity = 0.7
        })
        // The pool stays in the world, so the trail streams behind the truck.
        effect.root
        source.attach(effect.source)
        effect.start()
        state.smoke = effect
        state.tick = on(world, "FrameTick", fn(event) { effect.update(event.dt_sec) })
    }
    fn start_fire() {
        if state.fire { return }
        let fire = flame()
        source.attach(fire)
        state.fire = fire
    }
    on(engine, "DataEvent", fn(event) {
        if event == "smoking" { start_smoke() }
        if event == "burning" {
            start_smoke()
            start_fire()
        }
        if event == "wrecked" {
            state.wrecked = true
            player_death(engine_part.world_trs().translation, 2.0)
            if state.on_wrecked { state.on_wrecked() }
        }
    })

    fn repair() {
        engine.repair()
        for index in range(4) { integrities["" + index].repair() }
        state.dead_wheels = 0
        state.wrecked = false
        if state.tick { state.tick.remove() }
        state.tick = null
        if state.smoke {
            state.smoke.stop()
            state.smoke.root.remove_subtree()
        }
        state.smoke = null
        if state.fire { state.fire.remove_subtree() }
        state.fire = null
    }
    return {
        parts = parts
        engine = engine
        repair = repair
        wrecked = fn() { return state.wrecked }
        traction = fn() {
            if state.wrecked { return 0.0 }
            return Math.clamp(1.0 - 0.25 * state.dead_wheels, 0.0, 1.0)
        }
        set_on_wrecked = fn(callback) { state.on_wrecked = callback }
    }
}
