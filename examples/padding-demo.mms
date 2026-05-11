// padding-demo.mms — visualize padding across nested block + inline-block
// boxes (=^･ω･^=)
//
// Three side-by-side columns (inline-block at the LayoutRoot level), each
// with the same three-row "cooking by the book" doc inside (block-flow rows
// of inline-block icon + text). Uniform padding & margin on every styled
// box; padding is amplified column-by-column so the box model is obvious:
//
//   col_a : padding 0.4 gu (everywhere)   margin 0.3
//   col_b : padding 0.8 gu (everywhere)   margin 0.3
//   col_c : padding 1.2 gu (everywhere)   margin 0.3
//
// Margin is the same on all three so column tops align; columns get taller
// downward as their padding grows.
//
// Background colors per nesting level:
//   column outer : saturated blue
//   row          : hot magenta
//   text cell    : bright lime
// Each nested T uses `T.position(0,0,0.2)` to bump local-z forward so the
// default `background_z = -0.1` keeps each level's bg behind its own
// children but in front of its parent's bg.

I {
    speed(1.0)
    InputTransformMode.forward_z() {
        roll_axis_y()
        fps_rotation()
    }
    T.position(0.0, 1.0, 3.0) {
        C3D {}
        Pointer {}
    }
}

let text_bg_color = [0.7, 0.7, 0.7, 1.0]
let item_bg_color = [1.0, 0.7, 0.8, 1.0]
let container_bg_color = [1.0, 1.0, 1.0,1.0]

let icon_color = C.rgba(1.0, 0.1, 0.4, 1.0)
let icon_color_2 = C.rgba(1.0, 0.7, 0.2, 1.0)
let icon_background_color = [1, 0.2, 0.3, 1]


T.position(-3.0, 2.0, 0.0).scale(0.10, 0.10, 0.10) {
    LayoutRoot {
        name = "padding_demo_root"
        available_width(80.0)
        available_height(40.0)

        // ── Column 1 — padding 0.4 ───────────────────────────────────
        T {
            name = "col_a"
            Style {
                display("inline-block")
                width(20.0)
                padding(0.4)
                margin(0.3)
                background_color = container_bg_color
            }

            T.position(0, 0, 0.2) {
                name = "col_a_row1"
                Style {
                    padding(0.4)
                    margin(0.3)
                    background_color = item_bg_color
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(0.4)
                        margin(0.3)
                    }
                    R.circle2d() { icon_color }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(0.4)
                        margin(0.3)
                        background_color = text_bg_color
                    }
                    Text {
                        "it's a piece of cake"
                        C.rgba(0.05, 0.05, 0.05, 1.0)
                    }
                }
            }

            T.position(0, 0, 0.2) {
                name = "col_a_row2"
                Style {
                    padding(0.4)
                    margin(0.3)
                    background_color = item_bg_color
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(0.4)
                        margin(0.3)
                    }
                    R.triangle() { icon_color }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(0.4)
                        margin(0.3)
                        background_color = text_bg_color
                    }
                    Text {
                        "to bake"
                        C.rgba(0.05, 0.05, 0.05, 1.0)
                    }
                }
            }

            T.position(0, 0, 0.2) {
                name = "col_a_row3"
                Style {
                    padding(0.4)
                    margin(0.3)
                    background_color = item_bg_color
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(0.4)
                        margin(0.3)
                    }
                    R.square() { icon_color }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(0.4)
                        margin(0.3)
                        background_color = text_bg_color
                    }
                    Text {
                        "a pretty cake"
                        C.rgba(0.05, 0.05, 0.05, 1.0)
                    }
                }
            }
        }

        // ── Column 2 — padding 0.8 ───────────────────────────────────
        T {
            name = "col_b"
            Style {
                display("inline-block")
                width(22.0)
                padding(0.8)
                margin(0.3)
                background_color = container_bg_color
            }

            T.position(0, 0, 0.2) {
                name = "col_b_row1"
                Style {
                    padding(0.8)
                    margin(0.3)
                    background_color = item_bg_color
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(0.8)
                        margin(0.3)
                    }
                    R.circle2d() { icon_color }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(0.8)
                        margin(0.3)
                        background_color = text_bg_color
                    }
                    Text {
                        "it's a piece of cake"
                        C.rgba(0.05, 0.05, 0.05, 1.0)
                    }
                }
            }

            T.position(0, 0, 0.2) {
                name = "col_b_row2"
                Style {
                    padding(0.8)
                    margin(0.3)
                    background_color = item_bg_color
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(0.8)
                        margin(0.3)
                    }
                    R.triangle() { icon_color }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(0.8)
                        margin(0.3)
                        background_color = text_bg_color
                    }
                    Text {
                        "to bake"
                        C.rgba(0.05, 0.05, 0.05, 1.0)
                    }
                }
            }

            T.position(0, 0, 0.2) {
                name = "col_b_row3"
                Style {
                    padding(0.8)
                    margin(0.3)
                    background_color = item_bg_color
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(0.8)
                        margin(0.3)
                    }
                    R.square() { icon_color }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(0.8)
                        margin(0.3)
                        background_color = text_bg_color
                    }
                    Text {
                        "a pretty cake"
                        C.rgba(0.05, 0.05, 0.05, 1.0)
                    }
                }
            }
        }

        // ── Column 3 — padding 1.2 ───────────────────────────────────
        T {
            name = "col_c"
            Style {
                display("inline-block")
                width(24.0)
                padding(1.2)
                margin(0.3)
                background_color = container_bg_color
            }

            T.position(0, 0, 0.2) {
                name = "col_c_row1"
                Style {
                    padding(1.2)
                    margin(0.3)
                    background_color = item_bg_color
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(1.2)
                        margin(0.3)
                        background_color = icon_background_color
                    }
                    R.circle2d() { icon_color_2 }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(1.2)
                        margin(0.3)
                        background_color = text_bg_color
                    }
                    Text {
                        "it's a piece of cake"
                        C.rgba(0.05, 0.05, 0.05, 1.0)
                    }
                }
            }

            T.position(0, 0, 0.2) {
                name = "col_c_row2"
                Style {
                    padding(1.2)
                    margin(0.3)
                    background_color = item_bg_color
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(1.2)
                        margin(0.3)
                        background_color = icon_background_color
                    }
                    R.triangle() { icon_color_2 }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(1.2)
                        margin(0.3)
                        background_color = text_bg_color
                    }
                    Text {
                        "to bake"
                        C.rgba(0.05, 0.05, 0.05, 1.0)
                    }
                }
            }

            T.position(0, 0, 0.2) {
                name = "col_c_row3"
                Style {
                    padding(1.2)
                    margin(0.3)
                    background_color = item_bg_color
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(1.2)
                        margin(0.3)
                        background_color = icon_background_color
                    }
                    R.square() { icon_color }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(1.2)
                        margin(0.3)
                        background_color = text_bg_color
                    }
                    Text {
                        "a pretty cake"
                        C.rgba(0.05, 0.05, 0.05, 1.0)
                    }
                }
            }
        }
    }
}

// ── Lighting ────────────────────────────────────────────────────────
AL {
    C.rgba(0.32, 0.32, 0.34, 1.0)
}
T.position(2.0, 3.0, 2.0) {
    DL {
        intensity(0.85)
        C.rgba(1.0, 0.96, 0.92, 1.0)
    }
}

BGC {
    C.rgba(0.9, 0.9, 0.9, 1.0)
}