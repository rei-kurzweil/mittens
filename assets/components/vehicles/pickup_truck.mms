import { ship_solid, ship_lamp } from "../ship_parts.mms"
import { ship_vehicle_wheel, ship_vehicle_window } from "./ship_vehicle_parts.mms"
import { pickup_painted_metal } from "../materials/pickup_painted_metal.mms"

// Destroyable retained body panels; wheels remain visual trim.
// Parked pickup, front toward -Z. Open cargo bed and a two-seat cab.
export fn pickup_truck(color, cargo) {
    if color == null { color = [0.59, 0.52, 0.42] }
    let metal = pickup_painted_metal()
    return T {
        name = "pickup_truck"
        Destroyable.threshold(3.0)
        CombineMesh.keep_transforms() {
            name = "pickup_painted_body"
            metal
            Texture.with_uri("assets/textures/pool/chrome-matcap.png")
            TextureFiltering.linear()
            ship_solid([0.0, 0.65, 0.0], [2.0, 0.30, 4.8], color)
            ship_solid([0.0, 1.0, -1.65], [2.0, 0.45, 1.5], color)
            ship_solid([0.0, 1.25, -0.45], [1.9, 1.0, 1.45], color)
            ship_solid([0.0, 1.85, -0.45], [2.0, 0.12, 1.7], color)
            ship_vehicle_window([0.0, 1.55, -1.19], [1.75, 0.48, 0.025])
            ship_vehicle_window([0.0, 1.55, 0.29], [1.75, 0.48, 0.025])
            for side in [-1.0, 1.0] {
                ship_vehicle_window([side * 0.96, 1.55, -0.45], [0.025, 0.48, 1.18])
                ship_solid([side * 0.95, 1.10, 1.35], [0.10, 0.65, 2.1], color)
                ship_solid([side * 0.72, 1.12, -2.42], [0.38, 0.20, 0.04], [0.72, 0.67, 0.46])
            }
            ship_solid([0.0, 1.10, 2.35], [2.0, 0.65, 0.10], color)
            ship_solid([0.0, 0.68, -2.48], [2.10, 0.16, 0.12], [0.51, 0.53, 0.48])
        }
        for side in [-1.0, 1.0] {
            ship_lamp([side * 0.85, 1.0, 2.42], [0.15, 0.24, 0.04], [0.60, 0.08, 0.04])
        }
        for x in [-0.96, 0.96] {
            for z in [-1.65, 1.55] { ship_vehicle_wheel(x, z, 0.40, metal) }
        }
        // Bed floor top is 0.8 m; cargo is attached to the vehicle, not the room.
        T.position(0.0, 0.82, 1.35) {
            name = "pickup_bed_cargo_mount"
            if cargo != null { cargo }
        }
    }
}
