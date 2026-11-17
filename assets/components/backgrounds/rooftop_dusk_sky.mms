import { rooftop_dusk_sky_material } from "../materials/rooftop_dusk_sky.mms"

// A single rotation-only sky draw, unaffected by scene lights and bloom.
// Foreground geometry covers it normally.
export fn rooftop_dusk_sky(config) {
    return BG {
        name = "rooftop_dusk_sky"
        Raycastable.disabled()
        Selectable.off()
        // Radius 80 m stays inside the CXR default 100 m far plane.
        T.scale(160.0, 160.0, 160.0) {
            R.sphere() {
                name = "rooftop_dusk_sky_surface"
                rooftop_dusk_sky_material(config)
            }
        }
    }
}
