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

fn flat_road(x, z, x_size, z_size, road_material) {
    return T.position(x, -0.210, z).rotation(-Math.pi / 2.0, 0.0, 0.0).scale(x_size, z_size, 1.0) {
        R.plane() { road_material C.rgba(0.24, 0.23, 0.28, 1.0) }
    }
}

export fn sakura_twilight_street(roof_x, roof_height, ramp_z, ground_material, road_material, rail_material) {
    let road_z = -13.0
    let road_width = 9.0
    let road_length = 200.0
    let ground_y = -0.23
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
    return T {
        T { name = "sakura_twilight_ground"
            T.position(0.0, ground_y - 0.25, road_z).scale(240.0, 0.50, 180.0) {
                Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.85) {} }
            }
            T.position(0.0, ground_y + 0.004, road_z).rotation(-Math.pi / 2.0, 0.0, 0.0).scale(240.0, 180.0, 1.0) {
                R.plane() {
                    ground_material
                    C.rgba(1.0, 1.0, 1.0, 1.0)
                    UV.uv(-60,45).uv(60,45).uv(60,-45).uv(-60,-45)
                    Texture.with_uri("assets/textures/grass/ship-park-grass.png")
                    TextureFiltering.linear()
                }
            }
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
        T { name = "sakura_twilight_road"
            Raycastable.disabled()
            flat_road(0.0, road_z, road_length, road_width, road_material)
            T { name = "sakura_twilight_ramp_landing"
                flat_road((ramp_bottom_x + landing_end_x) / 2.0, ramp_z, landing_end_x - ramp_bottom_x, ramp_width, road_material)
            }
            T.position(landing_end_x, -0.210, turn_end_z).rotation(-Math.pi / 2.0, 0.0, 0.0) {
                name = "sakura_twilight_road_turn"
                R.partial_annulus_2d(turn_radius - ramp_width / 2.0, turn_radius + ramp_width / 2.0,
                    -Math.pi / 2.0, Math.pi / 2.0, 32) { road_material C.rgba(0.24, 0.23, 0.28, 1.0) }
            }
            flat_road(turn_end_x, (turn_end_z + road_edge_z) / 2.0, ramp_width, turn_end_z - road_edge_z, road_material)
            CombineMesh.keep_transforms() {
                name = "sakura_twilight_road_dashes"
                road_material
                for dash in range(50) {
                    T.position(-road_length / 2.0 + 2.0 + dash * 4.0, -0.201, road_z).scale(2.0, 0.01, 0.18) {
                        R.cube() { C.rgba(0.93, 0.90, 0.80, 1.0) }
                    }
                }
            }
        }
    }
}
