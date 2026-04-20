BGC {
    C.rgba(0.5, 0.5, 0.5, 1.0)
}

let stencil_clip_debug_texture = Texture.render_image("render_graph.stencil_clip.debug")

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

        T.position(0.0, 0.0, -3.0).scale(1.4, 1.0, 1.0) {
            OV {
                R.square() {
                    C.rgba(0.98, 0.98, 0.98, 1.0)
                }

                T.position(0.0, 0.0, 0.01).scale(0.86, 0.66, 1.0) {
                    R.square() {
                        C.rgba(1.0, 1.0, 1.0, 1.0)
                        stencil_clip_debug_texture
                        TextureFiltering.linear()
                    }
                }

                T.position(-0.4, -0.42, 0.02).scale(0.05, 0.05, 1.0) {
                    TXT {
                        "s-buffer debug"
                        C.rgba(0.08, 0.08, 0.10, 1.0)
                    }
                }
            }
        }
    }
}

T.position(-2.0, 4.0, 0.4).scale(0.1, 0.1, 0.1) {
    LayoutRoot {
        name = "diy_panel_demo"
        available_width(40.0)
        available_height(40.0)

        Router {
            target = "container"
            ignore = ["toolbar", "status"]
        }

        T {
            name = "toolbar"
            Style {
                height(4.0)
                margin(0.5)
                padding_xy(1.0, 0.75)
                background_color = [0.18, 0.78, 0.22, 0.95]
            }
            Text {
                "DIY panel header (4w x 8h world units)"
                C.rgba(0.0, 0.0, 0.0, 1.0)
            }
        }

        T {
            name = "container"
            Style {
                height(20.0)
                margin(0.5)
                padding(0.75)
                overflow("scroll")
                background_color = [0.96, 0.92, 0.18, 0.80]
            }
        }

        T {
            name = "status"
            Style {
                height(4.0)
                margin(0.5)
                padding_xy(1.0, 0.5)
                background_color = [0.85, 0.85, 0.85, 1.0]
            }
            Text {
                "rows are routed into the yellow scroll container"
                C.rgba(0, 0, 0, 1.0)
            }
        }

        for i in range(10) {
            T.position(0,0,0.2) {
                Style {
                    margin_xy(0.25, 0.25)
                    padding_xy(0.5, 0.5)
                    height(2.5)
                    background_color = [0.94, 0.94, 0.94, 1.0]
                }
                T.position(0, 0, 0.2) {
                    Text {
                        "root row " + i
                        C.rgba(0.0, 0.0, 0.0, 1.0)
                    }
                }
            }
        }


        T {
            name = "authored_child"
            Style {
                margin_xy(0.25, 0.25)
                padding_xy(0.5, 2.25)
                background_color = [1.0, 0.85, 0.85, 1.0]
            }
            T.position(0, 0, 0.1) {
                Text {
                    "authored child routed at init"
                    C.rgba(0.35, 0.0, 0.0, 1.0)
                }
            }
        }
    }
}

T.position(-2, 3, 2) {
    PL {
        intensity(2.5)
        distance(70.0)
        C.rgba(1.0, 1.0, 1.0, 1.0)
    }
}
AL {
    C.rgba(0.28, 0.28, 0.28, 1.0)
}
