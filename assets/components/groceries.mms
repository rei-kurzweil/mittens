// Retained, low-poly grocery geometry. Callers batch matching materials/textures.
// Cylinder side vertices are two rings of 21; map U around the jar and V down
// the label rather than using the tube primitive's native longitudinal U.
fn wrapper_uv() {
    return UV.uv(0.00, 1.0).uv(0.05, 1.0).uv(0.10, 1.0).uv(0.15, 1.0)
        .uv(0.20, 1.0).uv(0.25, 1.0).uv(0.30, 1.0).uv(0.35, 1.0)
        .uv(0.40, 1.0).uv(0.45, 1.0).uv(0.50, 1.0).uv(0.55, 1.0)
        .uv(0.60, 1.0).uv(0.65, 1.0).uv(0.70, 1.0).uv(0.75, 1.0)
        .uv(0.80, 1.0).uv(0.85, 1.0).uv(0.90, 1.0).uv(0.95, 1.0).uv(1.00, 1.0)
        .uv(0.00, 0.0).uv(0.05, 0.0).uv(0.10, 0.0).uv(0.15, 0.0)
        .uv(0.20, 0.0).uv(0.25, 0.0).uv(0.30, 0.0).uv(0.35, 0.0)
        .uv(0.40, 0.0).uv(0.45, 0.0).uv(0.50, 0.0).uv(0.55, 0.0)
        .uv(0.60, 0.0).uv(0.65, 0.0).uv(0.70, 0.0).uv(0.75, 0.0)
        .uv(0.80, 0.0).uv(0.85, 0.0).uv(0.90, 0.0).uv(0.95, 0.0).uv(1.00, 0.0)
}

export fn orange() {
    return T.scale(0.15, 0.14, 0.15) {
        R.icosahedron(1, 1.0) { C.rgba(1.0, 0.37, 0.025, 1.0) }
        T.position(0.0, 0.48, 0.0).scale(0.13, 0.04, 0.13) {
            R.cube() { C.rgba(0.20, 0.32, 0.07, 1.0) }
        }
    }
}

// A short curved tube gives the banana thickness from every XR viewpoint.
// Six sides keep a slightly faceted peel; small end caps taper into dark tips.
export fn banana() {
    return T {
        R.helix_tube(0.14, 0.027, 0.0, 0.35, 2.25, 10, 6) {
            C.rgba(0.98, 0.79, 0.10, 1.0)
        }
        for angle in [0.35, 2.60] {
            T.position(0.14 * Math.cos(angle), 0.0, 0.14 * Math.sin(angle)).scale(0.04, 0.04, 0.04) {
                R.icosahedron(0, 1.0) { C.rgba(0.48, 0.34, 0.055, 1.0) }
            }
        }
    }
}

export fn toilet_paper_roll() {
    return T {
        R.cylinder(0.055, 0.11, 16) { C.rgba(0.96, 0.95, 0.89, 1.0) }
        T.position(0.0, 0.111, 0.0) {
            R.cylinder(0.018, 0.002, 12) { C.rgba(0.26, 0.22, 0.17, 1.0) }
        }
    }
}

// Split into material/texture layers so a shelf of jars needs three batches,
// rather than mixing the lid texture with the cylindrical wrapper in one batch.
export fn peanut_butter_jar(layer) {
    return T {
        if layer == "body" {
            R.cylinder(0.060, 0.19, 20) { C.rgba(0.48, 0.26, 0.095, 1.0) }
        }
        if layer == "label" {
            T.position(0.0, 0.035, 0.0) {
                R.cylinder(0.061, 0.12, 20) { C.rgba(1.0, 1.0, 1.0, 1.0) wrapper_uv() }
            }
        }
        if layer == "lid" {
            T.position(0.0, 0.19, 0.0) {
                R.cylinder(0.065, 0.026, 20) { C.rgba(0.78, 0.035, 0.045, 1.0) }
            }
        }
        if layer == "seal" {
            T.position(0.0, 0.217, 0.0).rotation(-Math.pi / 2.0, 0.0, 0.0).scale(0.128, 0.128, 1.0) {
                R.circle_2d() { C.rgba(1.0, 1.0, 1.0, 1.0) }
            }
        }
    }
}
