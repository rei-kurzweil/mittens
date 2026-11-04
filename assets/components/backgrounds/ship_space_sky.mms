import { f32, vec3 } from "types"
import { emissive_surface as ship_emissive } from "../materials/emissive_surface.mms"
import { star_kawaii_background } from "./star_kawaii_background.mms"

// Rotation-only background shared by all ship windows, the park dome and mirrors.
// Monochrome clouds are baked offline and tinted over the Sakura two-stop
// gradient. The moon has a visible halo even when bloom is off.
export fn ship_space_sky(low_color, high_color, low_point, high_point, cloud_color, cloud_strength) {
    if low_color == null {low_color=[1.0,0.72,0.66]}
    if high_color == null {high_color=[0.23,0.32,0.62]}
    if low_point == null {low_point=-0.12}
    if high_point == null {high_point=0.82}
    if cloud_color == null {cloud_color=[1.0,0.95,0.90]}
    if cloud_strength == null {cloud_strength=0.65}
    let definition = MaterialDefinition(MaterialBase.extracted_emissive())
        .fragment("assets/shaders/ship-space-sky.frag")
        .inputs({ low_color: vec3, low_point: f32,
            high_color: vec3, high_point: f32,
            cloud_color: vec3, cloud_strength: f32,
            moon_direction: vec3, moon_radius: f32,
            halo_strength: f32, moon_strength: f32 })
    return BG.occlusion_and_lighting() {
        name = "ship_space_sky"
        // Radius 120 m; the ship's cameras use a 1,000 m far plane.
        T.scale(240.0, 240.0, 240.0) {
            R.sphere() {
                name = "ship_nebula_and_moon"
                Material(definition).inputs({
                    low_color=low_color low_point=low_point
                    high_color=high_color high_point=high_point
                    cloud_color=cloud_color cloud_strength=cloud_strength
                    moon_direction = [-0.35, 0.09, -1.0]
                    moon_radius = 0.055
                    halo_strength = 0.65
                    moon_strength = 2.8
                })
                Texture.with_uri("assets/textures/space/sky-cloud-mask.png")
                TextureFiltering.linear()
            }
        }
        // Half the previous radius (30% of the shared default); same pale gold shapes.
        T { name = "ship_small_golden_stars"
            ship_emissive()
            star_kawaii_background([1.0, 0.94, 0.82, 1.0], { scale = 0.30 })
        }
    }
}
