import { ship_solid, ship_box } from "./ship_parts.mms"

// Bridge swivel chair, seated occupant looks toward -Z.
export fn chair() {
    return T {
        name = "chair"
        ship_solid([0.0, 0.47, 0.0], [0.58, 0.12, 0.55], [0.17, 0.25, 0.23])
        ship_solid([0.0, 0.85, 0.23], [0.58, 0.68, 0.10], [0.17, 0.25, 0.23])
        ship_box([0.0, 0.23, 0.0], [0.10, 0.46, 0.10], [0.44, 0.49, 0.47])
        ship_box([0.0, 0.045, 0.0], [0.65, 0.09, 0.12], [0.25, 0.30, 0.28])
        ship_box([0.0, 0.045, 0.0], [0.12, 0.09, 0.65], [0.25, 0.30, 0.28])
        for side in [-1.0, 1.0] {
            ship_box([side * 0.33, 0.66, 0.02], [0.08, 0.08, 0.44], [0.32, 0.39, 0.35])
        }
    }
}
