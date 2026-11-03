import { ship_solid } from "./ship_parts.mms"
import { pickup_painted_metal } from "./materials/pickup_painted_metal.mms"

// Door faces +Z; origin sits on a floor or the pickup's cargo mount.
export fn mini_fridge() {
    return T {
        name = "mini_fridge"
        Destroyable.threshold(3.0)
        CombineMesh.keep_transforms() {
            pickup_painted_metal()
            Texture.with_uri("assets/textures/pool/chrome-matcap.png")
            TextureFiltering.linear()
            ship_solid([0.0, 0.46, 0.0], [0.66, 0.92, 0.60], [0.73, 0.72, 0.62])
            ship_solid([0.0, 0.47, 0.31], [0.62, 0.86, 0.045], [0.86, 0.84, 0.74])
            ship_solid([0.23, 0.63, 0.35], [0.04, 0.25, 0.035], [0.30, 0.34, 0.32])
            for vent in range(5) {
                ship_solid([-0.18+vent*0.09, 0.08, 0.342], [0.055, 0.028, 0.015], [0.13, 0.17, 0.16])
            }
        }
    }
}
