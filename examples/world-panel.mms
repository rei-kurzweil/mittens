// world-panel.mms — import the reusable world-panel factory and use it in-scene.

BGC {
    C.rgba(0.25, 0.25, 0.25, 1.0)
}

I {
    speed(1.0)
    InputTransformMode.forward_z() {
        roll_axis_y()
        fps_rotation()
    }

    T.position(0.0, 1.1, 3.6) {
        C3D {
            Pointer {}
        }
    }
}

import { world_panel } from "../assets/components/world-panel.mms"

let TEXT_SCALE = 0.08
let WORLD_PANEL_WIDTH_GU = 29.5
let WORLD_PANEL_TOTAL_HEIGHT_GU = 57.5
let EDITOR_LAYOUT_WIDTH_GU = 3.0 * WORLD_PANEL_WIDTH_GU + 6.0

let items = [
    "root child routed into rows_mount",
    "save/load handlers can query named descendants",
    "rows_mount is the intended Rust injection target"
]

let panel = world_panel("World", items)

Selectable.off() {
    T.position(-0.7, 1.6, -1.2) {
        Overlay {
            LayoutRoot {
                name = "example_editor_layout_root"
                available_width(EDITOR_LAYOUT_WIDTH_GU)
                unit_scale(TEXT_SCALE)

                T {
                    name = "example_world_panel_shell"
                    Style {
                        display("inline-block")
                        width(WORLD_PANEL_WIDTH_GU)
                        height(WORLD_PANEL_TOTAL_HEIGHT_GU)
                        margin_xy(0.5, 0.5)
                    }

                    panel
                }
            }
        }
    }
}

let save_btn = panel.query("#save_button")
let load_btn = panel.query("#load_button")
let status_text = panel.query("#panel_status_value")
let rows_mount = panel.query("#rows_mount")

if rows_mount {
    status_text.set_text("rows_mount ready")
}

on(save_btn, "Click", fn(e) {
    status_text.set_text("save_button clicked")
})

on(load_btn, "Click", fn(e) {
    status_text.set_text("load_button clicked")
})

AL {
    C.rgba(0.22, 0.22, 0.22, 1.0)
}

T.position(-1.6, 2.8, 2.2) {
    DL {
        intensity(0.95)
        color(1.0, 1.0, 1.0)
    }
}

T.position(1.8, 1.4, 2.8) {
    DL {
        intensity(0.35)
        color(1.0, 1.0, 1.0)
    }
}
