// examples/signal-handler.mms
// Three clickable cubes. Click one — MMS prints which cube was clicked.
// Demonstrates on() signal handler registration from MMS script.

let cube_a = T.position(-1.2, 0.0, 0.0).scale(0.4, 0.4, 0.4) {
    name = "cube_a"
    R.cube() {
        C.rgba(0.25, 0.55, 1.0, 1.0)
        Raycastable.enabled()
    }
}

let cube_b = T.position(0.0, 0.0, 0.0).scale(0.4, 0.4, 0.4) {
    name = "cube_b"
    R.cube() {
        C.rgba(0.30, 0.85, 0.45, 1.0)
        Raycastable.enabled()
    }
}

let cube_c = T.position(1.2, 0.0, 0.0).scale(0.4, 0.4, 0.4) {
    name = "cube_c"
    R.cube() {
        C.rgba(1.0, 0.55, 0.20, 1.0)
        Raycastable.enabled()
    }
}

on(cube_a, "Click", fn(event) {
    print("clicked: cube_a (blue)")
})

on(cube_b, "Click", fn(event) {
    print("clicked: cube_b (green)")
})

on(cube_c, "Click", fn(event) {
    print("clicked: cube_c (orange)")
})
