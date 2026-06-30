// pride scene
// Corresponds to examples/pride.rs

RendererSettings {
    window_size(1280, 960)
}

BGC.rgba(0.95, 0.96, 0.99, 1.0)
AL.rgb(0.55, 0.55, 0.60)

T.position(0.0, 3.5, 3.5) {
    PL {
        intensity(5.0)
        distance(20.0)
        color(1.0, 0.98, 0.95)
    }
}

I.speed(1.5) {
    InputTransformMode.forward_z() {
        fps_rotation()
        roll_axis_y()
    }
    T.position(0.0, 0.0, 6.8) {
        C3D {
            Pointer {}
        }
    }
}

T.position(-0.15, 2.35, 0.2).scale(0.075, 0.075, 1.0) {
    TXT {
        "partial_annulus_2d()\n5 concentric quarter-rings"
        C.rgba(0.08, 0.08, 0.10, 1.0)
        EM.on()
        TextureFiltering.linear()
    }
}

T.position(2.55, -2.1, -4.1).scale(0.05, 0.05, 1.0) {
    TXT {
        "mesh geometry from Rust\nscene shell from MMS"
        C.rgba(0.12, 0.12, 0.15, 1.0)
        EM.on()
        TextureFiltering.linear()
    }
}
