import { ship_solid, ship_box, ship_lamp } from "./ship_parts.mms"
import { pickup_painted_metal } from "./materials/pickup_painted_metal.mms"
import { hot_dog_sausage } from "./food/hot_dog.mms"

export fn hot_dog_grill() {
    return T {
        name = "hot_dog_grill"
        ship_solid([0.0, 0.52, 0.0], [1.35, 0.90, 0.72], [0.28, 0.32, 0.29])
        CombineMesh {
            pickup_painted_metal()
            Texture.with_uri("assets/textures/pool/chrome-matcap.png")
            TextureFiltering.linear()
            ship_box([0.0, 0.93, 0.0], [1.42, 0.06, 0.77], [0.32, 0.36, 0.33])
            ship_box([0.0, 0.97, 0.0], [1.20, 0.035, 0.56], [0.045, 0.055, 0.045])
            for bar in range(12) {
                ship_box([-0.55+bar*0.10, 1.0, 0.0], [0.015, 0.018, 0.54], [0.37, 0.39, 0.34])
            }
            for x in [-0.45, 0.0, 0.45] {
                ship_box([x, 0.78, 0.38], [0.10, 0.10, 0.055], [0.13, 0.16, 0.14])
            }
        }
        CombineMesh {
            Material(MaterialDefinition.anime())
            for x in [-0.36, 0.0, 0.36] {
                for z in [-0.15, 0.15] {
                    T.position(x, 0.94, z) { hot_dog_sausage() }
                }
            }
        }
        ship_lamp([0.0, 0.84, 0.376], [0.22, 0.035, 0.012], [0.90, 0.24, 0.035])
    }
}
