import { radio_tower } from "./radio_tower.mms"

// The eastbound descent, landing, and quarter turn share one road layout.
fn ramp_segment(x0, y0, x1, y1, z, width, road_material, rail_material) {
    let dx = x1 - x0
    let dy = y1 - y0
    let length = Math.sqrt(dx * dx + dy * dy)
    return T.position((x0 + x1) / 2.0, (y0 + y1) / 2.0, z).rotation(0.0, -Math.pi / 2.0, 0.0) {
        T.rotation(Math.atan2(dy, dx), 0.0, 0.0) {
            T.position(0.0, -0.15, 0.0).scale(width, 0.30, length) {
                R.cube() { road_material C.rgba(0.24, 0.23, 0.28, 1.0) }
                Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.56) {} }
            }
            if rail_material {
                for side in [-1.0, 1.0] {
                    T.position(side * (width / 2.0 - 0.09), 0.14, 0.0).scale(0.18, 0.28, length - 0.30) {
                        R.cube() { rail_material C.rgba(0.82, 0.68, 0.69, 1.0) }
                        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static() {} }
                    }
                }
            }
        }
    }
}

fn flat_road(x, y, z, x_size, z_size, road_material) {
    return T.position(x, y, z).rotation(-Math.pi / 2.0, 0.0, 0.0).scale(x_size, z_size, 1.0) {
        R.plane() { road_material C.rgba(0.24, 0.23, 0.28, 1.0) }
    }
}

fn grass_tile(x0, x1, z0, z1, y) {
    let width = x1 - x0
    let depth = z1 - z0
    // Anchor the texture to the terrain. Whole grid tiles share the same UVs
    // and mesh, and the narrower tiles meet them without changing texel density.
    let u0 = x0 / 2.0 - Math.floor(x0 / 2.0)
    let v0 = z0 / 2.0 - Math.floor(z0 / 2.0)
    let u1 = u0 + width / 2.0
    let v1 = v0 + depth / 2.0
    return T.position((x0 + x1) / 2.0, y, (z0 + z1) / 2.0).rotation(-Math.pi / 2.0, 0.0, 0.0).scale(width, depth, 1.0) {
        R.plane() {
            C.rgba(1.0, 1.0, 1.0, 1.0)
            UV.uv(u0,v1).uv(u1,v1).uv(u1,v0).uv(u0,v0)
            Texture.with_uri("assets/textures/grass/ship-park-grass.png")
            TextureFiltering.linear()
        }
    }
}

fn grass_region(x0, x1, z0, z1, y) {
    let tile = 40.0
    let gx = Math.floor(x0 / tile)
    let gz = Math.floor(z0 / tile)
    return T {
        for iz in range(Math.ceil(z1 / tile) - gz) {
            let z = (gz + iz) * tile
            for ix in range(Math.ceil(x1 / tile) - gx) {
                let x = (gx + ix) * tile
                grass_tile(Math.clamp(x, x0, x1), Math.clamp(x + tile, x0, x1),
                    Math.clamp(z, z0, z1), Math.clamp(z + tile, z0, z1), y)
            }
        }
    }
}

fn corner_grass_outline(radius, outside, segments) {
    // Match the polygon builder's vertex order so the UVs stay aligned.
    let outline = "[[0," + (-radius) + "]"
    if outside { outline = outline + ",[" + radius + "," + (-radius) + "],[" + radius + ",0]" }
    for index in range(segments - 1) {
        let step = index + 1
        if outside { step = segments - step }
        let angle = -Math.pi / 2.0 * (1.0 - step / segments)
        outline = outline + ",[" + radius * Math.cos(angle) + "," + radius * Math.sin(angle) + "]"
    }
    if !outside { outline = outline + ",[" + radius + ",0],[0,0]" }
    return JSON.parse(outline + "]")
}

fn corner_grass(label, outline, x, y, z) {
    return T.position(x, y, z).rotation(-Math.pi / 2.0, 0.0, 0.0) {
        R.polygon(label, outline) {
            C.rgba(1.0, 1.0, 1.0, 1.0)
            UV {
                for point in outline { uv((x + point[0]) / 2.0, (z - point[1]) / 2.0) }
            }
            Texture.with_uri("assets/textures/grass/ship-park-grass.png")
            TextureFiltering.linear()
        }
    }
}

export fn sakura_twilight_street(roof_x, roof_height, ramp_z, ground_material, road_material, rail_material, single_tower) {
    let road_z = -13.0
    let road_width = 9.0
    let road_x0 = -100.0
    let road_x1 = 188.0
    let road_length = road_x1 - road_x0
    let ground_y = -0.23
    let surface_y = ground_y + 0.004
    let ground_x0 = -120.0
    let ground_x1 = 208.0
    let ground_z0 = road_z - 90.0
    let ground_z1 = road_z + 90.0
    let road_far_z = road_z - road_width / 2.0
    let ramp_top_x = roof_x + 6.20
    let ramp_bottom_x = ramp_top_x + 40.0
    let ramp_width = 4.25
    let easing_run = 8.0
    let grade = (roof_height - ground_y) / (ramp_bottom_x - ramp_top_x - easing_run / 2.0)
    let easing_top_y = ground_y + grade * easing_run / 2.0
    let landing_end_x = ramp_bottom_x + 6.0
    let turn_radius = 7.0
    let turn_end_x = landing_end_x + turn_radius
    let turn_end_z = ramp_z - turn_radius
    let road_edge_z = road_z + road_width / 2.0
    let inner_radius = turn_radius - ramp_width / 2.0
    let outer_radius = turn_radius + ramp_width / 2.0
    let curve_segments = 32
    let grass_regions = [
        [ground_x0, ground_x1, ground_z0, road_far_z],
        [ground_x0, road_x0, road_far_z, road_edge_z],
        [road_x1, ground_x1, road_far_z, road_edge_z],
        [ground_x0, turn_end_x - ramp_width / 2.0, road_edge_z, turn_end_z],
        [turn_end_x + ramp_width / 2.0, ground_x1, road_edge_z, turn_end_z],
        [ground_x0, landing_end_x, turn_end_z, ramp_z - ramp_width / 2.0],
        [ground_x0, ramp_bottom_x, ramp_z - ramp_width / 2.0, ramp_z + ramp_width / 2.0],
        [landing_end_x + outer_radius, ground_x1, turn_end_z, ramp_z + ramp_width / 2.0],
        [ground_x0, ground_x1, ramp_z + ramp_width / 2.0, ground_z1],
    ]
    return T {
        T { name = "sakura_twilight_ground"
            ground_material
            T.position((ground_x0 + ground_x1) / 2.0, ground_y - 0.25, road_z)
                .scale(ground_x1 - ground_x0, 0.50, ground_z1 - ground_z0) {
                Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.85) {} }
            }
            for region in grass_regions {
                grass_region(region[0], region[1], region[2], region[3], surface_y)
            }
            // Use the same angular subdivisions as the road arc at both seams.
            corner_grass("sakura/street/grass/inside-turn", corner_grass_outline(inner_radius, false, curve_segments),
                landing_end_x, surface_y, turn_end_z)
            corner_grass("sakura/street/grass/outside-turn", corner_grass_outline(outer_radius, true, curve_segments),
                landing_end_x, surface_y, turn_end_z)
        }
        T { name = "sakura_twilight_road_ramp"
            ramp_segment(ramp_top_x, roof_height, ramp_bottom_x - easing_run, easing_top_y, ramp_z, ramp_width, road_material, rail_material)
            // The grade approaches zero before the landing. Each surface joins
            // the next at the same height, without a raised road collider.
            for segment in range(4) {
                let r0 = easing_run * (1.0 - segment / 4.0)
                let r1 = easing_run * (1.0 - (segment + 1.0) / 4.0)
                let y0 = ground_y + grade * r0 * r0 / (2.0 * easing_run)
                let y1 = ground_y + grade * r1 * r1 / (2.0 * easing_run)
                ramp_segment(ramp_bottom_x - r0, y0, ramp_bottom_x - r1, y1, ramp_z, ramp_width, road_material, null)
            }
        }
        T.position(road_x1 - 36.0, ground_y, road_z - 10.0) {
            radio_tower("sakura_twilight_radio_tower_1", rail_material)
        }
        if !single_tower {
            T.position(road_x1 - 22.0, ground_y, road_z + 11.0) {
                radio_tower("sakura_twilight_radio_tower_2", rail_material)
            }
            T.position(road_x1 - 10.0, ground_y, road_z - 12.0) {
                radio_tower("sakura_twilight_radio_tower_3", rail_material)
            }
        }
        T { name = "sakura_twilight_road"
            Raycastable.disabled()
            flat_road((road_x0 + road_x1) / 2.0, surface_y, road_z, road_length, road_width, road_material)
            T { name = "sakura_twilight_ramp_landing"
                flat_road((ramp_bottom_x + landing_end_x) / 2.0, surface_y, ramp_z, landing_end_x - ramp_bottom_x, ramp_width, road_material)
            }
            T.position(landing_end_x, surface_y, turn_end_z).rotation(-Math.pi / 2.0, 0.0, 0.0) {
                name = "sakura_twilight_road_turn"
                R.partial_annulus_2d(inner_radius, outer_radius,
                    -Math.pi / 2.0, Math.pi / 2.0, curve_segments) { road_material C.rgba(0.24, 0.23, 0.28, 1.0) }
            }
            flat_road(turn_end_x, surface_y, (turn_end_z + road_edge_z) / 2.0, ramp_width, turn_end_z - road_edge_z, road_material)
            CombineMesh.keep_transforms() {
                name = "sakura_twilight_road_dashes"
                road_material
                for dash in range(Math.floor(road_length / 4.0)) {
                    T.position(road_x0 + 2.0 + dash * 4.0, -0.201, road_z).scale(2.0, 0.01, 0.18) {
                        R.cube() { C.rgba(0.93, 0.90, 0.80, 1.0) }
                    }
                }
            }
        }
    }
}
