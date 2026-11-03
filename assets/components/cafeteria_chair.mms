import { ship_solid, ship_box } from "./ship_parts.mms"

// Separate lightweight cafeteria chair; occupant looks toward -Z.
export fn cafeteria_chair() {
    return T {
        name = "cafeteria_chair"
        ship_solid([0.0, 0.46, 0.0], [0.46, 0.07, 0.46], [0.67, 0.40, 0.15])
        ship_solid([0.0, 0.72, 0.20], [0.46, 0.44, 0.06], [0.67, 0.40, 0.15])
        for x in [-0.18, 0.18] {
            for z in [-0.18, 0.18] {
                ship_box([x, 0.22, z], [0.035, 0.44, 0.035], [0.55, 0.61, 0.59])
            }
        }
    }
}
