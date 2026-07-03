// assets/components/primitives.mms
//
// A small shelf of spawnable geometry primitives for the asset panel.
// Each export wraps one Renderable constructor in a simple factory so the
// module shows up as a set of ready-to-drop shapes.

fn primitive_shell(root_name, content) {
    return T.scale(0.45, 0.45, 0.45) {
        name = root_name
        content
    }
}

export fn cube() {
    return primitive_shell("primitive_cube", R.cube() {
        C.rgba(0.92, 0.42, 0.32, 1.0)
    })
}

export fn sphere() {
    return primitive_shell("primitive_sphere", R.sphere() {
        C.rgba(0.30, 0.72, 0.96, 1.0)
    })
}

export fn plane() {
    return primitive_shell("primitive_plane", R.plane() {
        C.rgba(0.24, 0.80, 0.60, 1.0)
    })
}

export fn triangle() {
    return primitive_shell("primitive_triangle", R.triangle() {
        C.rgba(0.98, 0.78, 0.24, 1.0)
    })
}

export fn square() {
    return primitive_shell("primitive_square", R.square() {
        C.rgba(0.78, 0.52, 0.96, 1.0)
    })
}

export fn circle2d() {
    return primitive_shell("primitive_circle2d", R.circle2d() {
        C.rgba(0.96, 0.56, 0.72, 1.0)
    })
}

export fn tetrahedron() {
    return primitive_shell("primitive_tetrahedron", R.tetrahedron() {
        C.rgba(0.96, 0.64, 0.18, 1.0)
    })
}

export fn star() {
    return primitive_shell("primitive_star", R.star() {
        C.rgba(1.0, 0.90, 0.30, 1.0)
    })
}

export fn heart() {
    return primitive_shell("primitive_heart", R.heart() {
        C.rgba(0.96, 0.22, 0.46, 1.0)
    })
}

export fn partial_annulus_2d() {
    return primitive_shell(
        "primitive_partial_annulus_2d",
        R.partial_annulus_2d() {
            C.rgba(0.22, 0.86, 0.86, 1.0)
        }
    )
}
