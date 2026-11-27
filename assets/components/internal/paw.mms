// Angle from the line of centers to either intersection, at the first circle.
fn intersection_angle(radius, other_radius, spacing) {
    let cosine = (radius * radius + spacing * spacing - other_radius * other_radius)
        / (2.0 * radius * spacing)
    return Math.atan2(Math.sqrt(1.0 - cosine * cosine), cosine)
}

fn heel_outline(center_radius, side_radius, overlap_fraction, lift, segments) {
    let spacing = center_radius + side_radius * (1.0 - overlap_fraction)
    let right = lift
    let left = Math.pi - lift
    let turn = 2.0 * Math.pi
    let side_x = spacing * Math.cos(lift)
    let side_y = spacing * Math.sin(lift)
    let center_cut = intersection_angle(center_radius, side_radius, spacing)
    let side_cut = intersection_angle(side_radius, center_radius, spacing)
    let left_inward = left + Math.pi
    let right_inward = right + Math.pi

    // The side discs intersect the center disc only. Walk the exposed arcs
    // counterclockwise, one sample per join. Order: top, left, bottom, right.
    let arcs = [
        { x = 0.0 y = 0.0 radius = center_radius first = right + center_cut last = left - center_cut },
        { x = -side_x y = side_y radius = side_radius first = left_inward + side_cut last = left_inward + turn - side_cut },
        { x = 0.0 y = 0.0 radius = center_radius first = left + center_cut last = right + turn - center_cut },
        { x = side_x y = side_y radius = side_radius first = right_inward + side_cut last = right_inward + turn - side_cut },
    ]
    let outline = []
    for arc in arcs {
        for step in range(segments) {
            let angle = arc.first + (arc.last - arc.first) * step / segments
            outline.push([
                arc.x + arc.radius * Math.cos(angle),
                arc.y + arc.radius * Math.sin(angle),
            ])
        }
    }
    return outline
}

export fn paw_print(color) {
    let center_radius = 0.10
    let side_radius = 0.066
    let overlap_fraction = 0.60
    let lift = 0.12
    let segments = 32
    let outline = heel_outline(center_radius, side_radius, overlap_fraction, lift, segments)
    let mesh_key = "paw/heel/v1/" + center_radius + "/" + side_radius + "/"
        + overlap_fraction + "/" + lift + "/" + segments

    // Radius is in toe-local units. circle_2d has a unit diameter.
    let toe_unit = 0.10
    let toe_vertical_scale = 0.44
    // x, y, radius, rotation about Z
    let toes = [
        [-0.158, 0.084, 0.42, 0.22],
        [-0.062, 0.113, 0.50, 0.12],
        [0.062, 0.113, 0.50, -0.12],
        [0.158, 0.084, 0.42, -0.22],
    ]

    return T {
        name = "paw_print"
        T.position(0.0, -0.03, 0.0) {
            R.polygon(mesh_key, outline) {
                C.rgba(color[0], color[1], color[2], color[3])
                Unlit {}
            }
        }
        for toe in toes {
            let diameter = 2.0 * toe[2] * toe_unit
            T.position(toe[0], toe[1], 0.0).rotation(0.0, 0.0, toe[3])
                .scale(diameter, diameter * toe_vertical_scale, 1.0) {
                R.circle_2d() {
                    C.rgba(color[0], color[1], color[2], color[3])
                    Unlit {}
                }
            }
        }
    }
}
