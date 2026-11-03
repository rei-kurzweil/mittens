import { ship_box } from "./ship_parts.mms"

// Structural dome with clear panels; dimensions are metres.
export fn ship_biodome(label, width, length, drum, crown, door_a, door_b) {
    let half_sector = Math.pi / 16.0
    return T.scale(width/2.0, crown, length/2.0) {
        name = label
        // Retain the dome skeleton, leaving the sky completely unobstructed.
        for band in range(6) {
            let a = band * Math.pi / 12.0
            let b = (band + 1) * Math.pi / 12.0
            let r0 = Math.cos(a)
            let r1 = Math.cos(b)
            let y0 = Math.sin(a)
            let y1 = Math.sin(b)
            let z0 = r0 * Math.cos(half_sector)
            let z1 = r1 * Math.cos(half_sector)
            let length = Math.sqrt((y1-y0)*(y1-y0) + (z1-z0)*(z1-z0))
            for sector in range(16) {
                T.rotation(0.0, sector * Math.pi / 8.0, 0.0) {
                    T.position(0.0, drum/crown + (y0+y1)/2.0, (z0+z1)/2.0)
                        .rotation(Math.atan2(z1-z0, y1-y0), 0.0, 0.0) {
                        // Structural ribs only: no transparent layers across the sky.
                        ship_box([0.0, 0.0, 0.0], [0.007, length, 0.007], [0.35, 0.49, 0.46])
                    }
                }
            }
        }
        for sector in range(16) {
            T.rotation(0.0, sector * Math.pi / 8.0, 0.0) {
                // Leave the requested entrance sectors open at floor level.
                if sector != door_a && sector != door_b {
                    ship_box([0.0, 0.04, Math.cos(half_sector)],
                        [2.0*Math.sin(half_sector), 0.08, 0.008], [0.25, 0.39, 0.35])
                    ship_box([0.0, drum/crown/2.0, Math.cos(half_sector)],
                        [0.007, drum/crown, 0.012], [0.35, 0.49, 0.46])
                }
            }
        }
    }
}

