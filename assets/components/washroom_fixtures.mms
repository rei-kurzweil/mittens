import { ship_box, ship_lamp } from "./ship_parts.mms"

// Wall fixtures face +Z; callers rotate them to the room's wall.
export fn hand_dryer() {
    return T {
        name = "hand_dryer"
        CombineMesh {
            Material(MaterialDefinition.anime())
            ship_box([0.0, 0.0, 0.0], [0.38, 0.48, 0.20], [0.77, 0.80, 0.73])
            ship_box([0.0, -0.11, 0.105], [0.29, 0.15, 0.015], [0.15, 0.22, 0.21])
            for row in range(4) {
                ship_box([0.0, -0.16 + row*0.035, 0.118], [0.27, 0.012, 0.014], [0.46, 0.53, 0.49])
            }
            ship_box([0.0, -0.245, 0.03], [0.23, 0.012, 0.10], [0.07, 0.12, 0.12])
        }
        ship_lamp([0.0, 0.11, 0.108], [0.03, 0.015, 0.01], [0.20, 0.66, 0.60])
    }
}

export fn paper_towel_dispenser() {
    return T {
        name = "paper_towel_dispenser"
        CombineMesh {
            Material(MaterialDefinition.anime())
            ship_box([0.0, 0.0, 0.0], [0.42, 0.46, 0.18], [0.52, 0.59, 0.55])
            ship_box([0.0, -0.19, 0.095], [0.32, 0.035, 0.012], [0.10, 0.15, 0.14])
            ship_box([0.0, -0.30, 0.08], [0.28, 0.20, 0.008], [0.95, 0.94, 0.86])
        }
    }
}

// Open top basin and simple squared faucet; origin is counter height.
export fn washroom_sink() {
    return CombineMesh {
        name = "washroom_sink"
        Material(MaterialDefinition.anime())
        T.scale(1.0, 1.0, 1.35) {
            R.helix(0.13, 0.22, 0.0, 0.0, Math.pi*2.0, 24, 0.06) { C.rgba(0.88, 0.90, 0.83, 1.0) }
            T.position(0.0, 0.015, 0.0).rotation(-Math.pi/2.0, 0.0, 0.0).scale(0.26, 0.26, 1.0) {
                R.circle_2d() { C.rgba(0.23, 0.32, 0.31, 1.0) }
            }
        }
        ship_box([0.0, 0.15, -0.24], [0.05, 0.30, 0.05], [0.62, 0.69, 0.65])
        ship_box([0.0, 0.29, -0.15], [0.05, 0.05, 0.22], [0.62, 0.69, 0.65])
    }
}
