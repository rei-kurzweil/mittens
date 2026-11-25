import { radio_tower } from "./radio_tower.mms"
import { cartoon_grass } from "../materials/cartoon_grass.mms"

// One shared grass material, made on first use.
let street = { grass = null }
fn street_grass() {
    if street.grass == null { street.grass = cartoon_grass("far", null) }
    return street.grass
}

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
    // Anchor the texture to the terrain, one repeat per 4 m. Whole grid tiles
    // share the same UVs and mesh, and the narrower tiles meet them without
    // changing texel density.
    let u0 = x0 / 4.0 - Math.floor(x0 / 4.0)
    let v0 = z0 / 4.0 - Math.floor(z0 / 4.0)
    let u1 = u0 + width / 4.0
    let v1 = v0 + depth / 4.0
    return T.position((x0 + x1) / 2.0, y, (z0 + z1) / 2.0).rotation(-Math.pi / 2.0, 0.0, 0.0).scale(width, depth, 1.0) {
        R.plane() {
            street_grass()
            C.rgba(0.80, 0.96, 0.62, 1.0)
            UV.uv(u0,v1).uv(u1,v1).uv(u1,v0).uv(u0,v0)
            Texture.with_uri("assets/textures/grass/cartoon-grass-tufts.png")
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
            street_grass()
            C.rgba(0.80, 0.96, 0.62, 1.0)
            UV {
                for point in outline { uv((x + point[0]) / 4.0, (z - point[1]) / 4.0) }
            }
            Texture.with_uri("assets/textures/grass/cartoon-grass-tufts.png")
            TextureFiltering.linear()
        }
    }
}

fn shack_box(position, size, material, color) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        Destroyable.threshold(3.0)
        R.cube() { Raycastable.enabled() material C.rgba(color[0], color[1], color[2], 1.0) }
        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.85) {} }
    }
}

// A small equipment shack (3 x 2.6 x 3 m, door on its -Z side), destroyable.
fn shack(material) {
    let wall = [0.62, 0.60, 0.56]
    let roof = [0.30, 0.30, 0.34]
    return T {
        name = "sakura_twilight_tower_shack"
        shack_box([0.0, 1.3, 1.45], [3.0, 2.6, 0.1], material, wall)
        for side in [-1.0, 1.0] {
            shack_box([side * 1.45, 1.3, 0.0], [0.1, 2.6, 3.0], material, wall)
            shack_box([side * 0.95, 1.3, -1.45], [1.1, 2.6, 0.1], material, wall)
        }
        shack_box([0.0, 2.3, -1.45], [0.8, 0.6, 0.1], material, wall)
        shack_box([0.0, 2.65, 0.0], [3.3, 0.1, 3.3], material, roof)
    }
}

fn add_region(out, x0, x1, z0, z1) {
    if x1 - x0 > 0.001 && z1 - z0 > 0.001 {
        let items = out.items
        items["" + out.count] = [x0, x1, z0, z1]
        out.count = out.count + 1
    }
}

// Up to four rectangles around a hole: full-width strips before and after it
// in Z, then the pieces to either side of it in X.
fn cut_hole(out, r, h) {
    if h[0] >= r[1] || h[1] <= r[0] || h[2] >= r[3] || h[3] <= r[2] {
        add_region(out, r[0], r[1], r[2], r[3])
        return
    }
    let hz0 = Math.clamp(h[2], r[2], r[3])
    let hz1 = Math.clamp(h[3], r[2], r[3])
    add_region(out, r[0], r[1], r[2], hz0)
    add_region(out, r[0], r[1], hz1, r[3])
    add_region(out, r[0], Math.clamp(h[0], r[0], r[1]), hz0, hz1)
    add_region(out, Math.clamp(h[1], r[0], r[1]), r[1], hz0, hz1)
}

// Regions are [x0, x1, z0, z1]; holes punch rectangles out of all of them.
fn subtract_holes(regions, holes) {
    let current = { items = {} count = 0 }
    for r in regions { add_region(current, r[0], r[1], r[2], r[3]) }
    if holes {
        for h in holes {
            let next = { items = {} count = 0 }
            for index in range(current.count) { cut_hole(next, current.items["" + index], h) }
            current = next
        }
    }
    return current
}

// ground_holes: optional [x0, x1, z0, z1] footprints (a sunken building) with
// neither grass nor ground collision, so nothing draws under their floors.
export fn sakura_twilight_street(roof_x, roof_height, ramp_z, ground_material, road_material, rail_material, single_tower, ground_holes) {
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
    let grass = subtract_holes(grass_regions, ground_holes)
    let solid = subtract_holes([[ground_x0, ground_x1, ground_z0, ground_z1]], ground_holes)
    return T {
        T { name = "sakura_twilight_ground"
            ground_material
            for index in range(solid.count) {
                let region = solid.items["" + index]
                T.position((region[0] + region[1]) / 2.0, ground_y - 0.25, (region[2] + region[3]) / 2.0)
                    .scale(region[1] - region[0], 0.50, region[3] - region[2]) {
                    Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.85) {} }
                }
            }
            for index in range(grass.count) {
                let region = grass.items["" + index]
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
            // Between the guy wires, six metres out, door toward the tower.
            T.position(5.8, 0.0, -1.4).rotation(0.0, 1.8 + Math.pi, 0.0) { shack(rail_material) }
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
