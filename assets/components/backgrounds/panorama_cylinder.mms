import { f32 } from "types"

let definition = MaterialDefinition(MaterialBase())
    .fragment("assets/shaders/panorama-cylinder.frag")
    .inputs({ brightness: f32, coverage: f32 })

// An inward-facing, uncapped cylinder textured with a panorama photo. Plain
// BG: rotation-only, never occludes or receives lighting, so the viewer can
// never walk up to it. The ring is the open helix_tube primitive (caps would
// smear the image), and the background pipeline draws both faces.
//
// config (false for defaults):
//   aspect     image width / height (default 4.0)
//   coverage   fraction of 360 degrees the image spans (default 1.0)
//   yaw        radians; rotates the image's left edge around the viewer
//   brightness linear multiplier, below 1 to darken a daytime photo
export fn panorama_cylinder(uri, config) {
    let options = { aspect = 4.0 coverage = 1.0 yaw = 0.0 brightness = 1.0 }
    if config {
        for field in ["aspect", "coverage", "yaw", "brightness"] {
            if config[field] != null { options[field] = config[field] }
        }
    }
    // Height / radius keeps the photo's proportions; the radius keeps the top
    // and bottom rims 80 m away, inside the CXR default 100 m far plane.
    let half_height_ratio = Math.pi * options.coverage / options.aspect
    let radius = 80.0 / Math.sqrt(1.0 + half_height_ratio * half_height_ratio)
    let height = 2.0 * half_height_ratio * radius
    let material = Material(definition).inputs({
        brightness = options.brightness
        coverage = options.coverage
    })
    return BG {
        name = "panorama_cylinder"
        Raycastable.disabled()
        Selectable.off()
        // Centered on the eye so the photo's horizon sits at eye level.
        T.position(0.0, -height / 2.0, 0.0).rotation(0.0, options.yaw, 0.0) {
            R.helix_tube(0.0, radius, height, 0.0, 0.0, 1, 64) {
                name = "panorama_cylinder_surface"
                material
                Texture.with_uri(uri)
                TextureFiltering.linear()
            }
        }
    }
}
