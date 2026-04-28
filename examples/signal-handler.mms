// examples/signal-handler.mms
// Three clickable cubes. Click one — MMS prints which cube was clicked.
// Demonstrates on() signal handler registration from MMS script.

RendererSettings {
    window_size(1280, 720)
}

BGC.rgba(0.10, 0.10, 0.14, 1.0)
AL.rgb(0.35, 0.35, 0.38)

T.position(3.0, 5.0, 4.0) {
    DL {}
}

I.speed(3.0) {
    InputTransformMode.forward_z() {
        fps_rotation()
        roll_axis_y()
    }

    T.position(0.0, 0.5, 4.0) {
        C3D {}
        Pointer {}

        T.position(0.0, 1.65, -2.0).scale(0.055, 0.055, 1.0) {
            TXT {
                "click cubes to print\nwasd/rf/qe to move\nright-mouse drag to look"
                C.rgba(0.0, 0.0, 0.0, 1.0)
                TextureFiltering.linear()
            }
        }
    }
}

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
