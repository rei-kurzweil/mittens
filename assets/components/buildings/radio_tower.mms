import { truss, truss_span } from "../truss.mms"
import { matcap_metal } from "../materials/matcap_metal.mms"

// A guyed lattice tower: three truss sections of dark galvanised matcap
// steel, beacons that blink large and red (tiny between blinks), two sets of
// three guy wires anchored at a third and two thirds of the height, dish
// drums facing outward and tall panel antennas on top.
//
// Destructible by damage: each section has an Integrity. A section that
// takes enough damage breaks off with everything above it (the sections are
// nested), falling as a free body with gravity, a push and a spin, and is
// removed once it has fallen for a while. Bars aren't cut: thin truss
// members would need one CSG surface each.
let SECTION_HEALTH = 80.0
let BEACON_OFF = 0.08
let BEACON_ON = 1.4
let FALL_SECONDS = 15.0

fn wire(from, to) {
    let d = [to[0] - from[0], to[1] - from[1], to[2] - from[2]]
    let horizontal = Math.sqrt(d[0] * d[0] + d[2] * d[2])
    let length = Math.sqrt(horizontal * horizontal + d[1] * d[1])
    let yaw = Math.atan2(d[0], d[2])
    let elevation = Math.atan2(d[1], horizontal)
    return T.position((from[0] + to[0]) / 2.0, (from[1] + to[1]) / 2.0, (from[2] + to[2]) / 2.0)
        .rotation(0.0, yaw, 0.0) {
        T.rotation(-elevation, 0.0, 0.0).scale(0.025, 0.025, length) {
            R.cube() { C.rgba(0.55, 0.56, 0.60, 1.0) }
        }
    }
}

// { frame, glow }: the beacon starts lit, like the first blink.
fn beacon(index, height) {
    let glow = EM.on().intensity(3.8) {}
    let frame = T.position(0.0, height, 0.0).scale(BEACON_ON, BEACON_ON, BEACON_ON) {
        name = "radio_tower_beacon_" + index
        R.sphere() {
            C.rgba(0.96, 0.07, 0.035, 1.0)
            Material(MaterialDefinition.toon())
            glow
        }
    }
    return { frame = frame glow = glow }
}

// Dish drums (flat faces outward) and panel antennas around the top.
fn top_equipment(height) {
    return T.position(0.0, height, 0.0) {
        for index in range(3) {
            T.rotation(0.0, index * 2.0 * Math.pi / 3.0, 0.0) {
                T.position(0.0, -1.4, 0.95).rotation(Math.pi / 2.0, 0.0, 0.0) {
                    R.cylinder(0.42, 0.28, 24) { C.rgba(0.86, 0.87, 0.88, 1.0) }
                }
            }
        }
        for index in range(4) {
            T.rotation(0.0, Math.pi / 4.0 + index * Math.pi / 2.0, 0.0) {
                T.position(0.0, -0.2, 0.85).scale(0.22, 1.6, 0.10) {
                    R.cube() { C.rgba(0.80, 0.81, 0.83, 1.0) }
                }
            }
        }
    }
}

export fn radio_tower(label, material) {
    let bays = 8
    let span = truss_span(bays)
    let height = 3.0 * span
    let steel = matcap_metal(0.42)
    // b0..b2: which sections broke; fall0..fall2: their falling bodies.
    let state = {}
    let events = T { name = "radio_tower_events" }
    let beacons = [beacon(0, span * 0.5), beacon(1, span * 0.5), beacon(2, span * 0.5 + 0.62)]
    let integrities = [
        Integrity.max(SECTION_HEALTH).reach(span * 0.5).threshold("broken", 0.0) { name = "radio_tower_integrity_0" },
        Integrity.max(SECTION_HEALTH).reach(span * 0.5).threshold("broken", 0.0) { name = "radio_tower_integrity_1" },
        Integrity.max(SECTION_HEALTH).reach(span * 0.5).threshold("broken", 0.0) { name = "radio_tower_integrity_2" },
    ]

    // One truss section, centred `y` above its parent; `above` nests the next
    // section so breaking this one takes it along.
    fn section(index, y, above, equipment) {
        return T.position(0.0, y, 0.0) {
            name = "radio_tower_section_" + index
            steel
            Texture.with_uri("assets/textures/pool/chrome-matcap.png")
            Climbable {}
            Raycastable.enabled()
            Zone.cube([0.72, span / 2.0, 0.72]) { Collidable.static().friction(0.85) {} }
            T.rotation(0.0, 0.0, Math.pi / 2.0) {
                // Retain the individual bars for grip rays alongside the visual batch.
                truss(bays, true)
            }
            integrities[index]
            beacons[index].frame
            if above { above }
            if equipment { equipment }
        }
    }
    let top = section(2, span, null, top_equipment(span * 0.5 + 0.62))
    let middle = section(1, span, top, null)
    let bottom = section(0, span * 0.5, middle, null)
    let sections = [bottom, middle, top]

    fn break_off(index) {
        // A section already falling with a lower one stays with it.
        for below in range(index + 1) {
            if state["b" + below] == true { return }
        }
        state["b" + index] = true
        let section = sections[index]
        let pose = section.world_trs()
        let body = Velocity { name = "radio_tower_falling_" + index }
        let fall = Gravity { body }
        // A free body in the world, carrying its section (and those above).
        fall
        state["fall" + index] = fall
        body.attach(section)
        section.world_trs(pose)
        let contact = Zone.sphere(1.0) { Collidable.slide() {} }
        section.attach(contact)
        let side = 1.0
        if index == 1 { side = -1.0 }
        body.translate([side * 1.5, 0.8, 0.9])
        body.rotate([0.35, 0.2, side * 0.45])
        let expiry = Animation.paused().length(FALL_SECONDS * 2.0) {
            Keyframe.at(0.0) {}
            Keyframe.at(FALL_SECONDS * 2.0) { emit_data(events, "FallenAway" + index) }
        }
        events.attach(expiry)
        expiry.play()
    }
    // One handler per section, registered by a function so each keeps its index.
    fn watch(index) {
        on(integrities[index], "DataEvent", fn(event) {
            if event == "broken" { break_off(index) }
        })
    }
    watch(0)
    watch(1)
    watch(2)
    on(events, "DataEvent", fn(event) {
        for index in range(3) {
            let fall = state["fall" + index]
            if event == "FallenAway" + index && fall {
                fall.remove_subtree()
                state["fall" + index] = null
            }
        }
    })

    // The beacons blink large and red, and nearly vanish between blinks.
    let blink_events = T { name = "radio_tower_blink_events" }
    let blink = Animation.looping().length(4.0) {
        Keyframe.at(0.0) { emit_data(blink_events, "BeaconOn") }
        Keyframe.at(0.7) { emit_data(blink_events, "BeaconOff") }
    }
    on(blink_events, "DataEvent", fn(event) {
        let lit = event == "BeaconOn"
        let size = BEACON_OFF
        if lit { size = BEACON_ON }
        for light in beacons {
            light.frame.update_transform(light.frame.trs().translation, [0.0, 0.0, 0.0], [size, size, size])
            if lit { light.glow.set_intensity(3.8) } else { light.glow.off() }
        }
    })

    // Guy wires: three at a third of the height and three at two thirds,
    // anchored around the base at 120 degrees.
    let anchors = T {
        name = "radio_tower_guy_wires"
        for level in [1.0, 2.0] {
            for index in range(3) {
                let angle = index * 2.0 * Math.pi / 3.0 + level * 0.5
                let radius = height * 0.42
                wire([Math.sin(angle) * radius, 0.0, Math.cos(angle) * radius],
                    [Math.sin(angle) * 0.7, height * level / 3.0, Math.cos(angle) * 0.7])
            }
        }
    }
    return T {
        name = label
        bottom
        anchors
        events
        blink_events
        blink
    }
}
