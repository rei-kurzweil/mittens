import { ship_solid } from "../ship_parts.mms"

// Wheels rotate a Y-axis cylinder onto X, axle end faces outward.
export fn ship_vehicle_wheel(x, z, radius, hub_material) {
    let roll = -Math.pi / 2.0
    if x < 0.0 { roll = Math.pi / 2.0 }
    return T.position(x, radius, z).rotation(0.0, 0.0, roll) {
        R.cylinder(radius, 0.22, 16) { C.rgba(0.035, 0.040, 0.038, 1.0) }
        T.position(0.0, 0.225, 0.0) {
            R.cylinder(radius * 0.50, 0.015, 12) {
                C.rgba(0.46, 0.49, 0.44, 1.0)
                if hub_material != null {
                    hub_material
                    Texture.with_uri("assets/textures/pool/chrome-matcap.png")
                    TextureFiltering.linear()
                }
            }
        }
    }
}

export fn ship_vehicle_window(position, size) {
    return T {
        ship_solid(position, size, [0.035, 0.095, 0.12])
        ship_solid([position[0], position[1] + size[1] * 0.28, position[2]],
            [size[0] * 1.01, size[1] * 0.07, size[2] * 1.01], [0.16, 0.28, 0.30])
    }
}
