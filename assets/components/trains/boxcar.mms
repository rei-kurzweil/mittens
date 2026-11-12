// Two thin-cube boxcars coupled as one inert train body.
//
// Local +Z runs along the train. The root sits at the top of the rails, with
// the underframe touching Y=0 and the car body starting just above it. Doors
// are split into two leaves per side and swing outward so the openings read
// clearly from the track approaches.
//
// Slice 1 deliberately leaves the train without motion or event handlers. The
// named sweep-sensor zone/collidable pair is present for the next slice, but it
// has no bound Velocity and no listener.

fn shell_part(position, size, color, material) {
    return T.position(position[0], position[1], position[2])
        .scale(size[0], size[1], size[2]) {
        R.cube() {
            if material != null { material }
            C.rgba(color[0], color[1], color[2], 1.0)
        }
    }
}

fn door_leaf(x, z, y, height, leaf_length, direction, angle, color, material) {
    return T.position(x, y, z).rotation(0.0, angle, 0.0) {
        shell_part([0.0, 0.0, direction * leaf_length / 2.0],
            [0.05, height, leaf_length], color, material)
    }
}

export fn boxcar(label, position, length, width, height, body_color, roof_color, door_color, material) {
    let wall = 0.10
    let floor_bottom = 0.12
    let floor_thickness = 0.16
    let roof_thickness = 0.14
    let floor_top = floor_bottom + floor_thickness
    let wall_height = height - floor_top - roof_thickness
    let wall_center_y = floor_top + wall_height / 2.0
    let roof_center_y = height - roof_thickness / 2.0

    let door_length = 1.00
    let door_height = 1.70
    let door_leaf_length = 0.48
    let door_leaf_angle = 0.92
    let door_center_y = floor_top + door_height / 2.0
    let door_lintel_height = wall_height - door_height
    let door_lintel_y = floor_top + door_height + door_lintel_height / 2.0

    let side_x = width / 2.0 - wall / 2.0
    let end_z = length / 2.0 - wall / 2.0
    let side_segment_length = (length - door_length) / 2.0
    let side_segment_z = door_length / 2.0 + side_segment_length / 2.0

    return T.position(position[0], position[1], position[2]) {
        name = label

        // Underframe and bogies sit above the rail heads without a full wheel
        // simulation; the first slice is intentionally a visual proxy.
        shell_part([0.0, 0.06, 0.0], [width * 0.58, 0.12, length * 0.88], roof_color, material)
        for z in [-length * 0.30, length * 0.30] {
            shell_part([0.0, 0.06, z], [width * 0.74, 0.12, 0.56], roof_color, material)
        }

        // Floor, sides split around the door openings, ends, and roof.
        shell_part([0.0, floor_bottom + floor_thickness / 2.0, 0.0],
            [width, floor_thickness, length], body_color, material)

        for side in [-1.0, 1.0] {
            for segment in [-1.0, 1.0] {
                shell_part([side * side_x, wall_center_y, segment * side_segment_z],
                    [wall, wall_height, side_segment_length], body_color, material)
            }
            shell_part([side * side_x, door_lintel_y, 0.0],
                [wall, door_lintel_height, door_length], body_color, material)
        }

        for end in [-1.0, 1.0] {
            shell_part([0.0, wall_center_y, end * end_z],
                [width, wall_height, wall], body_color, material)
        }

        shell_part([0.0, roof_center_y, 0.0],
            [width + 0.14, roof_thickness, length + 0.12], roof_color, material)

        // Two leaves per side, hinged at the edges of each opening. The sign
        // makes each leaf swing away from the car instead of into it.
        for side in [-1.0, 1.0] {
            for direction in [-1.0, 1.0] {
                let angle = side * direction * door_leaf_angle
                door_leaf(
                    side * (width / 2.0 + 0.02),
                    direction * door_length / 2.0,
                    door_center_y,
                    door_height,
                    door_leaf_length,
                    direction,
                    angle,
                    door_color,
                    material
                )
            }
        }
    }
}

export fn boxcar_train(label, car_length, car_width, car_height, gap, body_color, roof_color, door_color, material) {
    let car_offset = (car_length + gap) / 2.0
    let total_length = car_length * 2.0 + gap

    return T {
        name = label

        boxcar(label + "_car_0", [0.0, 0.0, -car_offset],
            car_length, car_width, car_height, body_color, roof_color, door_color, material)
        boxcar(label + "_car_1", [0.0, 0.0, car_offset],
            car_length, car_width, car_height, body_color, roof_color, door_color, material)

        // A short coupler bridges the visible gap and the two underframes.
        shell_part([0.0, 0.10, 0.0], [0.18, 0.14, gap + 0.50], roof_color, material)

        // For the next slice: the train's sensor spans slightly past the body so
        // a building can be reported before the shell visually enters the wall.
        // No Velocity binding and no listener yet.
        T.position(0.0, car_height / 2.0, 0.0) {
            name = label + "_sweep_sensor_mount"
            Zone.cube([car_width / 2.0 + 0.15, car_height / 2.0, total_length / 2.0 + 0.80]) {
                name = label + "_sweep_sensor"
                Collidable.static().friction(0.0) {}
            }
        }
    }
}
