// Source-built pool hall. Measurements are metres; entrance is centred on local Z=0.
// Regenerate textures with the Rust examples generate-pool-tiles and generate-metal-matcap.
import { f32 } from "types"
import { pool_tile_material } from "../materials/pool_tiles.mms"
import { matcap_metal } from "../materials/matcap_metal.mms"

fn tile_box(position, size, material) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        R.cube() {
            C.rgba(1.0, 1.0, 1.0, 1.0)
            material
            Texture.with_uri("assets/textures/pool/white-tiles-surface.png")
            TextureFiltering.linear()
        }
        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.8) {} }
    }
}

fn metal_cylinder(position, radius, height, material) {
    return T.position(position[0], position[1], position[2]) {
        R.cylinder(radius, height, 20) {
            material
            Texture.with_uri("assets/textures/pool/chrome-matcap.png")
            TextureFiltering.linear()
        }
        T.position(0.0, height / 2.0, 0.0) {
            Zone.capsule_y(radius, height / 2.0 - radius) { Collidable.static().friction(0.5) {} }
        }
    }
}

export fn indoor_pool(origin, width, length, basin_width, basin_length, water_enabled) {
    let tiles = pool_tile_material()
    let chrome = matcap_metal()
    let water_definition = MaterialDefinition(MaterialBase.planar_reflective()).fragment("assets/shaders/pool-water.frag").inputs({
        time_seconds: f32
        wave_scale: f32
        wave_strength: f32
        reflection_strength: f32
        reflection_distortion: f32
        specular_strength: f32
        shininess: f32
    })
    let water = Material(water_definition).inputs({
        time_seconds = 0.0
        wave_scale = 3.0
        wave_strength = 0.025
        reflection_strength = 0.85
        reflection_distortion = 0.012
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
    // Keep the entrance and pool fixed; extra room length goes behind the basin.
    let end_deck = 1.5
    let back_deck = length - basin_length - end_deck
    let basin_center_z = end_deck + basin_length / 2.0
    let basin_end_z = end_deck + basin_length
    // The latest metre of width is added on +X only. Keep the doorway and
    // access ladder fixed, with the room and basin centres shifted half a metre.
    let center_x = 0.5
    let half_width = width / 2.0
    let hall_height = 8.91
    let cut_bottom = hall_height - 1.575
    let cut_top = hall_height - 0.525
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
            tile_box([center_x + side * (width + basin_width) / 4.0, -0.06, length / 2.0],
                [(width - basin_width) / 2.0, 0.12, length], tiles)
        }
        tile_box([center_x, -0.06, end_deck / 2.0], [basin_width, 0.12, end_deck], tiles)
        tile_box([center_x, -0.06, length - back_deck / 2.0], [basin_width, 0.12, back_deck], tiles)

        // Tiled basin: 1.4 m deep, with independent, correctly lit vertical sides.
        tile_box([center_x, -1.46, basin_center_z], [basin_width, 0.12, basin_length], tiles)
        for side in [-1.0, 1.0] {
            tile_box([center_x + side * (basin_width / 2.0 + 0.06), -0.70, basin_center_z],
                [0.12, 1.4, basin_length + 0.24], tiles)
        }
        tile_box([center_x, -0.70, end_deck - 0.06], [basin_width, 1.4, 0.12], tiles)
        tile_box([center_x, -0.70, basin_end_z + 0.06], [basin_width, 1.4, 0.12], tiles)

        // White ceiling and end walls; the near wall has the same 2.5 m doorway.
        tile_box([center_x, hall_height + 0.08, length / 2.0], [width + 0.32, 0.16, length + 0.32], tiles)
        tile_box([center_x, hall_height / 2.0, length + 0.08], [width + 0.32, hall_height, 0.16], tiles)
        for side in [-1.0, 1.0] {
            let room_edge = center_x + side * half_width
            let doorway_edge = side * 1.25
            tile_box([(room_edge + doorway_edge) / 2.0, hall_height / 2.0, -0.08],
                [side * (room_edge - doorway_edge), hall_height, 0.16], tiles)
        }
        tile_box([0.0, (hall_height + 2.4) / 2.0, -0.08], [2.5, hall_height - 2.4, 0.16], tiles)

        // Each side wall is built around three actual rectangular holes.
        for side in [-1.0, 1.0] {
            tile_box([center_x + side * (half_width + 0.08), cut_bottom / 2.0, length / 2.0],
                [0.16, cut_bottom, length], tiles)
            tile_box([center_x + side * (half_width + 0.08), (hall_height + cut_top) / 2.0, length / 2.0],
                [0.16, hall_height - cut_top, length], tiles)
            for segment in range(4) {
                let a = 0.0
                let b = length
                if segment > 0 { a = length * segment / 4.0 + cut_width / 2.0 }
                if segment < 3 { b = length * (segment + 1) / 4.0 - cut_width / 2.0 }
                tile_box([center_x + side * (half_width + 0.08), (cut_bottom + cut_top) / 2.0, (a + b) / 2.0],
                    [0.16, cut_height, b - a], tiles)
            }
            for slot in range(3) {
                let z = length * (slot + 1) / 4.0
                // Recess the luminous panel beyond the wall thickness; the
                // aperture's jambs remain physical tile surfaces around it.
                T.position(center_x + side * (half_width + 0.22), (cut_bottom + cut_top) / 2.0, z)
                    .scale(0.025, cut_height - 0.10, cut_width - 0.10) {
                    R.cube() {
                        C.rgba(1.0, 1.0, 1.0, 1.0)
                        Shading.toon()
                        EM.on().intensity(1.8)
                    }
                }
                // Throw across the width toward the opposite wall, with only
                // a shallow downward pitch instead of aiming below the basin.
                T.position(center_x + side * (half_width - 0.03), (cut_bottom + cut_top) / 2.0, z)
                    .looking_at([origin[0] + center_x - side * half_width,
                        origin[1] + (cut_bottom + cut_top) / 2.0 - width * 0.25, origin[2] + z]) {
                    SL.color(1.0, 1.0, 1.0).intensity(1.8).distance(light_range).angle(0.85).penumbra(0.35)
                }
            }
        }

        // Flat alpha-blended cyan water. Analytic normal ripples animate the
        // highlights and planar reflection; vertex displacement/refraction come later.
        if water_enabled {
            T.position(center_x, -0.12, basin_center_z).rotation_euler(-Math.pi / 2.0, 0.0, 0.0)
                .scale(basin_width, basin_length, 1.0) {
                R.plane() {
                    name = "pool_water_surface"
                    C.rgba(1.0, 1.0, 1.0, 0.28)
                    water
                    TextureFiltering.linear()
                }
            }
        }

        // Chrome ladder: ordinary cylinders with semicircular handle tops.
        for side in [-1.0, 1.0] {
            T { Climbable {} metal_cylinder([side * 0.28, 0.0, end_deck - 0.24], 0.025, 0.55, chrome) }
            T { Climbable {} metal_cylinder([side * 0.28, -1.20, end_deck + 0.20], 0.025, 1.75, chrome) }
            T.position(side * 0.28, 0.55, end_deck - 0.02).rotation_euler(0.0, 0.0, Math.pi / 2.0) {
                Climbable {}
                R.helix_tube(0.22, 0.025, 0.0, -Math.pi / 2.0, Math.pi, 24, 16) {
                    chrome
                    Texture.with_uri("assets/textures/pool/chrome-matcap.png")
                }
            }
        }
        for step in range(4) {
            T.position(0.28, 0.18 - step * 0.34, end_deck + 0.20).rotation_euler(0.0, 0.0, Math.pi / 2.0) {
                Climbable {}
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
        let slide_z = basin_end_z - 1.25
        let slide_radius = 1.62
        // Extra pitch clears the doubled tube and collars on adjacent turns.
        let slide_height = 4.5696
        let slide_bottom = 0.62
        let slide_sweep = Math.pi * 4.0
        // Clockwise quarter-turn from the side-facing layout: the lower
        // mouth now exits toward -Z, down the pool toward its access ladder.
        // This lateral offset keeps the exit capsule inside the basin while
        // the tube and collars remain clear of the left wall.
        T.position(-0.70, 0.0, slide_z) {
            T.position(0.0, slide_bottom, 0.0) {
                R.helix_tube(slide_radius, 0.60, slide_height, 0.0, slide_sweep, 320, 32) {
                    name = "pool_helix_slide"
                    C.rgba(0.24, 0.67, 0.80, 1.0)
                    outer_plastic
                }
                R.helix_tube(slide_radius, 0.56, slide_height, 0.0, slide_sweep, 320, 32) {
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
            // Hollow collision proxy: short tangent-oriented wall panels,
            // not a solid capsule through the passage. The visual meshes stay smooth.
            let collision_sections = 64
            let collision_sides = 12
            let collision_wall_radius = 0.58
            let collision_panel_width = 2.0 * collision_wall_radius * Math.tan(Math.pi / collision_sides)
            for section in range(collision_sections) {
                let t = (section + 0.5) / collision_sections
                let angle = slide_sweep * t
                T.position(slide_radius * Math.cos(angle), slide_bottom + slide_height * t,
                    slide_radius * Math.sin(angle)).rotation_euler(0.0, -angle, 0.0) {
                    T.rotation_euler(ring_tilt, 0.0, 0.0) {
                        for side in range(collision_sides) {
                            let a = Math.pi * 2.0 * side / collision_sides
                            T.position(collision_wall_radius * Math.cos(a), 0.0, collision_wall_radius * Math.sin(a))
                                .rotation_euler(0.0, -a, 0.0)
                                .scale(0.04, arc_length / collision_sections + 0.04, collision_panel_width + 0.008) {
                                Zone.cube([0.5, 0.5, 0.5]) {
                                    name = "slide_wall_collision"
                                    Collidable.static().friction(0.12) {}
                                }
                            }
                        }
                    }
                }
            }
            // Close the 4 cm shell thickness at the two open mouths, keeping the
            // central passage open and the exterior/interior surfaces distinct.
            for t in [0.0, 1.0] {
                let angle = slide_sweep * t
                T.position(slide_radius * Math.cos(angle), slide_bottom + slide_height * t,
                    slide_radius * Math.sin(angle)).rotation_euler(0.0, -angle, 0.0) {
                    T.rotation_euler(ring_tilt, 0.0, 0.0).scale(1.0, 0.5, 1.0) {
                        R.torus(0.58, 0.028284272, 64, 4) {
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
                T.position(slide_radius * Math.cos(angle), slide_bottom + slide_height * t,
                    slide_radius * Math.sin(angle)).rotation_euler(0.0, -angle, 0.0) {
                    // Torus axis +Y follows the local helix tangent. Four flat
                    // profile sides, 64 circumference segments, compressed thickness.
                    T.rotation_euler(ring_tilt, 0.0, 0.0).scale(1.0, 0.45, 1.0) {
                        R.torus(0.68, 0.11, 64, 4) {
                            name = "slide_reinforcement_ring"
                            C.rgba(0.12, 0.36, 0.45, 1.0)
                            outer_plastic
                        }
                    }
                }
            }
            metal_cylinder([0.0, -1.4, 0.0], 0.055, slide_bottom + slide_height + 1.4, chrome)

            // Square landing meets the lower lip of the upper tube opening.
            let landing_top = slide_bottom + slide_height - 0.60 * Math.sin(ring_tilt)
            let landing_z = 1.10
            T.position(slide_radius, landing_top - 0.06, landing_z).scale(2.2, 0.12, 2.2) {
                R.cube() {
                    name = "slide_landing"
                    C.rgba(0.24, 0.67, 0.80, 1.0)
                    outer_plastic
                }
                Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.8) {} }
            }
            for side in [-1.0, 1.0] {
                // Metal ladder rises from the extended rear deck to the landing.
                T { Climbable {} metal_cylinder([slide_radius + side * 0.28, 0.0, 2.28],
                    0.025, landing_top + 0.65, chrome) }
                // The near support stays in the helix's central opening;
                // the far support stands on the deck outside the coil.
                let support_z = 2.10
                let support_base = 0.0
                if side < 0.0 {
                    support_z = 0.25
                    support_base = -1.4
                }
                metal_cylinder([slide_radius + side * 0.88, support_base, support_z],
                    0.035, landing_top - support_base, chrome)
                // Landing side guardrails leave the tube mouth and ladder open.
                for z in [0.35, 2.10] {
                    metal_cylinder([slide_radius + side * 1.03, landing_top, z], 0.022, 0.65, chrome)
                }
                T.position(slide_radius + side * 1.03, landing_top + 0.65, 0.35)
                    .rotation_euler(Math.pi / 2.0, 0.0, 0.0) {
                    R.cylinder(0.022, 1.75, 16) {
                        chrome
                        Texture.with_uri("assets/textures/pool/chrome-matcap.png")
                    }
                }
            }
            let rung_count = Math.floor(landing_top / 0.25)
            for rung in range(rung_count) {
                T.position(slide_radius + 0.28, 0.20 + rung * 0.25, 2.28)
                    .rotation_euler(0.0, 0.0, Math.pi / 2.0) {
                    Climbable {}
                    R.cylinder(0.022, 0.56, 16) {
                        name = "slide_ladder_rung"
                        chrome
                        Texture.with_uri("assets/textures/pool/chrome-matcap.png")
                    }
                }
            }
        }
    }
}
