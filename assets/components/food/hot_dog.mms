import { ship_box } from "../ship_parts.mms"

// X is the long axis. Separate parts allow the grill to batch all its sausages.
export fn hot_dog_sausage() {
    return T {
        T.position(-0.14, 0.075, 0.0).rotation(0.0, 0.0, -Math.pi/2.0) {
            R.cylinder(0.026, 0.28, 12) { C.rgba(0.65, 0.22, 0.085, 1.0) }
        }
        for x in [-0.14, 0.14] {
            T.position(x, 0.075, 0.0).scale(0.052, 0.052, 0.052) {
                R.icosahedron(1, 1.0) { C.rgba(0.65, 0.22, 0.085, 1.0) }
            }
        }
    }
}

export fn hot_dog(on_tray) {
    return CombineMesh {
        name = "hot_dog"
        Material(MaterialDefinition.anime())
        for side in [-1.0, 1.0] {
            T.position(0.0, 0.045, side*0.037).scale(0.35, 0.10, 0.09) {
                R.icosahedron(2, 1.0) { C.rgba(0.85, 0.58, 0.24, 1.0) }
            }
        }
        hot_dog_sausage()
        for segment in range(8) {
            let sign = 1.0
            if segment % 2 == 0 { sign = -1.0 }
            T.position(-0.12+segment*0.034, 0.103, sign*0.006).rotation(0.0, sign*0.5, 0.0) {
                ship_box([0.0, 0.0, 0.0], [0.042, 0.009, 0.012], [0.98, 0.76, 0.06])
            }
        }
        if on_tray {
            ship_box([0.0, -0.012, 0.0], [0.40, 0.012, 0.17], [0.92, 0.88, 0.73])
            for side in [-1.0, 1.0] {
                ship_box([0.0, 0.012, side*0.08], [0.40, 0.045, 0.01], [0.92, 0.88, 0.73])
            }
        }
    }
}
