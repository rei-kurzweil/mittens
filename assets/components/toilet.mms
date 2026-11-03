import { ship_box } from "./ship_parts.mms"

// Forward is -Z, tank behind the bowl on +Z. All dimensions are metres.
export fn toilet() {
    return T {
        name = "toilet"
        CombineMesh {
            Material(MaterialDefinition.anime())
            // Pedestal supports the bowl; the ring has actual inner/outer walls.
            T.position(0.0, 0.0, 0.02) {
                R.cylinder(0.13, 0.34, 20) { C.rgba(0.87, 0.89, 0.82, 1.0) }
            }
            T.position(0.0, 0.32, -0.05).scale(1.0, 1.0, 1.3) {
                R.helix(0.10, 0.23, 0.0, 0.0, Math.pi*2.0, 24, 0.15) { C.rgba(0.87, 0.89, 0.82, 1.0) }
                T.position(0.0, 0.035, 0.0).rotation(-Math.pi/2.0, 0.0, 0.0).scale(0.20, 0.20, 1.0) {
                    R.circle_2d() { C.rgba(0.24, 0.37, 0.39, 1.0) }
                }
            }
            // A thick annulus makes an open seat, rather than a solid disc.
            T.position(0.0, 0.49, -0.05).scale(1.0, 1.0, 1.3) {
                R.helix_tube(0.19, 0.032, 0.0, 0.0, Math.pi*2.0, 24, 8) { C.rgba(0.96, 0.95, 0.89, 1.0) }
            }
            ship_box([0.0, 0.73, 0.32], [0.48, 0.48, 0.18], [0.87, 0.89, 0.82])
            ship_box([0.0, 0.98, 0.32], [0.50, 0.035, 0.20], [0.96, 0.95, 0.89])
            ship_box([0.12, 1.005, 0.32], [0.07, 0.02, 0.055], [0.55, 0.62, 0.61])
            ship_box([-0.255, 0.84, 0.30], [0.035, 0.04, 0.11], [0.55, 0.62, 0.61])
        }
        // Keep contact topology outside the disposable visual batch.
        T.position(0.0, 0.25, -0.02) {
            Zone.cube([0.23, 0.25, 0.31]) { Collidable.static().friction(0.8) {} }
        }
        T.position(0.0, 0.73, 0.32) {
            Zone.cube([0.24, 0.24, 0.09]) { Collidable.static().friction(0.8) {} }
        }
    }
}
