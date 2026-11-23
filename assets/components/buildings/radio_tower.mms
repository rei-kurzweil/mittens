import { truss, truss_span } from "../truss.mms"

export fn radio_tower(label, material) {
    let bays = 8
    let span = truss_span(bays)
    let lower_glow = Emissive.on() { intensity(3.8) }
    let upper_glow = Emissive.on() { intensity(3.8) }
    let top_glow = Emissive.on() { intensity(3.8) }
    let beacons = [lower_glow, upper_glow, top_glow]
    let blink = Animation.looping().length(4.0) {
        Keyframe.at(0.0) {
            lower_glow.set_intensity(3.8)
            upper_glow.set_intensity(3.8)
            top_glow.set_intensity(3.8)
        }
        Keyframe.at(0.7) {
            lower_glow.off()
            upper_glow.off()
            top_glow.off()
        }
    }
    return T {
        name = label
        for section in range(3) {
            T.position(0.0, (section + 0.5) * span, 0.0) {
                name = "radio_tower_section_" + section
                material
                Climbable {}
                Raycastable.enabled()
                Zone.cube([0.72, span / 2.0, 0.72]) { Collidable.static().friction(0.85) {} }
                T.rotation(0.0, 0.0, Math.pi / 2.0) {
                    // Retain the individual bars for grip rays alongside the visual batch.
                    truss(bays, true)
                }
            }
            let height = (section + 1) * span
            if section == 2 { height = height + 0.62 }
            let glow = beacons[section]
            T.position(0.0, height, 0.0).scale(1.2, 1.2, 1.2) {
                R.sphere() {
                    name = "radio_tower_beacon_" + section
                    C.rgba(0.96, 0.07, 0.035, 1.0)
                    glow
                }
            }
        }
        blink
    }
}
