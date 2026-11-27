import { kawaii_twilight_sky_material } from "../materials/kawaii-twilight-sky.mms"

// Plain BG keeps the procedural moon and glow independent of lighting and bloom.
export fn kawaii_twilight_sky_with_material(material, cloud_banks) {
    let sky = BG {
        name = "kawaii_twilight_sky"
        Raycastable.disabled()
        Selectable.off()
        // Radius 80 m stays inside the workshop CXR default 100 m far plane.
        T.scale(160.0, 160.0, 160.0) {
            R.sphere() {
                name = "kawaii_twilight_sky_surface"
                material
                if !cloud_banks {
                    Texture.with_uri("assets/textures/space/sky-cloud-mask.png")
                    TextureFiltering.linear()
                }
            }
        }
    }
    if !cloud_banks { return sky }
    return T {
        sky
        BG.occlusion_and_lighting() { cloud_banks }
    }
}

export fn kawaii_twilight_sky(config) {
    return kawaii_twilight_sky_with_material(kawaii_twilight_sky_material(config))
}
