BGC {
    C.rgba(0.08, 0.08, 0.10, 1.0)
}

I {
    speed(1.0)
    InputTransformMode.forward_z() {
        roll_axis_y()
        fps_rotation()
    }

    T.position(0.0, 1.1, 3.8) {
        C3D {
            Pointer {}
        }
    }
}

T.position(-1.8, 2.3, 0.2).scale(0.09, 0.09, 0.09) {
    name="manual_scroll_demo"

    T.position(-1.8, 6.8, 0.02) {
        TXT {
            "ancestor renderable drag source"
            C.rgba(0.95, 0.95, 0.98, 1.0)
            Emissive.on()
        }
    }

    T.position(0.0, 0.0, 0.0) {
        R.square() {
            C.rgba(0.18, 0.32, 0.54, 0.95)
            Raycastable.enabled()

            StencilClip {
                T.position(-2.6, 3.4, 0.02) {
                    Scrolling.new(8.0, 28.0) {
                        name="manual_scroll"
                    }
                }
            }
        }
    }
}

T.position(1.8, 2.3, 0.2).scale(0.09, 0.09, 0.09) {
    name="layout_mock_demo"

    T.position(-1.0, 6.8, 0.02) {
        TXT {
            "sibling __bg drag source"
            C.rgba(0.95, 0.95, 0.98, 1.0)
            Emissive.on()
        }
    }

    StencilClip {}

    T {
        name="__bg"
        R.square() {
            C.rgba(0.54, 0.34, 0.16, 0.95)
            Raycastable.enabled()
        }
    }

    T.position(-2.6, 3.4, 0.02) {
        Scrolling.new(8.0, 28.0) {
            name="layout_scroll"
        }
    }
}

T.position(-2, 3, 2) {
    PL {
        intensity(2.0)
        distance(40.0)
        C.rgba(1.0, 1.0, 1.0, 1.0)
    }
}
AL {
    C.rgba(0.26, 0.26, 0.28, 1.0)
}