import { ship_box } from "../ship_parts.mms"

// X is the long axis. Raw geometry is batched within each movable food part.
fn sausage_mesh() {
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

// Grip origins sit at the food centres. Like the gun, XR uses the solved hand
// attachment and a small palm offset, rather than the ray's grab clearance.
export fn hot_dog_sausage(with_mustard) {
    return T.position(0.0,0.075,0.0) {
        name="hot_dog_sausage_grip"
        Selectable.off()
        Grabbable.to_hand(true).hand_offset([0.0,0.0,-0.035]) {}
        CombineMesh {
            Material(MaterialDefinition.anime())
            T.position(0.0,-0.075,0.0) {
                sausage_mesh()
                if with_mustard {
                    for segment in range(8) {
                        let sign=1.0
                        if segment%2==0 {sign=-1.0}
                        T.position(-0.12+segment*0.034,0.103,sign*0.006).rotation(0.0,sign*0.5,0.0) {
                            ship_box([0,0,0],[0.042,0.009,0.012],[0.98,0.76,0.06])
                        }
                    }
                }
            }
        }
    }
}

export fn hot_dog_bun() {
    return T.position(0.0,0.045,0.0) {
        name="hot_dog_bun_grip"
        Selectable.off()
        Grabbable.to_hand(true).hand_offset([0.0,0.0,-0.035]) {}
        CombineMesh {
            Material(MaterialDefinition.anime())
            // Leave the sausage exposed above the bun for independent ray hits.
            for side in [-1.0,1.0] {
                T.position(0.0,0.0,side*0.037).scale(0.18,0.040,0.040) {
                    R.icosahedron(2,1.0) {C.rgba(0.85,0.58,0.24,1.0)}
                }
            }
        }
    }
}

export fn hot_dog(on_tray) {
    return T {
        name="hot_dog"
        // Siblings: grabbing either food part cannot drag the other or the tray.
        hot_dog_bun()
        hot_dog_sausage(true)
        if on_tray {
            CombineMesh {
                Material(MaterialDefinition.anime())
                ship_box([0.0,-0.012,0.0],[0.40,0.012,0.17],[0.92,0.88,0.73])
                for side in [-1.0,1.0] {
                    ship_box([0.0,0.012,side*0.08],[0.40,0.045,0.01],[0.92,0.88,0.73])
                }
            }
        }
    }
}
