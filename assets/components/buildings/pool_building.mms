// Pool ceramics as architecture: real open bays, floor slabs and balconies.
// No glass, hidden solid building block, or baked texture-only windows.
export fn ceramic_box(position, size, color, material) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        R.cube() {
            Raycastable.enabled()
            material
            C.rgba(color[0], color[1], color[2], 1.0)
            Texture.with_uri("assets/textures/pool/white-tiles-surface.png")
            TextureFiltering.linear()
        }
        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.85) {} }
    }
}

// Keep visual meshes separate from collision topology: CombineMesh may collapse
// its source transforms without deleting the building's physical walls/floors.
fn ceramic_parts(base_y) {
    return { base_y = base_y white = {} trim = {} white_count = 0 trim_count = 0 }
}

fn building_box(data, position, size, trim) {
    let part = { position = [position[0], position[1] - data.base_y, position[2]] size = size }
    if trim {
        let parts = data.trim
        parts["" + data.trim_count] = part
        data.trim_count = data.trim_count + 1
    } else {
        let parts = data.white
        parts["" + data.white_count] = part
        data.white_count = data.white_count + 1
    }
}

fn combined_ceramic(label, parts, count, color, material) {
    return CombineMesh.keep_transforms() {
        name = label
        // Retained boxes remain BVH ray targets; only the combined mesh draws.
        Raycastable.enabled()
        material
        Texture.with_uri("assets/textures/pool/white-tiles-surface.png")
        TextureFiltering.linear()
        for index in range(count) {
            let part = parts["" + index]
            T.position(part.position[0], part.position[1], part.position[2])
                .scale(part.size[0], part.size[1], part.size[2]) {
                R.cube() { C.rgba(color[0], color[1], color[2], 1.0) }
            }
        }
    }
}

fn building_columns(data, y, height, width, depth) {
    for column in range(4) {
        let x = -width / 2.0 + column * width / 3.0
        let z = -depth / 2.0 + column * depth / 3.0
        for side in [-1.0, 1.0] {
            building_box(data, [x, y, side * depth / 2.0], [0.32, height, 0.32], false)
            building_box(data, [side * width / 2.0, y, z], [0.32, height, 0.32], false)
        }
    }
}

fn building_collisions(label, data) {
    return T {
        name = label + "_collisions"
        for group in [{ parts = data.white count = data.white_count }, { parts = data.trim count = data.trim_count }] {
            for index in range(group.count) {
                let part = group.parts["" + index]
                T.position(part.position[0], part.position[1], part.position[2])
                    .scale(part.size[0], part.size[1], part.size[2]) {
                    Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.85) {} }
                }
            }
        }
    }
}

// Each section has a local editing frame and owns its visual sources/colliders.
// Separate tint groups retain the ceramic material while keeping rebuilds local.
fn building_section(label, data, white, trim_color, tiles) {
    return T.position(0.0, data.base_y, 0.0) {
        name = label
        combined_ceramic(label + "_ceramic_white", data.white, data.white_count, white, tiles)
        combined_ceramic(label + "_ceramic_trim", data.trim, data.trim_count, trim_color, tiles)
        building_collisions(label, data)
    }
}

export fn pool_building(label, origin, floors, street_side, roof_access, tiles, trim_color, full_detail, per_floor) {
    let width = 14.0
    let depth = 12.0
    let storey = 3.2
    let height = floors * storey
    let white = [0.94, 0.96, 0.97]
    let slab = 0.22
    if full_detail == null { full_detail = true }
    if roof_access || per_floor { full_detail = true }
    let first_floor = 0
    if !full_detail { first_floor = floors - 2 }
    if first_floor < 0 { first_floor = 0 }
    let data = ceramic_parts(0.0)
    let sections = {}
    if per_floor {
        // The roof slab and parapets form their own section above the storeys.
        for floor in range(floors + 1) {
            sections["" + floor] = ceramic_parts(floor * storey)
        }
    }
    if first_floor > 0 {
        let solid_height = first_floor * storey
        building_box(data, [0.0, solid_height / 2.0 - slab, 0.0], [width, solid_height, depth], false)
    }
    for floor in range(floors) {
        if floor >= first_floor {
            let y = floor * storey
            let floor_data = data
            if per_floor { floor_data = sections["" + floor] }
            building_box(floor_data, [0.0, y - slab / 2.0, 0.0], [width, slab, depth], false)
            // Horizontal spandrels leave 1.95 m of open window height.
            for side in [-1.0, 1.0] {
                // Front/back facades along Z; columns are emitted separately.
                building_box(floor_data, [0.0, y + 0.45, side * depth / 2.0], [width, 0.9, 0.24], false)
                building_box(floor_data, [0.0, y + 3.0, side * depth / 2.0], [width, 0.4, 0.24], true)
                // Street-facing facade has a door-sized opening on the ground floor.
                if floor == 0 && side == street_side {
                    for end in [-1.0, 1.0] {
                        building_box(floor_data, [side * width / 2.0, y + 0.45, end * 4.0], [0.24, 0.9, 4.0], false)
                    }
                } else {
                    building_box(floor_data, [side * width / 2.0, y + 0.45, 0.0], [0.24, 0.9, depth], false)
                }
                building_box(floor_data, [side * width / 2.0, y + 3.0, 0.0], [0.24, 0.4, depth], true)
            }
            if floor > 0 {
                // One continuous balcony per storey, facing the street.
                let x = street_side * (width / 2.0 + 0.95)
                building_box(floor_data, [x, y - 0.10, 0.0], [1.9, 0.2, 11.0], false)
                building_box(floor_data, [street_side * (width / 2.0 + 1.85), y + 0.48, 0.0], [0.14, 0.96, 11.0], true)
                for end in [-1.0, 1.0] {
                    building_box(floor_data, [x, y + 0.48, end * 5.45], [1.9, 0.96, 0.14], false)
                }
            }
            if per_floor { building_columns(floor_data, y + storey / 2.0, storey, width, depth) }
        }
    }
    // Distant buildings keep continuous columns and one combined pair overall.
    if !per_floor {
        building_columns(data, (height + first_floor * storey) / 2.0, height - first_floor * storey, width, depth)
    }
    let roof_data = data
    let stair_data = data
    if per_floor {
        roof_data = sections["" + floors]
        stair_data = sections["" + (floors - 1)]
    }
    if roof_access {
        // Roof opening x=[-5.8,-3.4], z=[1.2,5.2], reached from the top floor.
        building_box(roof_data, [-6.4, height - slab / 2.0, 0.0], [1.2, slab, 12.0], false)
        building_box(roof_data, [1.8, height - slab / 2.0, 0.0], [10.4, slab, 12.0], false)
        building_box(roof_data, [-4.6, height - slab / 2.0, -2.4], [2.4, slab, 7.2], false)
        building_box(roof_data, [-4.6, height - slab / 2.0, 5.6], [2.4, slab, 0.8], false)
        for step in range(16) {
            let rise = (step + 1) * 0.2
            building_box(stair_data, [-4.6, height - storey + rise / 2.0, 5.05 - step * 0.24], [2.0, rise, 0.24], false)
        }
    } else {
        building_box(roof_data, [0.0, height - slab / 2.0, 0.0], [width, slab, depth], false)
    }
    // Low rooftop parapets leave the city visible and the edge reachable.
    for side in [-1.0, 1.0] {
        building_box(roof_data, [side * width / 2.0, height + 0.30, 0.0], [0.22, 0.60, depth], true)
        building_box(roof_data, [0.0, height + 0.30, side * depth / 2.0], [width, 0.60, 0.22], true)
    }
    let detail_label = "_full_detail"
    if !full_detail { detail_label = "_top_two_only" }
    return T.position(origin[0], origin[1], origin[2]) {
        name = label
        T {
            name = label + detail_label
            if per_floor {
                for floor in range(floors) {
                    building_section(label + "_floor_" + floor, sections["" + floor], white, trim_color, tiles)
                }
                building_section(label + "_roof", roof_data, white, trim_color, tiles)
            } else {
                combined_ceramic(label + "_ceramic_white", data.white, data.white_count, white, tiles)
                combined_ceramic(label + "_ceramic_trim", data.trim, data.trim_count, trim_color, tiles)
                building_collisions(label, data)
            }
        }
    }
}
