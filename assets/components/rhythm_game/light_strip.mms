// A strip facing local +Z. The base occupies the XY plane; each light is a
// shallow cube extending forward from it. All light geometry is one baked
// CombineMesh visual with the material of its first cube.
//
// light_strip({ light_count = 12, light_width = 0.22, spacing = 0.08,
//               light_height = 0.025, base_color = [0.08, 0.09, 0.14, 1.0],
//               light_color = [0.2, 0.85, 1.0, 1.0], intensity = 2.0 })
// All fields are optional. spacing is the clear edge-to-edge gap.

export fn light_strip(config) {
    let light_count = 12
    let light_width = 0.22
    let spacing = 0.08
    let light_height = 0.025
    let base_width = 0.34
    let end_padding = 0.12
    let base_color = [0.08, 0.09, 0.14, 1.0]
    let light_color = [0.20, 0.85, 1.0, 1.0]
    let glow_intensity = 2.0

    if config != null {
        if config["light_count"] != null { light_count = config["light_count"] }
        if config["light_width"] != null { light_width = config["light_width"] }
        if config["spacing"] != null { spacing = config["spacing"] }
        if config["light_height"] != null { light_height = config["light_height"] }
        if config["base_width"] != null { base_width = config["base_width"] }
        if config["end_padding"] != null { end_padding = config["end_padding"] }
        if config["base_color"] != null { base_color = config["base_color"] }
        if config["light_color"] != null { light_color = config["light_color"] }
        if config["intensity"] != null { glow_intensity = config["intensity"] }
    }

    let strip_length = light_count * light_width + (light_count - 1) * spacing + 2.0 * end_padding

    return T {
        name = "light_strip"

        T.scale(strip_length, base_width, 1.0) {
            name = "light_strip_base"
            R.plane() {
                C.rgba(base_color[0], base_color[1], base_color[2], base_color[3])
            }
        }

        CombineMesh {
            name = "light_strip_lights"
            for index in range(light_count) {
                let x = (index - (light_count - 1) / 2.0) * (light_width + spacing)
                T.position(x, 0.0, light_height / 2.0 + 0.002)
                    .scale(light_width, light_width, light_height) {
                    R.cube() {
                        C.rgba(light_color[0], light_color[1], light_color[2], light_color[3])
                        EM.on() { intensity(glow_intensity) }
                    }
                }
            }
        }
    }
}
