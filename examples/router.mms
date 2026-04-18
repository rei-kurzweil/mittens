BGC {
    C.rgba(0.95, 0.95, 0.98, 1.0)
}

I {
    speed(1.0)
    InputTransformMode.forward_z() {
        roll_axis_y()
        fps_rotation()
    }

    T.position(0.0, 1.1, 3.4) {
        C3D {
            Pointer {}
        }
    }
}

T.position(-1.35, 0.75, 2.4).scale(0.08, 0.08, 0.08) {
    LayoutRoot {
        name = "router_demo"
        available_width(34.0)
        available_height(20.0)

        Router {
            target = "container"
            ignore = ["toolbar", "status"]
        }

        T {
            name = "toolbar"
            Style {
                height(3.0)
                margin(0.5)
                padding_xy(1.0, 0.75)
                background_color = [0.22, 0.25, 0.34, 1.0]
            }
            Text {
                "toolbar"
                C.rgba(1.0, 1.0, 1.0, 1.0)
            }
        }

        T {
            name = "container"
            Style {
                height(11.0)
                margin(0.5)
                padding(0.75)
                overflow("scroll")
                background_color = [0.97, 0.97, 1.0, 1.0]
            }
        }

        T {
            name = "status"
            Style {
                height(2.5)
                margin(0.5)
                padding_xy(1.0, 0.5)
                background_color = [0.84, 0.87, 0.93, 1.0]
            }
            Text {
                "status: router demo"
                C.rgba(0.16, 0.18, 0.24, 1.0)
            }
        }

        T {
            name = "authored_child"
            Style {
                margin(0.5)
                padding_xy(0.75, 0.5)
                background_color = [0.90, 0.93, 1.0, 1.0]
            }
            Text {
                "authored child routed at init"
                C.rgba(0.14, 0.24, 0.56, 1.0)
            }
        }
    }
}

AL {
    C.rgba(0.18, 0.18, 0.18, 1.0)
}