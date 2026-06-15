I {
    speed(1.0)
    InputTransformMode.forward_z() {
        roll_axis_y()
        fps_rotation()
    }
    T.position(0.0, 1.1, 4.0) {
        C3D {
            Pointer {}
        }
    }
}

let meme_dimensions = [2.85188wu, 4wu]

let bg_quad = T.scale(meme_dimensions[0], meme_dimensions[1], 1.0) {
    R.square() {
        C.rgba(1.0, 1.0, 1.0, 1.0)
        Texture.with_uri("assets/images/expanding-brain.png")
        TextureFiltering.linear()
    }
}

let panel = T.position(-meme_dimensions[0] / 2.0, meme_dimensions[1] / 2.0, 0.0).scale(0.10, 0.10, 0.10) {
    LayoutRoot {
        name = "text_input_demo"
        available_width(meme_dimensions[0])
        available_height(meme_dimensions[1])
        T {
            name = "row_1"
            Style {
                height(25%)
                margin_bottom(0.8)
                padding(0.35)
                background_color = [0.04, 0.04, 0.06, 0.58]
            }

            T {
                name = "row_1_input"
                Style {
                    display("inline-block")
                    width(50%)
                    padding_xy(0.75, 0.65)
                    color = [0.07, 0.08, 0.10, 1.0]
                    font_size(2.0)
                    
                }
                TextInput {
                    "eating meat"
                }
            }
        }

        T {
            name = "row_2"
            Style {
                margin_bottom(0.8)
                padding(0.35)
                height(25%)
                background_color = [0.04, 0.04, 0.06, 0.58]
            }

            T {
                name = "row_2_input"
                Style {
                    display("inline-block")
                    width(50%)
                    padding_xy(0.75, 0.65)
                    color = [0.0, 0.0, 0.0, 1.0]
                    font_size(1.25)
                    
                }
                TextInput {
                    "being a vegetarian"
                }
            }
        }

        T {
            name = "row_3"
            Style {
                margin_bottom(0.8)
                padding(0.35)
                height(25%)
                background_color = [0.04, 0.04, 0.06, 0.58]
            }

            T {
                name = "row_3_input"
                Style {
                    display("inline-block")
                    width(50%)
                    padding_xy(0.75, 0.65)
                    color = [0.0, 0.0, 0.0, 1.0]
                    font_size(2.0)
                }
                TextInput {
                    "being vegan"
                }
            }
        }

        T {
            name = "row_4"
            Style {
                padding(0.35)
                height(25%)
                background_color = [0.04, 0.04, 0.06, 0.58]
            }

            T {
                name = "row_4_input"
                Style {
                    display("inline-block")
                    width(50%)
                    padding_xy(0.75, 0.65)
                    color = [0.0, 0.0, 0.0, 1.0]
                    font_size(1.5)
                }
                TextInput {
                    "letting animals eat you"
                }
            }
        }
    }
}


// meme anchor 
T.position(0.0, 2.0, -0.3) {
    bg_quad
    panel
}


T.position(0.0, 4.2, 1.6) {
    PL {
        intensity(3.0)
        distance(40.0)
        C.rgba(1.0, 1.0, 1.0, 1.0)
    }
}

AL {
    C.rgba(0.24, 0.24, 0.26, 1.0)
}

BGC {
    C.rgba(0.95, 0.95, 0.95, 1.0)
}

// ground plane
T.position(0.0, 0.0, 0.0).rotation(-1.5708, 0.0, 0.0).scale(400.0, 400.0, 0.01) {
    R.square() {
        C.rgba(0.85, 0.85, 0.85, 1.0)
        EM.on()
    }
}