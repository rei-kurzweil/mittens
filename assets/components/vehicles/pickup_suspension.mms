import { ship_vehicle_wheel_mesh } from "./ship_vehicle_parts.mms"
import { ship_environment_material } from "../materials/ship_anime_materials.mms"

// Chassis coordinates: +X right, +Y up, -Z forward; dimensions in metres.
// The fixed mount remains suitable for a future downward ground probe.
// Scaling spring_<corner>.Y moves its endpoint and compresses the visual;
// the fork removes that scale before wheel steering/spin is applied.
export fn pickup_suspension(corner, x, z, radius, hub_material, layout) {
    let rest_length = 0.50
    let axle_roll = -Math.pi / 2.0
    if x < 0 { axle_roll = Math.pi / 2.0 }
    let wheel = T {
        name = "steering_" + corner
        T {
            name = "wheel_" + corner
            // Keep wheel_<corner>'s spin axis at chassis-local X.
            T.rotation(0,0,axle_roll) {
                ship_vehicle_wheel_mesh(radius,hub_material)
            }
        }
    }
    let endpoint = T.position(0,-rest_length,0) {
        name = "suspension_endpoint_" + corner
        if layout == "siblings" { wheel } else {
            // Resolve the scaled endpoint before removing scale from the wheel.
            TOP.fork_trs().drop("scale") { wheel }
        }
    }
    let spring = T {
        name = "spring_" + corner
        // GLB origin is the upper mount; its mesh extends 0.5 m down local -Y.
        GLTF.new("assets/models/suspension-spring.glb") {
            name = "spring_mesh_" + corner
            ship_environment_material()
        }
        if layout != "siblings" { endpoint }
    }
    let mount = T.position(x, radius + rest_length, z) {
        name = "suspension_mount_" + corner
        spring
        if layout == "siblings" { endpoint }
    }
    mount.set_compression = fn(ratio) {
        let compression = Math.clamp(ratio,0.35,1.35)
        spring.update_transform([0,0,0],[0,0,0],[1,compression,1])
        // Siblings require explicit wheel travel; the fork variant inherits it.
        if layout == "siblings" {
            endpoint.update_transform([0,-rest_length*compression,0],[0,0,0],[1,1,1])
        }
    }
    return mount
}
