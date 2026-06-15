// percentage-layout.mms — sidebar + content using % unit literals (=^･ω･^=)
//
// One LayoutRoot, 80 gu wide. Two inline-block siblings:
//   - sidebar     : width 25%
//   - content     : width 75%, holds a stack of cards
// Cards inside the content area use 100% width and 5% padding to show that
// percent padding/margin resolves against the *inline-axis* container width
// (CSS semantic) — even on the top/bottom sides.

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

let panel_bg   = [0.96, 0.96, 0.98, 1.0]
let sidebar_bg = [0.30, 0.40, 0.70, 1.0]
let content_bg = [1.00, 1.00, 1.00, 1.0]
let card_bg    = [0.95, 0.88, 0.92, 1.0]
let text_color = [0.05, 0.05, 0.05, 1.0]

let panel = T.position(-4.0, 2.0, 0.0).scale(0.10, 0.10, 0.10) {
    LayoutRoot {
        name = "percentage_layout_root"
        available_width(80.0)
        available_height(40.0)

        // ── Sidebar — 25% of available width ─────────────────────────
        T {
            name = "sidebar"
            Style {
                display("inline-block")
                width(25%)
                padding(2%)
                background_color = sidebar_bg
                color = [0.95, 0.96, 1.0, 1.0]
            }
            T.position(0, 0, 0.2) {
                Style {
                }
                Text { "sidebar nav" }
            }
        }

        // ── Content — 75% of available width, holds cards ────────────
        T {
            name = "content"
            Style {
                display("inline-block")
                width(75%)
                padding(2%)
                background_color = content_bg
                color = text_color
            }

            // Card 1
            T.position(0, 0, 0.2) {
                Style {
                    width(100%)
                    padding(5%)
                    margin(1%)
                    background_color = card_bg
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                    }
                    Text { "first card — percent padding scales with the container" }
                }
            }

            // Card 2
            T.position(0, 0, 0.2) {
                Style {
                    width(100%)
                    padding(5%)
                    margin(1%)
                    background_color = card_bg
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                    }
                    Text { "second card — try shrinking the panel width" }
                }
            }

            // Card 3
            T.position(0, 0, 0.2) {
                Style {
                    width(100%)
                    padding(5%)
                    margin(1%)
                    background_color = card_bg
                }
                T.position(0, 0, 0.2) {
                    Style {
                        display("inline-block")
                    }
                    Text { "third card — vertical padding resolves against width too" }
                }
            }
        }
    }
}

panel
let layout = panel.query("#percentage_layout_root")

// ── Width controls ──────────────────────────────────────────────────
import { button } from "../assets/components/button.mms"

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

let control_panel = T.position(-4.0, 3.0, 0.0).scale(0.30, 0.30, 0.30) {
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

on(shrink_btn, "Click", fn(e) {
    let w = layout.available_width()
    let new_w = w - 4.0
    if new_w < 20.0 { new_w = 20.0 }
    layout.set_available_width(new_w)
    label_text.set_text("" + new_w)
})
on(grow_btn, "Click", fn(e) {
    let w = layout.available_width()
    let new_w = w + 4.0
    if new_w > 120.0 { new_w = 120.0 }
    layout.set_available_width(new_w)
    label_text.set_text("" + new_w)
})

// ── Lighting ────────────────────────────────────────────────────────
AL { C.rgba(0.32, 0.32, 0.34, 1.0) }
T.position(2.0, 3.0, 2.0) {
    DL {
        intensity(0.85)
        C.rgba(1.0, 0.96, 0.92, 1.0)
    }
}

BGC { C.rgba(0.15, 0.15, 0.15, 1.0) }
