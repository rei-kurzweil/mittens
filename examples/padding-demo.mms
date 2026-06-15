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
        C3D {
            Pointer {}
        }
    }
}

let text_bg_color = [0.9, 0.9, 0.9, 1.0]
let item_bg_color = [1.0, 0.6, 0.7, 1.0]
let container_bg_color = [1.0, 0.7, 0.8, 1.0]

let icon_color = C.rgba(1.0, 0.1, 0.4, 1.0)
let icon_color_2 = C.rgba(1.0, 0.7, 0.2, 1.0)
let icon_background_color = [1, 0.2, 0.3, 1]


import { button } from "../assets/components/button.mms"

let panel = T.position(-3.0, 2.0, 0.0).scale(0.10, 0.10, 0.10) {
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
                    R.circle2d() { EM.on() icon_color }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(0.4)
                        margin(0.3)
                        word_wrap("normal")
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
                    R.triangle() { EM.on() icon_color }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(0.4)
                        margin(0.3)
                        word_wrap("normal")
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
                    R.square() { EM.on() icon_color }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(0.4)
                        margin(0.3)
                        word_wrap("normal")
                        background_color = text_bg_color
                    }
                    Text {
                        "a pretty cake (WHAT?)"
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
                        text_align("center")
                        vertical_align("middle")
                    }
                    R.circle2d() { EM.on() icon_color }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(0.8)
                        margin(0.3)
                        word_wrap("normal")
                        background_color = text_bg_color
                    }
                    Text {
                        "if the way is hazy"
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
                    R.triangle() { EM.on() icon_color }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(0.8)
                        margin(0.3)
                        word_wrap("normal")
                        background_color = text_bg_color
                    }
                    Text {
                        "you gotta do your cooking by the book"
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
                        text_align("center")
                        vertical_align("middle")
                    }
                    R.square() { EM.on() icon_color }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(0.8)
                        margin(0.3)
                        word_wrap("normal")
                        background_color = text_bg_color
                    }
                    Text {
                        "you know you can't be lazy"
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
                        text_align("center")
                        vertical_align("middle")
                    }
                    R.circle2d() { icon_color_2 }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(1.2)
                        margin(0.3)
                        word_wrap("normal")
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
                        text_align("center")
                        vertical_align("middle")
                    }
                    R.triangle() { icon_color_2 }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(1.2)
                        margin(0.3)
                        word_wrap("normal")
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
                        text_align("center")
                        vertical_align("middle")
                    }
                    R.square() { EM.on() icon_color }
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                        padding(1.2)
                        margin(0.3)
                        word_wrap("normal")
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

panel
let layout = panel.query("#padding_demo_root")

// ── Width controls ──────────────────────────────────────────────────
// A small inline-block control bar (separate LayoutRoot) sitting above
// the demo panel. Contains: [shrink_btn]  [width readout]  [grow_btn].
let shrink_btn = button("-")
let grow_btn   = button("+")

let width_label = T {
    name = "width_label_wrap"
    Style {
        display("inline-block")
        padding_xy(0.6, 0.6)
        text_align("center")
    }
    T.position(0.0, 0.0, 0.05) {
        Text {
            name = "width_value"
            "80"
        }
    }
}
let label_text = width_label.query("#width_value")

let control_panel = T.position(-3.0, 6.0, 0.0).scale(0.10, 0.10, 0.10) {
    LayoutRoot {
        name = "control_root"
        available_width(40.0)
        available_height(8.0)
        shrink_btn
        width_label
        grow_btn
    }
}
control_panel

// ±4 gu per click, clamped to [40, 120].
on(shrink_btn, "Click", fn(e) {
    let w = layout.available_width()
    let new_w = w - 4.0

    if new_w < 40.0 {
        new_w = 40.0
    }
    layout.set_available_width(new_w)
    label_text.set_text("" + new_w)
})
on(grow_btn, "Click", fn(e) {
    let w = layout.available_width()
    let new_w = w + 4.0
    
    if new_w > 120.0 {
        new_w = 120.0
    }
    layout.set_available_width(new_w)
    label_text.set_text("" + new_w)
})

// ── Lighting ────────────────────────────────────────────────────────
AL {
    C.rgba(0.22, 0.22, 0.22, 1.0)
}
T.position(2.0, 3.0, 22.0) {
    DL {
        intensity(0.85)
        C.rgba(1.0, 1.0, 1.0, 1.0)
    }
}

BGC {
    C.rgba(0.6, 0.8, 1.0, 1.0)
}


// ground plane
T.position(0.0, -1.3, 0.0).rotation(-1.5708, 0.0, 0.0).scale(400.0, 400.0, 1.0) {
    R.plane() {
        C.rgba(0.7, 1.0, 0.3, 1.0)
    }
}

T.position(0, 25, 0) {
        PL {
            intensity(0.9)
            distance(250.0)
            C.rgba(1.0, 1.0, 1.0, 1.0)
        }
}