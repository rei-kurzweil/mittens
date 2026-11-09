import { kawaii_twilight_sky_material } from "../materials/kawaii-twilight-sky.mms"

// A single rotation-only sky draw. Plain BG keeps its baked moon/glow independent
// of scene lights and bloom. Foreground geometry covers it normally.
export fn kawaii_twilight_sky(config) {
    return BG {
        name = "kawaii_twilight_sky"
        Raycastable.disabled()
        Selectable.off()
        // Radius 80 m stays inside the workshop CXR default 100 m far plane.
        T.scale(160.0, 160.0, 160.0) {
            R.sphere() {
                name = "kawaii_twilight_sky_surface"
                kawaii_twilight_sky_material(config)
                Texture.with_uri("assets/textures/sky/kawaii-twilight-masks.png")
                TextureFiltering.linear()
            }
        }
    }
}
