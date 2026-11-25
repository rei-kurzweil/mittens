// A reusable translucent unlit cloud. `puff_count` is currently either 3 or 5;
// `base_width` is the approximate world-space width of the bottom puff row.
// `components` is a caller-owned component subtree, usually a Color with an
// optional visual modifiers, so individual clouds can choose their own color.
// Supply a larger world-space voxel size for distant, scaled-up clouds.
export fn cloud(puff_count, base_width, components, voxel_size) {
    let radius = base_width * 0.27
    let upper_radius = base_width * 0.23
    if voxel_size == null { voxel_size = 0.14 }

    return ImplicitSurface
        .bounds(
            -base_width * 0.58, -base_width * 0.48, -base_width * 0.34,
             base_width * 0.58,  base_width * 0.56,  base_width * 0.34,
        )
        .voxel_size(voxel_size)
        .iso_level(0.0)
        .smooth_min_radius(base_width * 0.10) {

        Opacity.opacity(0.58)
        Unlit {}
        components

        if puff_count <= 3.0 {
            T.position(-base_width * 0.24, -base_width * 0.12, 0.00) {
                ImplicitSphere.radius(radius) {}
            }
            T.position( base_width * 0.24, -base_width * 0.12, 0.04) {
                ImplicitSphere.radius(radius) {}
            }
            T.position(0.00, base_width * 0.16, 0.06) {
                ImplicitSphere.radius(upper_radius) {}
            }
        } else {
            T.position(-base_width * 0.26, -base_width * 0.15, 0.00) {
                ImplicitSphere.radius(radius) {}
            }
            T.position(0.00, -base_width * 0.18, 0.07) {
                ImplicitSphere.radius(radius * 1.05) {}
            }
            T.position( base_width * 0.26, -base_width * 0.14, 0.00) {
                ImplicitSphere.radius(radius) {}
            }
            T.position(-base_width * 0.13, base_width * 0.18, 0.06) {
                ImplicitSphere.radius(upper_radius) {}
            }
            T.position( base_width * 0.14, base_width * 0.20, 0.03) {
                ImplicitSphere.radius(upper_radius) {}
            }
        }
    }
}
