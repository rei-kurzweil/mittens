import { f32, vec3 } from "types"

// Cartoon grass ground (assets/shaders/cartoon-grass.frag): the albedo times
// a soft lightness pattern, with little outlined tufts scattered
// over it that always stand upright toward the viewer. Lit with the Anime
// ramp and the same shade settings as ship_environment_material: never
// brighter than the albedo, shaded toward shade_color where little light
// reaches, with a soft ramp across each light's terminator.
//
// The albedo is the material's `albedo` times the instance colour (C.rgba).
// Each grass renderable takes the material, a colour and the tuft atlas:
//     R.plane() {
//         cartoon_grass("far", { albedo = [0.80, 0.96, 0.62] })
//         C.rgba(1.0, 1.0, 1.0, 1.0)
//         Texture.with_uri("assets/textures/grass/cartoon-grass-tufts.png")
//         TextureFiltering.linear()
//     }
// Regenerate the atlas with scripts/generate_grass_tufts.py. Tufts are placed
// by world position, so the renderable's UVs don't matter.

let shared = { near = null middle = null far = null }
fn definition(bucket) {
    if shared[bucket] == null {
        shared[bucket] = MaterialDefinition(MaterialBase()).opaque_bucket(bucket)
            .fragment("assets/shaders/cartoon-grass.frag")
            .inputs({
                shade_color: vec3, shade_strength: f32,
                albedo: vec3, facing_softness: f32,
                shade_threshold: f32, lit_threshold: f32, cell_size: f32, density: f32,
                tuft_min: f32, tuft_max: f32, fade_near: f32, fade_far: f32,
                patch_strength: f32, patch_scale: f32, ground_lightness: f32, line_strength: f32
            })
    }
    return shared[bucket]
}
// The same inputs plus the dome's ellipsoid (assets/shaders/cartoon-grass-dome.frag).
let shared_dome = { near = null middle = null far = null }
fn dome_definition(bucket) {
    if shared_dome[bucket] == null {
        shared_dome[bucket] = MaterialDefinition(MaterialBase()).opaque_bucket(bucket)
            .fragment("assets/shaders/cartoon-grass-dome.frag")
            .inputs({
                shade_color: vec3, shade_strength: f32,
                albedo: vec3, facing_softness: f32,
                shade_threshold: f32, lit_threshold: f32, cell_size: f32, density: f32,
                tuft_min: f32, tuft_max: f32, fade_near: f32, fade_far: f32,
                patch_strength: f32, patch_scale: f32, ground_lightness: f32, line_strength: f32,
                dome_center: vec3, dome_pad0: f32, dome_radii: vec3, dome_pad1: f32
            })
    }
    return shared_dome[bucket]
}

// options (all optional): albedo ([r, g, b], default white: the colour then
// comes from C.rgba), facing_softness (half width of the ramp across
// N.L = 0; 0 switches), shade_color, shade_strength, shade_threshold,
// lit_threshold, cell_size (m; at most one tuft per cell), density
// (0..1, share of cells with a tuft), tuft_min and tuft_max (tuft width, m),
// fade_near and fade_far (m from the eye), patch_strength, patch_scale (m),
// ground_lightness, line_strength.
fn grass_inputs(options) {
    let inputs = {
        shade_color = [0.15, 0.17, 0.30], shade_strength = 0.82,
        albedo = [1.0, 1.0, 1.0], facing_softness = 0.15,
        shade_threshold = 0.12, lit_threshold = 1.60, cell_size = 0.42, density = 0.7,
        tuft_min = 0.12, tuft_max = 0.22, fade_near = 8.0, fade_far = 14.0,
        patch_strength = 0.06, patch_scale = 3.0, ground_lightness = 0.8, line_strength = 1.0
    }
    if options {
        for field in ["albedo", "facing_softness", "shade_color", "shade_strength",
            "shade_threshold", "lit_threshold", "cell_size", "density", "tuft_min", "tuft_max", "fade_near", "fade_far",
            "patch_strength", "patch_scale", "ground_lightness", "line_strength"] {
            if options[field] != null { inputs[field] = options[field] }
        }
    }
    return inputs
}

export fn cartoon_grass(bucket, options) {
    if bucket == null { bucket = "far" }
    return Material(definition(bucket)).inputs(grass_inputs(options)) { name = "cartoon_grass_material" }
}

// Cartoon grass for a sphere mesh scaled into an ellipsoid dome (a hill): the
// normal comes from the ellipsoid, since the shared vertex shader's is wrong
// under non-uniform scale. center: the ellipsoid's world centre; radii: its
// [x, y, z] semi-axes (half the sphere mesh's scale). One material per dome.
export fn cartoon_grass_dome(bucket, center, radii, options) {
    if bucket == null { bucket = "far" }
    let inputs = grass_inputs(options)
    inputs.dome_center = center
    inputs.dome_pad0 = 0.0
    inputs.dome_radii = radii
    inputs.dome_pad1 = 0.0
    return Material(dome_definition(bucket)).inputs(inputs) { name = "cartoon_grass_dome_material" }
}
