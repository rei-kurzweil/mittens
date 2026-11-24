import { smoke } from "../smoke.mms"
import { player_death } from "../player_death.mms"

// A vehicle's vital part (an engine block, a mech's core) by damage
// received: smoking at 50%, burning at 25% (a glowing flame and light),
// wrecked at 0 (the death burst, then the owner's on_wrecked hook). The
// smoke pool stays in the world so the trail streams behind a moving
// vehicle; it updates per frame only while smoking.
// options: prefix (names), position (vehicle space), max, reach,
//          smoke_size, flame_scale (size of the burning glow)
// Returns { part (attach to the vehicle frame), integrity, wrecked(),
//           repair(), set_on_wrecked(callback) }.
fn flame(prefix, scale) {
    return T.position(0.0, 0.15, 0.0) {
        name = prefix + "_fire"
        Raycastable.disabled()
        T.scale(0.45 * scale, 0.30 * scale, 0.45 * scale) {
            R.sphere() {
                C.rgba(1.0, 0.45, 0.12, 1.0)
                Material(MaterialDefinition.toon())
                EM.on().intensity(2.4)
            }
        }
        PL.color(1.0, 0.55, 0.20).intensity(3.0).distance(6.0 * scale)
    }
}

export fn core_damage(options) {
    let prefix = options.prefix
    let p = options.position
    let smoke_size = 0.30
    if options["smoke_size"] != null { smoke_size = options.smoke_size }
    let flame_scale = 1.0
    if options["flame_scale"] != null { flame_scale = options.flame_scale }
    let state = { wrecked = false smoke = null tick = null fire = null on_wrecked = null }
    let integrity = Integrity.max(options.max).reach(options.reach)
        .threshold("smoking", 0.5).threshold("burning", 0.25).threshold("wrecked", 0.0) {
        name = prefix + "_integrity"
    }
    let source = T.position(0.0, 0.35, 0.0) { name = prefix + "_smoke_source" }
    let part = T.position(p[0], p[1], p[2]) { name = prefix integrity source }

    fn start_smoke() {
        if state.smoke { return }
        let effect = smoke({
            name = prefix + "_smoke" capacity = 24 size = smoke_size lifetime = 2.4 growth = 2.4
            interval = 0.10 velocity = [0.0, 0.9, 0.0] max_opacity = 0.7
        })
        // The pool stays in the world, so the trail streams behind the vehicle.
        effect.root
        source.attach(effect.source)
        effect.start()
        state.smoke = effect
        state.tick = on(world, "FrameTick", fn(event) { effect.update(event.dt_sec) })
    }
    fn start_fire() {
        if state.fire { return }
        let fire = flame(prefix, flame_scale)
        source.attach(fire)
        state.fire = fire
    }
    on(integrity, "DataEvent", fn(event) {
        if event == "smoking" { start_smoke() }
        if event == "burning" {
            start_smoke()
            start_fire()
        }
        if event == "wrecked" {
            state.wrecked = true
            player_death(part.world_trs().translation, 2.0)
            if state.on_wrecked { state.on_wrecked() }
        }
    })
    fn repair() {
        integrity.repair()
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
        part = part
        integrity = integrity
        repair = repair
        wrecked = fn() { return state.wrecked }
        set_on_wrecked = fn(callback) { state.on_wrecked = callback }
    }
}
