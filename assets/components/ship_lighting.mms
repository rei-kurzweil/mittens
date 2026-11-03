import { ship_lamp } from "./ship_parts.mms"

// Emissive markers are spaced fixtures, not flashing lights. They share the
// scene bloom; a few bounded point lights supply actual surface illumination.
export fn ship_path_markers(start, axis, count, spacing, color) {
    return T {
        for index in range(count) {
            ship_lamp([start[0] + axis[0] * index * spacing, start[1],
                start[2] + axis[1] * index * spacing],
                [0.045 + Math.abs(axis[0]) * 0.50, 0.035,
                    0.045 + Math.abs(axis[1]) * 0.50], color)
        }
    }
}

export fn ship_light_pool(position, color, intensity, reach) {
    return T.position(position[0], position[1], position[2]) {
        PL.color(color[0], color[1], color[2]).intensity(intensity).distance(reach)
    }
}
