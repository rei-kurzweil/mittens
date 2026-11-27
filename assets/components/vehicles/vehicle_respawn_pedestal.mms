import { button } from "../button.mms"

export fn vehicle_respawn_pedestal(material, respawn) {
    let control = button("Respawn vehicles", {
        compact = true
        background_color = [0.88, 0.35, 0.61, 1.0]
        color = [0.12, 0.07, 0.15, 1.0]
    })
    on(control, "Click", fn(event) { respawn() })
    fn solid(x, y, z, width, height, depth, color) {
        return T.position(x, y, z).scale(width, height, depth) {
            R.cube() { material C.rgba(color[0], color[1], color[2], 1.0) }
            Zone.cube([0.5, 0.5, 0.5]) { Collidable.static() {} }
        }
    }
    return T {
        name = "vehicle_respawn_pedestal"
        solid(0.0, 0.07, 0.0, 0.72, 0.14, 0.65, [0.29, 0.26, 0.36])
        solid(0.0, 0.60, 0.0, 0.28, 0.92, 0.31, [0.49, 0.43, 0.57])
        solid(0.0, 1.11, 0.0, 1.0, 0.18, 0.42, [0.29, 0.26, 0.36])
        T.position(-0.38, 1.16, 0.23).scale(0.074, 0.074, 0.074) {
            LayoutRoot { available_width(12.0) available_height(1.5) control }
        }
    }
}
