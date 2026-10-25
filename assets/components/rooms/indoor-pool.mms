// Source-built pool hall. Measurements are metres; entrance is centred on local Z=0.
// Regenerate textures with the Rust examples generate-pool-tiles and generate-metal-matcap.
import { f32 } from "types"

fn tile_box(position, size, material) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        R.cube() {
            C.rgba(1.0, 1.0, 1.0, 1.0)
            material
            Texture.with_uri("assets/textures/pool/white-tiles-surface.png")
            TextureFiltering.linear()
        }
    }
}

fn metal_cylinder(position, radius, height, material) {
    return T.position(position[0], position[1], position[2]) {
        R.cylinder(radius, height, 20) {
            material
            Texture.with_uri("assets/textures/pool/chrome-matcap.png")
            TextureFiltering.linear()
        }
    }
}

export fn indoor_pool(origin, width, length, basin_width, basin_length, water_enabled) {
    let tile_definition = MaterialDefinition(MaterialBase()).fragment("assets/shaders/pool-tiles.frag").inputs({
        tile_size: f32
        normal_strength: f32
        specular_strength: f32
        shininess: f32
        rim_strength: f32
        shade_threshold: f32
        lit_threshold: f32
    })
    let tiles = Material(tile_definition).inputs({
        tile_size = 0.25
        normal_strength = 1.0
        specular_strength = 0.65
        shininess = 64.0
        rim_strength = 0.2
        shade_threshold = 0.05
        lit_threshold = 2.0
    })
    let metal_definition = MaterialDefinition(MaterialBase()).fragment("assets/shaders/pool-metal-matcap.frag").inputs({
        brightness: f32
    })
    let chrome = Material(metal_definition).inputs({ brightness = 1.0 })
    let water_definition = MaterialDefinition(MaterialBase()).fragment("assets/shaders/pool-water.frag").inputs({
        time_seconds: f32
        wave_scale: f32
        wave_strength: f32
        specular_strength: f32
        shininess: f32
    })
    let water = Material(water_definition).inputs({
        time_seconds = 0.0
        wave_scale = 3.0
        wave_strength = 0.025
        specular_strength = 1.0
        shininess = 120.0
    })
    if water_enabled {
        let state = { time = 0.0 }
        on_global("FrameTick", fn(event) {
            state.time = (state.time + event.dt_sec) % (Math.pi * 20.0)
            water.set_input("time_seconds", state.time)
        })
    }
    let end_deck = (length - basin_length) / 2.0
    let half_width = width / 2.0
    let hall_height = 4.5
    let cut_bottom = 3.45
    let cut_top = 4.15
    let cut_height = cut_top - cut_bottom
    let cut_width = length * 0.14
    if cut_width > 1.4 { cut_width = 1.4 }
    let light_range = Math.sqrt(width * width + hall_height * hall_height) * 1.8

    return T.position(origin[0], origin[1], origin[2]) {
        name = "indoor_pool"
        Shading.anime().shade_color([0.48, 0.48, 0.50]).shade_strength(0.75)
            .rim_color([1.0, 1.0, 1.0]).rim_strength(0.18).rim_power(4.0)

        // Four deck strips leave a real opening over the basin.
        for side in [-1.0, 1.0] {
            tile_box([side * (width + basin_width) / 4.0, -0.06, length / 2.0],
                [(width - basin_width) / 2.0, 0.12, length], tiles)
        }
        tile_box([0.0, -0.06, end_deck / 2.0], [basin_width, 0.12, end_deck], tiles)
        tile_box([0.0, -0.06, length - end_deck / 2.0], [basin_width, 0.12, end_deck], tiles)

        // Tiled basin: 1.4 m deep, with independent, correctly lit vertical sides.
        tile_box([0.0, -1.46, length / 2.0], [basin_width, 0.12, basin_length], tiles)
        for side in [-1.0, 1.0] {
            tile_box([side * (basin_width / 2.0 + 0.06), -0.70, length / 2.0],
                [0.12, 1.4, basin_length + 0.24], tiles)
        }
        tile_box([0.0, -0.70, end_deck - 0.06], [basin_width, 1.4, 0.12], tiles)
        tile_box([0.0, -0.70, length - end_deck + 0.06], [basin_width, 1.4, 0.12], tiles)

        // White ceiling and end walls; the near wall has the same 2.5 m doorway.
        tile_box([0.0, hall_height + 0.08, length / 2.0], [width + 0.32, 0.16, length + 0.32], tiles)
        tile_box([0.0, hall_height / 2.0, length + 0.08], [width + 0.32, hall_height, 0.16], tiles)
        for side in [-1.0, 1.0] {
            tile_box([side * (width + 2.5) / 4.0, hall_height / 2.0, -0.08],
                [(width - 2.5) / 2.0, hall_height, 0.16], tiles)
        }
        tile_box([0.0, (hall_height + 2.4) / 2.0, -0.08], [2.5, hall_height - 2.4, 0.16], tiles)

        // Each side wall is built around three actual rectangular holes.
        for side in [-1.0, 1.0] {
            tile_box([side * (half_width + 0.08), cut_bottom / 2.0, length / 2.0],
                [0.16, cut_bottom, length], tiles)
            tile_box([side * (half_width + 0.08), (hall_height + cut_top) / 2.0, length / 2.0],
                [0.16, hall_height - cut_top, length], tiles)
            for segment in range(4) {
                let a = 0.0
                let b = length
                if segment > 0 { a = length * segment / 4.0 + cut_width / 2.0 }
                if segment < 3 { b = length * (segment + 1) / 4.0 - cut_width / 2.0 }
                tile_box([side * (half_width + 0.08), (cut_bottom + cut_top) / 2.0, (a + b) / 2.0],
                    [0.16, cut_height, b - a], tiles)
            }
            for slot in range(3) {
                let z = length * (slot + 1) / 4.0
                // Recess the luminous panel beyond the wall thickness; the
                // aperture's jambs remain physical tile surfaces around it.
                T.position(side * (half_width + 0.22), (cut_bottom + cut_top) / 2.0, z)
                    .scale(0.025, cut_height - 0.10, cut_width - 0.10) {
                    R.cube() {
                        C.rgba(1.0, 1.0, 1.0, 1.0)
                        Shading.toon()
                        EM.on().intensity(1.8)
                    }
                }
                // Throw across the width toward the opposite wall, with only
                // a shallow downward pitch instead of aiming below the basin.
                T.position(side * (half_width - 0.03), (cut_bottom + cut_top) / 2.0, z)
                    .looking_at([origin[0] - side * half_width,
                        origin[1] + (cut_bottom + cut_top) / 2.0 - 1.0, origin[2] + z]) {
                    SL.color(1.0, 1.0, 1.0).intensity(1.8).distance(light_range).angle(0.85).penumbra(0.35)
                }
            }
        }

        // Flat alpha-blended cyan water. Analytic normal ripples animate the
        // highlights; vertex displacement, scene reflection/refraction come later.
        if water_enabled {
            T.position(0.0, -0.12, length / 2.0).rotation_euler(-Math.pi / 2.0, 0.0, 0.0)
                .scale(basin_width, basin_length, 1.0) {
                R.plane() {
                    name = "pool_water_surface"
                    C.rgba(1.0, 1.0, 1.0, 0.28)
                    water
                }
            }
        }

        // Chrome ladder: ordinary cylinders with semicircular handle tops.
        for side in [-1.0, 1.0] {
            metal_cylinder([side * 0.28, 0.0, end_deck - 0.24], 0.025, 0.55, chrome)
            metal_cylinder([side * 0.28, -1.20, end_deck + 0.20], 0.025, 1.75, chrome)
            T.position(side * 0.28, 0.55, end_deck - 0.02).rotation_euler(0.0, 0.0, Math.pi / 2.0) {
                R.helix_tube(0.22, 0.025, 0.0, -Math.pi / 2.0, Math.pi, 24, 16) {
                    chrome
                    Texture.with_uri("assets/textures/pool/chrome-matcap.png")
                }
            }
        }
        for step in range(4) {
            T.position(0.28, 0.18 - step * 0.34, end_deck + 0.20).rotation_euler(0.0, 0.0, Math.pi / 2.0) {
                R.cylinder(0.022, 0.56, 16) {
                    chrome
                    Texture.with_uri("assets/textures/pool/chrome-matcap.png")
                }
            }
        }

        // One definition, separate exterior/interior plastic instances. The
        // liner's two lighting scales approximate occlusion by the outer shell.
        let plastic_definition = MaterialDefinition(MaterialBase()).fragment("assets/shaders/plastic.frag").inputs({
            diffuse_scale: f32
            specular_scale: f32
            ambient_strength: f32
            shininess: f32
            rim_strength: f32
            normal_sign: f32
        })
        let outer_plastic = Material(plastic_definition).inputs({
            diffuse_scale = 1.0
            specular_scale = 1.0
            ambient_strength = 0.16
            shininess = 64.0
            rim_strength = 0.30
            normal_sign = 1.0
        })
        let inner_plastic = Material(plastic_definition).inputs({
            diffuse_scale = 1.0 / 3.0
            specular_scale = 1.0 / 3.0
            ambient_strength = 0.16
            shininess = 64.0
            rim_strength = 0.30
            normal_sign = -1.0
        })
        let slide_z = length - end_deck - 0.75
        let slide_radius = 0.72
        let slide_height = 2.6
        let slide_sweep = Math.pi * 4.0
        T.position(0.0, 0.04, slide_z) {
            R.helix_tube(slide_radius, 0.30, slide_height, 0.0, slide_sweep, 320, 32) {
                name = "pool_helix_slide"
                C.rgba(0.24, 0.67, 0.80, 1.0)
                outer_plastic
            }
            R.helix_tube(slide_radius, 0.28, slide_height, 0.0, slide_sweep, 320, 32) {
                name = "pool_helix_slide_inner"
                C.rgba(0.24, 0.67, 0.80, 1.0)
                inner_plastic
            }
        }

        // Constant-speed helix: space collars by arc length, not angular distance.
        let circumference = slide_radius * slide_sweep
        let arc_length = Math.sqrt(circumference * circumference + slide_height * slide_height)
        let ring_count = Math.floor(arc_length / 1.5)
        let ring_tilt = Math.atan2(circumference, slide_height)
        // Close the 2 cm shell thickness at the two open mouths, keeping the
        // central passage open and the exterior/interior surfaces distinct.
        for t in [0.0, 1.0] {
            let angle = slide_sweep * t
            T.position(slide_radius * Math.cos(angle), 0.04 + slide_height * t,
                slide_z + slide_radius * Math.sin(angle)).rotation_euler(0.0, -angle, 0.0) {
                T.rotation_euler(ring_tilt, 0.0, 0.0).scale(1.0, 0.5, 1.0) {
                    R.torus(0.29, 0.014142136, 64, 4) {
                        name = "slide_mouth_lip"
                        C.rgba(0.24, 0.67, 0.80, 1.0)
                        outer_plastic
                    }
                }
            }
        }
        for ring in range(ring_count) {
            let t = (ring + 1) * 1.5 / arc_length
            let angle = slide_sweep * t
            T.position(slide_radius * Math.cos(angle), 0.04 + slide_height * t,
                slide_z + slide_radius * Math.sin(angle)).rotation_euler(0.0, -angle, 0.0) {
                // Torus axis +Y follows the local helix tangent. Four flat
                // profile sides, 64 circumference segments, compressed thickness.
                T.rotation_euler(ring_tilt, 0.0, 0.0).scale(1.0, 0.45, 1.0) {
                    R.torus(0.34, 0.055, 64, 4) {
                        name = "slide_reinforcement_ring"
                        C.rgba(0.12, 0.36, 0.45, 1.0)
                        outer_plastic
                    }
                }
            }
        }
        metal_cylinder([0.0, -1.4, slide_z], 0.055, 4.0, chrome)

        // Square landing meets the lower lip of the upper tube opening.
        let landing_top = 0.04 + slide_height - 0.30 * Math.sin(ring_tilt)
        let landing_z = slide_z + 0.60
        T.position(slide_radius, landing_top - 0.06, landing_z).scale(1.2, 0.12, 1.2) {
            R.cube() {
                name = "slide_landing"
                C.rgba(0.24, 0.67, 0.80, 1.0)
                outer_plastic
            }
        }
        for side in [-1.0, 1.0] {
            // Metal ladder rises from the far-end deck to the landing.
            metal_cylinder([slide_radius + side * 0.28, 0.0, slide_z + 1.28],
                0.025, landing_top + 0.65, chrome)
            metal_cylinder([slide_radius + side * 0.48, 0.0, slide_z + 1.10],
                0.035, landing_top, chrome)
            // Landing side guardrails leave the tube mouth and ladder open.
            for z in [slide_z + 0.25, slide_z + 1.10] {
                metal_cylinder([slide_radius + side * 0.55, landing_top, z], 0.022, 0.65, chrome)
            }
            T.position(slide_radius + side * 0.55, landing_top + 0.65, slide_z + 0.25)
                .rotation_euler(Math.pi / 2.0, 0.0, 0.0) {
                R.cylinder(0.022, 0.85, 16) {
                    chrome
                    Texture.with_uri("assets/textures/pool/chrome-matcap.png")
                }
            }
        }
        let rung_count = Math.floor(landing_top / 0.25)
        for rung in range(rung_count) {
            T.position(slide_radius + 0.28, 0.20 + rung * 0.25, slide_z + 1.28)
                .rotation_euler(0.0, 0.0, Math.pi / 2.0) {
                R.cylinder(0.022, 0.56, 16) {
                    name = "slide_ladder_rung"
                    chrome
                    Texture.with_uri("assets/textures/pool/chrome-matcap.png")
                }
            }
        }
    }
}
