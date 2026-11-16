// A wall plug with a round hole, built from rolled boxes that also collide.
//
// Local X runs along the wall, Y is up from the floor, and Z is the wall
// normal. Ring segments have flat inner faces at `radius` and reach outward to
// a `half_size` square, clipped to the floor and ceiling heights. The hole is
// a real collision hole: a box narrower than the diameter in both directions
// can still be blocked by its corners.
export fn round_aperture_wall(label, center_height, radius, half_size, ceiling, thickness, segments, color, material) {
    let half_angle = Math.pi / segments
    return CombineMesh.keep_transforms() {
        name = label
        Raycastable.enabled()
        if material != null { material }
        for k in range(segments) {
            let angle = k * 2.0 * half_angle
            let c = Math.cos(angle)
            let s = Math.sin(angle)
            let reach = half_size / Math.abs(c)
            if Math.abs(s) > Math.abs(c) { reach = half_size / Math.abs(s) }
            if s < -0.001 && center_height / -s < reach { reach = center_height / -s }
            if s > 0.001 && (ceiling - center_height) / s < reach { reach = (ceiling - center_height) / s }
            if reach > radius + 0.02 {
                let middle = (radius + reach) / 2.0
                T.position(middle * c, center_height + middle * s, 0.0)
                    .rotation(0.0, 0.0, angle)
                    .scale(reach - radius, 2.0 * reach * Math.tan(half_angle) + 0.02, thickness) {
                    name = label + "_ring_" + k
                    R.cube() { C.rgba(color[0], color[1], color[2], 1.0) }
                    Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.85) {} }
                }
            }
        }
    }
}
