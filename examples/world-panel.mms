// world-panel.mms — import the reusable world-panel factory and use it in-scene.

BGC {
    C.rgba(0.18, 0.18, 0.20, 1.0)
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

let panel = world_panel("World", true)

T.position(-2.0, 3.8, 0.4).scale(0.1, 0.1, 0.1) {
    panel
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

T.position(-2.0, 3.0, 2.0) {
    PL {
        intensity(2.4)
        distance(60.0)
        C.rgba(1.0, 1.0, 1.0, 1.0)
    }
}

AL {
    C.rgba(0.30, 0.30, 0.30, 1.0)
}