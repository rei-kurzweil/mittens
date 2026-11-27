import { core_damage } from "./core_damage.mms"

// The pickup's damage state: it owns operability, read from its parts'
// Integrity (damage received, whether or not the part can be cut).
//   engine block: smoking at 50%, burning at 25%, wrecked at 0 (explodes,
//                 stops driving); see core_damage
//   each wheel:   inoperable at 30%; every dead wheel costs a quarter of
//                 the truck's traction
// Chassis space. Attach `parts` to the chassis frame; the truck's
// set_control_input scales throttle by traction() and brakes when wrecked().
// on_wrecked(callback) lets a scene respawn or remove the truck; repair()
// restores everything and stops the smoke and fire.
let WHEELS = [[-0.80, 0.38, -1.25], [0.80, 0.38, -1.25], [-0.80, 0.38, 1.15], [0.80, 0.38, 1.15]]

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
    let state = { dead_wheels = 0 }
    let integrities = {}
    let engine = core_damage({ prefix = "pickup_engine" position = [0.0, 0.95, -1.45] max = 60.0 reach = 0.7 })
    let wheels = [wheel_part(0, state, integrities), wheel_part(1, state, integrities),
        wheel_part(2, state, integrities), wheel_part(3, state, integrities)]
    let parts = T {
        name = "pickup_damage_parts"
        engine.part
        for wheel in wheels { wheel }
    }
    fn repair() {
        engine.repair()
        for index in range(4) { integrities["" + index].repair() }
        state.dead_wheels = 0
    }
    return {
        parts = parts
        engine = engine.integrity
        repair = repair
        wrecked = engine.wrecked
        traction = fn() {
            if engine.wrecked() { return 0.0 }
            return Math.clamp(1.0 - 0.25 * state.dead_wheels, 0.0, 1.0)
        }
        set_on_wrecked = engine.set_on_wrecked
    }
}
