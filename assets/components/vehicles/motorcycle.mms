import { ship_solid } from "../ship_parts.mms"
import { ship_vehicle_wheel } from "./ship_vehicle_parts.mms"

// WIP motorcycle factory: nose -Z, metres. Optional rider is a seat-local tree.
// Cube/zone pairs retain their source transforms for the existing CSG damage
// system. Wheels are visual trim; frame, tank, seat and forks take damage.
export fn motorcycle(color, rider) {
    if color == null { color = [0.48, 0.24, 0.13] }
    return T {
        name = "motorcycle"
        Destroyable.threshold(3.0)
        CombineMesh.keep_transforms() {
            ship_solid([0.0, 0.52, 0.0], [0.28, 0.24, 1.65], [0.22, 0.28, 0.25])
            ship_solid([0.0, 0.86, -0.25], [0.50, 0.40, 0.70], color)
            ship_solid([0.0, 0.91, 0.42], [0.46, 0.16, 0.74], [0.10, 0.14, 0.12])
            for x in [-0.15, 0.15] {
                ship_solid([x, 0.62, -0.92], [0.065, 0.68, 0.09], [0.51, 0.56, 0.51])
                ship_solid([x, 0.52, 0.82], [0.065, 0.30, 0.12], [0.33, 0.39, 0.34])
            }
            ship_solid([0.0, 1.18, -0.55], [0.66, 0.065, 0.08], [0.28, 0.34, 0.30])
            for side in [-1.0, 1.0] {
                T { name = "motorcycle_handlebar_" + side
                    ship_solid([side * 0.38, 1.30, -0.38], [0.22, 0.075, 0.085], [0.10, 0.14, 0.12])
                    ship_solid([side * 0.32, 1.24, -0.47], [0.065, 0.15, 0.24], [0.44, 0.50, 0.45])
                }
                T { name = "motorcycle_footrest_" + side
                    ship_solid([side * 0.32, 0.39, 0.0], [0.28, 0.06, 0.26], [0.28, 0.34, 0.30])
                }
            }
            ship_solid([0.0, 0.91, -1.02], [0.20, 0.20, 0.14], [0.75, 0.70, 0.46])
            T.position(0.22, 0.0, 0.0) {
                for z in [-0.94, 0.94] { ship_vehicle_wheel(-0.11, z, 0.34) }
            }
        }
        // A sibling of the retained mesh, so batching never consumes the rider.
        T.position(0.0, 0.99, 0.18).rotation(0.0, Math.pi, 0.0) {
            name = "motorcycle_rider_mount"
            if rider != null { rider }
        }
    }
}
