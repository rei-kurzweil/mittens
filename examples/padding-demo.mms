// padding-demo.mms — visualize padding across nested block + inline-block
// boxes (=^･ω･^=)
//
// One LayoutRoot. Three side-by-side columns (inline-block) each holding
// the same three-row "cooking by the book" doc (block stacking with
// inline-block icon + text per row). Column padding increases left → right
// so you can see the padding box and content box at every nesting level.
//
// Background colors — high contrast per nesting level:
//   column outer : saturated blue
//   row          : hot magenta
//   text cell    : bright lime
//
// Z-stacking strategy: each nested T is bumped forward in local-z by a
// small Δz. The default `background_z = -0.1` then puts each level's bg
// quad just behind its TC's children but in front of the parent's bg.
// Authored z bumps:
//   column   z = 0.0  (LayoutRoot's children)
//   row      z = 0.2  (inside column TC)
//   icon/txt z = 0.2  (inside row TC)

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

T.position(-3.0, 2.0, 0.0).scale(0.10, 0.10, 0.10) {
    LayoutRoot {
        name = "padding_demo_root"
        available_width(80.0)
        available_height(40.0)

        // ── Column 1 — small padding (0.5 gu) ────────────────────────
        T {
            name = "col_a"
            Style {
                display("inline-block")
                width(20.0)
                padding(0.5)
                margin_right(1.0)
                background_color = [0.10, 0.20, 0.95, 1.0]
            }

            T.position(0, 0, 0.2) {
                name = "col_a_row1"
                Style {
                    height(2.5)
                    padding(0.4)
                    margin_bottom(0.4)
                    background_color = [0.92, 0.18, 0.55, 1.0]
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        width(2.0)
                        height(2.0)
                        margin_right(0.5)
                    }
                    R.circle2d() { C.rgba(1.0, 0.78, 0.10, 1.0) }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        width(15.0)
                        height(2.0)
                        background_color = [0.55, 0.95, 0.35, 1.0]
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
                    height(2.5)
                    padding(0.4)
                    margin_bottom(0.4)
                    background_color = [0.92, 0.18, 0.55, 1.0]
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        width(2.0)
                        height(2.0)
                        margin_right(0.5)
                    }
                    R.triangle() { C.rgba(1.0, 0.78, 0.10, 1.0) }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        width(15.0)
                        height(2.0)
                        background_color = [0.55, 0.95, 0.35, 1.0]
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
                    height(2.5)
                    padding(0.4)
                    background_color = [0.92, 0.18, 0.55, 1.0]
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        width(2.0)
                        height(2.0)
                        margin_right(0.5)
                    }
                    R.square() { C.rgba(1.0, 0.78, 0.10, 1.0) }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        width(15.0)
                        height(2.0)
                        background_color = [0.55, 0.95, 0.35, 1.0]
                    }
                    Text {
                        "a pretty cake"
                        C.rgba(0.05, 0.05, 0.05, 1.0)
                    }
                }
            }
        }

        // ── Column 2 — medium padding (1.5 gu) ───────────────────────
        T {
            name = "col_b"
            Style {
                display("inline-block")
                width(22.0)
                padding(1.5)
                margin_right(1.0)
                background_color = [0.10, 0.20, 0.95, 1.0]
            }

            T.position(0, 0, 0.2) {
                name = "col_b_row1"
                Style {
                    height(2.5)
                    padding(1.0)
                    margin_bottom(0.6)
                    background_color = [0.92, 0.18, 0.55, 1.0]
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        width(2.0)
                        height(2.0)
                        margin_right(0.5)
                    }
                    R.circle2d() { C.rgba(1.0, 0.78, 0.10, 1.0) }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        width(15.0)
                        height(2.0)
                        background_color = [0.55, 0.95, 0.35, 1.0]
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
                    height(2.5)
                    padding(1.0)
                    margin_bottom(0.6)
                    background_color = [0.92, 0.18, 0.55, 1.0]
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        width(2.0)
                        height(2.0)
                        margin_right(0.5)
                    }
                    R.triangle() { C.rgba(1.0, 0.78, 0.10, 1.0) }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        width(15.0)
                        height(2.0)
                        background_color = [0.55, 0.95, 0.35, 1.0]
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
                    height(2.5)
                    padding(1.0)
                    background_color = [0.92, 0.18, 0.55, 1.0]
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        width(2.0)
                        height(2.0)
                        margin_right(0.5)
                    }
                    R.square() { C.rgba(1.0, 0.78, 0.10, 1.0) }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        width(15.0)
                        height(2.0)
                        background_color = [0.55, 0.95, 0.35, 1.0]
                    }
                    Text {
                        "a pretty cake"
                        C.rgba(0.05, 0.05, 0.05, 1.0)
                    }
                }
            }
        }

        // ── Column 3 — generous padding (2.5 gu) ─────────────────────
        T {
            name = "col_c"
            Style {
                display("inline-block")
                width(24.0)
                padding(2.5)
                background_color = [0.10, 0.20, 0.95, 1.0]
            }

            T.position(0, 0, 0.2) {
                name = "col_c_row1"
                Style {
                    height(2.5)
                    padding(1.6)
                    margin_bottom(0.8)
                    background_color = [0.92, 0.18, 0.55, 1.0]
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        width(2.0)
                        height(2.0)
                        margin_right(0.5)
                    }
                    R.circle2d() { C.rgba(1.0, 0.78, 0.10, 1.0) }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        width(15.0)
                        height(2.0)
                        background_color = [0.55, 0.95, 0.35, 1.0]
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
                    height(2.5)
                    padding(1.6)
                    margin_bottom(0.8)
                    background_color = [0.92, 0.18, 0.55, 1.0]
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        width(2.0)
                        height(2.0)
                        margin_right(0.5)
                    }
                    R.triangle() { C.rgba(1.0, 0.78, 0.10, 1.0) }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        width(15.0)
                        height(2.0)
                        background_color = [0.55, 0.95, 0.35, 1.0]
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
                    height(2.5)
                    padding(1.6)
                    background_color = [0.92, 0.18, 0.55, 1.0]
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        width(2.0)
                        height(2.0)
                        margin_right(0.5)
                    }
                    R.square() { C.rgba(1.0, 0.78, 0.10, 1.0) }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        width(15.0)
                        height(2.0)
                        background_color = [0.55, 0.95, 0.35, 1.0]
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
