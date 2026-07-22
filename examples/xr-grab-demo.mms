// XR grip-ray grabbing demo. Aim either controller ray at a colored object,
// squeeze grip, move the controller, and release. Trigger remains available
// for ordinary pointer interaction and does not grab.

RendererSettings { window_size(960, 640) }
BGC.rgba(0.035, 0.045, 0.085, 1.0)
AL.rgb(0.30, 0.32, 0.40)

T.position(3.0, 5.0, 2.0) {
    DL { intensity(1.2) color(1.0, 0.90, 0.78) }
}

// Desktop fallback camera.
T.position(0.0, 1.8, 6.0) {
    C3D { Pointer {} }
}

// Tracked headset and grip-pose pointer rays.
InputXR.on() {
    T.position(0.0, 1.6, 0.0) {
        CXR {}
        XRHand.new(true, Left, Grip) { T { Pointer {} } }
        XRHand.new(true, Right, Grip) { T { Pointer {} } }
    }
}
XR.on()

T.position(0.0, -0.10, 0.0).scale(10.0, 0.20, 10.0) {
    R.cube() { C.rgba(0.10, 0.14, 0.22, 1.0) }
}

T.position(-1.4, 1.1, -1.8) {
    name = "grab_red_outer"
    Grabbable {}
    T.rotation(0.0, 0.3, 0.0) {
        T.scale(0.75, 0.75, 0.75) {
            R.cube() { C.rgba(0.95, 0.22, 0.25, 1.0) EM.on() }
        }
    }
}

T.position(0.0, 1.2, -2.2) {
    name = "grab_nested_outer"
    Grabbable {}
    T.position(0.0, 0.25, 0.0) {
        T.scale(0.55, 0.55, 0.55) {
            R.sphere() { C.rgba(0.20, 0.78, 1.0, 1.0) EM.on() }
        }
        T.position(0.0, -0.65, 0.0).scale(0.22, 0.75, 0.22) {
            R.cube() { C.rgba(0.15, 0.50, 0.95, 1.0) }
        }
    }
}

T.position(1.5, 1.0, -1.7) {
    name = "grab_gold_outer"
    Grabbable {}
    T.scale(0.70, 0.70, 0.70) {
        R.cone() { C.rgba(1.0, 0.72, 0.12, 1.0) EM.on() }
    }
}

T.position(-2.8, 2.7, -2.5).scale(0.018, 0.018, 0.018) {
    Text { "Aim grip-pose rays • squeeze GRIP • move • release" }
}
