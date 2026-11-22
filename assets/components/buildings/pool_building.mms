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

export fn destroyable_ceramic_box(position, size, color, material) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        Destroyable.threshold(3.0)
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

// Facade opening on `floor`: `axis` "z" is a front/back facade at
// z = side * depth / 2 spanning X; "x" is a side facade spanning Z. The gap is
// [center - width / 2, center + width / 2] along the facade and rises `height`
// from the floor; a header fills the rest of the window band.
fn facade_opening(openings, floor, axis, side) {
    let found = null
    if openings != null {
        for opening in openings {
            if opening.floor == floor && opening.axis == axis && opening.side == side { found = opening }
        }
    }
    return found
}

// One horizontal facade band, split around an opening when present.
fn facade_band(data, opening, axis, side, offset, span, y, height, trim) {
    let intervals = [[-span / 2.0, span / 2.0]]
    if opening != null {
        intervals = [[-span / 2.0, opening.center - opening.width / 2.0], [opening.center + opening.width / 2.0, span / 2.0]]
    }
    for interval in intervals {
        let length = interval[1] - interval[0]
        if length > 0.01 {
            let along = (interval[0] + interval[1]) / 2.0
            if axis == "z" {
                building_box(data, [along, y, side * offset], [length, height, 0.24], trim)
            } else {
                building_box(data, [side * offset, y, along], [0.24, height, length], trim)
            }
        }
    }
}

fn opening_header(data, opening, offset, y, band_top) {
    if opening.height < band_top {
        let height = band_top - opening.height
        let header_y = y + opening.height + height / 2.0
        if opening.axis == "z" {
            building_box(data, [opening.center, header_y, opening.side * offset], [opening.width, height, 0.24], false)
        } else {
            building_box(data, [opening.side * offset, header_y, opening.center], [0.24, height, opening.width], false)
        }
    }
}

fn column_clear(opening, along) {
    if opening == null { return true }
    return Math.abs(along - opening.center) > opening.width / 2.0 + 0.16
}

fn building_columns(data, y, height, width, depth, openings, floor) {
    for column in range(4) {
        let x = -width / 2.0 + column * width / 3.0
        let z = -depth / 2.0 + column * depth / 3.0
        for side in [-1.0, 1.0] {
            if column_clear(facade_opening(openings, floor, "z", side), x) {
                building_box(data, [x, y, side * depth / 2.0], [0.32, height, 0.32], false)
            }
            if column_clear(facade_opening(openings, floor, "x", side), z) {
                building_box(data, [side * width / 2.0, y, z], [0.32, height, 0.32], false)
            }
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

// Destroyable manages retained primitives as one floor. Both material batches
// stay combined until a successful cut activates this section.
fn destroyable_section(label, data, white, trim_color, tiles) {
    return T.position(0.0, data.base_y, 0.0) {
        name = label
        Destroyable.threshold(3.0)
        for group in [{ parts = data.white count = data.white_count color = white suffix = "_ceramic_white" }, { parts = data.trim count = data.trim_count color = trim_color suffix = "_ceramic_trim" }] {
            CombineMesh.keep_transforms() {
                name = label + group.suffix
                tiles
                Texture.with_uri("assets/textures/pool/white-tiles-surface.png")
                TextureFiltering.linear()
                for index in range(group.count) {
                    let part = group.parts["" + index]
                    ceramic_box(part.position, part.size, group.color, tiles)
                }
            }
        }
    }
}

fn stairwell_slab(data, y, slab) {
    // Opening x=[-5.8,-1.4], z=[1.6,5.6]. The full-width front landing
    // connects both flights to each floor; all boxes also generate collision.
    building_box(data, [-6.4, y - slab / 2.0, 0.0], [1.2, slab, 12.0], false)
    building_box(data, [2.8, y - slab / 2.0, 0.0], [8.4, slab, 12.0], false)
    building_box(data, [-3.6, y - slab / 2.0, -2.2], [4.4, slab, 7.6], false)
    building_box(data, [-3.6, y - slab / 2.0, 5.8], [4.4, slab, 0.4], false)
}

fn stairwell_flights(data, y, storey, slab) {
    // Keep both runs within the same slab opening. Taller storeys add steps
    // while keeping each rise within the standard 0.16 m height.
    let half_storey = storey / 2.0
    let steps = Math.ceil(half_storey / 0.16)
    let tread = 2.8 / steps
    for step in range(steps) {
        let rise = (step + 1) * half_storey / steps
        let run = (step + 0.5) * tread
        building_box(data, [-4.7, y + rise - 0.06, 1.6 + run], [1.8, 0.12, tread], false)
        building_box(data, [-2.5, y + half_storey + rise - 0.06, 4.4 - run], [1.8, 0.12, tread], false)
    }
    building_box(data, [-3.6, y + half_storey - slab / 2.0, 5.0], [4.4, slab, 1.2], false)
}

// A count uses standard 3.2 m storeys. A nonempty array supplies heights
// from ground to roof, measured between the tops of successive slabs.
fn storey_layout(spec) {
    let count = len(spec)
    let uniform = count == 0
    if uniform { count = spec }
    let levels = {}
    let y = 0.0
    for floor in range(count) {
        let height = 3.2
        if !uniform { height = spec[floor] }
        levels["" + floor] = { y = y height = height }
        y = y + height
    }
    return { count = count levels = levels roof = y }
}

fn build_pool_building(label, origin, floor_spec, street_side, roof_access, tiles, trim_color, full_detail, per_floor, full_stairwell, openings) {
    let width = 14.0
    let depth = 12.0
    let layout = storey_layout(floor_spec)
    let floors = layout.count
    let height = layout.roof
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
        for floor in range(floors) {
            sections["" + floor] = ceramic_parts(layout.levels["" + floor].y)
        }
        sections["" + floors] = ceramic_parts(height)
    }
    let detail_start = 0.0
    if first_floor > 0 {
        detail_start = layout.levels["" + first_floor].y
        building_box(data, [0.0, detail_start / 2.0 - slab, 0.0], [width, detail_start, depth], false)
    }
    for floor in range(floors) {
        if floor >= first_floor {
            let level = layout.levels["" + floor]
            let y = level.y
            let storey = level.height
            let trim_height = Math.clamp(storey / 8.0, 0.0, 0.4)
            let band_top = storey - trim_height
            let sill_height = Math.clamp(band_top, 0.0, 0.9)
            let trim_y = y + storey - trim_height / 2.0
            let sill_y = y + sill_height / 2.0
            let floor_data = data
            if per_floor { floor_data = sections["" + floor] }
            if full_stairwell {
                if floor == 0 {
                    building_box(floor_data, [0.0, y - slab / 2.0, 0.0], [width, slab, depth], false)
                } else {
                    stairwell_slab(floor_data, y, slab)
                }
                stairwell_flights(floor_data, y, storey, slab)
            } else {
                building_box(floor_data, [0.0, y - slab / 2.0, 0.0], [width, slab, depth], false)
            }
            // The upper trim follows the ceiling and the low sill leaves an open window band.
            for side in [-1.0, 1.0] {
                // Front/back facades along Z; columns are emitted separately.
                let front_opening = facade_opening(openings, floor, "z", side)
                facade_band(floor_data, front_opening, "z", side, depth / 2.0, width, sill_y, sill_height, false)
                building_box(floor_data, [0.0, trim_y, side * depth / 2.0], [width, trim_height, 0.24], true)
                if front_opening != null { opening_header(floor_data, front_opening, depth / 2.0, y, band_top) }
                // Street-facing facade has a door-sized opening on the ground floor.
                let side_opening = facade_opening(openings, floor, "x", side)
                if floor == 0 && side == street_side {
                    for end in [-1.0, 1.0] {
                        building_box(floor_data, [side * width / 2.0, sill_y, end * 4.0], [0.24, sill_height, 4.0], false)
                    }
                } else {
                    facade_band(floor_data, side_opening, "x", side, width / 2.0, depth, sill_y, sill_height, false)
                }
                building_box(floor_data, [side * width / 2.0, trim_y, 0.0], [0.24, trim_height, depth], true)
                if side_opening != null { opening_header(floor_data, side_opening, width / 2.0, y, band_top) }
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
            if per_floor { building_columns(floor_data, y + storey / 2.0, storey, width, depth, openings, floor) }
        }
    }
    // Distant buildings keep continuous columns and one combined pair overall.
    if !per_floor {
        building_columns(data, (height + detail_start) / 2.0, height - detail_start, width, depth, null, -1)
    }
    let roof_data = data
    let stair_data = data
    if per_floor {
        roof_data = sections["" + floors]
        stair_data = sections["" + (floors - 1)]
    }
    if full_stairwell {
        stairwell_slab(roof_data, height, slab)
    } else if roof_access {
        // Roof opening x=[-5.8,-3.4], z=[1.2,5.2], reached from the top floor.
        building_box(roof_data, [-6.4, height - slab / 2.0, 0.0], [1.2, slab, 12.0], false)
        building_box(roof_data, [1.8, height - slab / 2.0, 0.0], [10.4, slab, 12.0], false)
        building_box(roof_data, [-4.6, height - slab / 2.0, -2.4], [2.4, slab, 7.2], false)
        building_box(roof_data, [-4.6, height - slab / 2.0, 5.6], [2.4, slab, 0.8], false)
        let top = layout.levels["" + (floors - 1)]
        let steps = Math.ceil(top.height / 0.2)
        let tread = 3.84 / steps
        for step in range(steps) {
            let rise = (step + 1) * top.height / steps
            building_box(stair_data, [-4.6, top.y + rise / 2.0, 5.17 - (step + 0.5) * tread], [2.0, rise, tread], false)
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
                    destroyable_section(label + "_floor_" + floor, sections["" + floor], white, trim_color, tiles)
                }
                destroyable_section(label + "_roof", roof_data, white, trim_color, tiles)
            } else {
                combined_ceramic(label + "_ceramic_white", data.white, data.white_count, white, tiles)
                combined_ceramic(label + "_ceramic_trim", data.trim, data.trim_count, trim_color, tiles)
                building_collisions(label, data)
            }
        }
    }
}

// `floors` accepts either a count or ground-to-roof floor-to-floor heights.
export fn pool_building(label, origin, floors, street_side, roof_access, tiles, trim_color, full_detail, per_floor) {
    return build_pool_building(label, origin, floors, street_side, roof_access, tiles, trim_color, full_detail, per_floor, false, null)
}

// Full-detail, floor-batched building with facade openings (see facade_opening).
// Openings exist in the collision boxes too, not only in the visuals.
export fn pool_building_with_openings(label, origin, floors, street_side, tiles, trim_color, openings) {
    return build_pool_building(label, origin, floors, street_side, false, tiles, trim_color, true, true, false, openings)
}

// Full-detail, floor-batched variant with a continuous ground-to-roof stairwell.
// Existing pool_building callers retain their original slabs and top-floor stairs.
export fn pool_building_with_stairs(label, origin, floors, street_side, tiles, trim_color) {
    return build_pool_building(label, origin, floors, street_side, true, tiles, trim_color, true, true, true, null)
}
