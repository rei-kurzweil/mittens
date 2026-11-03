import { f32, vec3 } from "types"
import { emissive_surface as ship_emissive } from "../materials/emissive_surface.mms"
import { star_kawaii_background } from "./star_kawaii_background.mms"

// Rotation-only background shared by all ship windows, the park dome and mirrors.
// Nebula detail is baked by generate-ship-nebula-texture, never sampled as noise
// in the fragment shader. The moon has a visible halo even when bloom is off.
export fn ship_space_sky() {
    let definition = MaterialDefinition(MaterialBase.extracted_emissive())
        .fragment("assets/shaders/ship-space-sky.frag")
        .inputs({ moon_direction: vec3, moon_radius: f32,
            nebula_strength: f32, halo_strength: f32, moon_strength: f32 })
    return BG.occlusion_and_lighting() {
        name = "ship_space_sky"
        // Radius 120 m; the ship's cameras use a 1,000 m far plane.
        T.scale(240.0, 240.0, 240.0) {
            R.sphere() {
                name = "ship_nebula_and_moon"
                Material(definition).inputs({
                    moon_direction = [-0.35, 0.09, -1.0]
                    moon_radius = 0.055
                    nebula_strength = 0.72
                    halo_strength = 0.65
                    moon_strength = 2.8
                })
                Texture.with_uri("assets/textures/space/ship-nebula.png")
                TextureFiltering.linear()
            }
        }
        // Sixty percent of the usual star size; retain the familiar gold shape.
        T { name = "ship_small_golden_stars"
            ship_emissive()
            star_kawaii_background([1.0, 0.68, 0.12, 1.0], { scale = 0.60 })
        }
    }
}
