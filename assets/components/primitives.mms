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

export fn wireframe_square() {
    return primitive_shell("primitive_wireframe_square", R.wireframe_square(0.1) {
        C.rgba(0.12, 0.12, 0.12, 1.0)
    })
}

export fn annulus_2d() {
    return primitive_shell("primitive_annulus_2d", R.annulus_2d() {
        C.rgba(0.96, 0.56, 0.72, 1.0)
    })
}

export fn circle_2d() {
    return primitive_shell("primitive_circle_2d", R.circle_2d() {
        C.rgba(0.96, 0.56, 0.72, 1.0)
    })
}

export fn cone() {
    return primitive_shell("primitive_cone", R.cone() {
        C.rgba(0.98, 0.70, 0.34, 1.0)
    })
}

export fn polygon() {
    return primitive_shell("primitive_polygon", R.polygon("primitives/hexagon/v1", [[0.0, 0.5], [0.43, 0.25], [0.43, -0.25], [0.0, -0.5], [-0.43, -0.25], [-0.43, 0.25]]) {
        C.rgba(0.58, 0.88, 0.52, 1.0)
    })
}

export fn wireframe_box() {
    return primitive_shell("primitive_wireframe_box", R.wireframe_box() {
        C.rgba(0.38, 0.78, 0.92, 1.0)
    })
}

export fn wireframe_sphere() {
    return primitive_shell("primitive_wireframe_sphere", R.wireframe_sphere() {
        C.rgba(0.62, 0.72, 0.98, 1.0)
    })
}

export fn wireframe_icosahedron() {
    return primitive_shell("primitive_wireframe_icosahedron", R.wireframe_icosahedron() {
        C.rgba(0.76, 0.62, 0.96, 1.0)
    })
}

export fn tetrahedron() {
    return primitive_shell("primitive_tetrahedron", R.tetrahedron() {
        C.rgba(0.96, 0.64, 0.18, 1.0)
    })
}

export fn icosahedron() {
    return primitive_shell("primitive_icosahedron", R.icosahedron() {
        C.rgba(0.42, 0.64, 0.96, 1.0)
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
