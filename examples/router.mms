BGC {
    C.rgba(0.5, 0.5, 0.5, 1.0)
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

// button
let button_surface = T.position(0,0,0) {
    name="button_surface"
    Transition {
        duration_beats(0.25)
        ease_in_out_sine()
    }
    T.position(0, 0.1, 0).scale(0.9, 0.1, 0.9) {
        R.cube() {
            C.rgba(1.0, 0.1, 0.1, 1.0)
            Emissive.on()
        }
    }
    T.position(-0.2, 0.2, 0)
        .rotation(-3.141/2.0, 0, 0).scale(0.2, 0.2, 0.2) {
        TXT {
            "add"
            C.rgba(1.0, 1.0, 1.0, 1.0)
            TextureFiltering.linear()
            Emissive.on()
        }   
    }
}

let button_root = T.position(0, -0.2, 1.5) {
    name="button"
    Raycastable.enabled()

    T.position(0, 0, 0).scale(1.0, 0.2, 1.0) {
        R.cube() {
            C.rgba(0.9, 0.9, 0.9, 1.0)
        }
    }
    button_surface
}
button_root

// button animation
Animation.looping() {
    name="button_animation"
    
    Keyframe.at(0) {
        button_surface.update_transform(
            [0,0,0], [0,0,0], [1,1,1]) 
    }
    Keyframe.at(0.25) {
        button_surface.update_transform(
            [0, -0.04,0], [0,0,0], [1,1,1])   
    }
}

T.position(-1.35, 2.0, 0.4).scale(0.08, 0.08, 0.08) {
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
                background_color = [1.0, 1.0, 1.0, 1.0]
            }
            Text {
                "toolbar"
                C.rgba(0.0, 0.0, 0.0, 1.0)
            }
        }

        T {
            name = "container"
            Style {
                height(11.0)
                margin(0.5)
                padding(0.75)
                overflow("scroll")
                background_color = [0.9, 0.9, 0.9, 1.0]
            }
        }

        T {
            name = "status"
            Style {
                height(2.5)
                margin(0.5)
                padding_xy(1.0, 0.5)
                background_color = [0.8, 0.8, 0.8, 1.0]
            }
            Text {
                "press 'add' to continue"
                C.rgba(0, 0, 0, 1.0)
            }
        }

        T {
            name = "authored_child"
            Style {
                margin(0.5)
                padding_xy(1.5, 0.5)
                background_color = [0.90, 0.93, 1.0, 1.0]
            }
            T.position(0,0,0.1) {
                Text {
                    "authored child routed at init"
                    C.rgba(0.14, 0.24, 1.0, 1.0)
                }
            }
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
    C.rgba(0.28, 0.28, 0.28, 1.0)
}
