// mms-tables.mms
//
// Sketch-only MMS example for future table support.
// This file is intended to show the target authoring shape before the parser
// and evaluator support the full table feature set end-to-end.
//
// Desired not-yet-implemented features exercised here:
//   1. anonymous table evaluation
//   2. table field reads: settings.color / settings.x_translation
//   3. table reassignment from click handlers
//   4. applying color fields from a table back onto live components
//
// Current runtime note:
// - `set_position(...)` exists today
// - table values do not evaluate yet
// - table field reads do not evaluate yet
// - color mutation methods are not wired yet

RendererSettings {
    window_size(1280, 900)
}

BGC.rgba(0.94, 0.95, 0.97, 1.0)
AL.rgb(0.22, 0.22, 0.24)

T.position(2.8, 4.8, 3.2) {
    DL {
        intensity(0.92)
        C.rgba(1.0, 0.98, 0.95, 1.0)
    }
}

T.position(-2.6, 2.6, 2.0) {
    DL {
        intensity(0.45)
        C.rgba(0.90, 0.94, 1.0, 1.0)
    }
}

I.speed(3.0) {
    InputTransformMode.forward_z() {
        fps_rotation()
        roll_axis_y()
    }

    T.position(0.0, 0.35, 4.4) {
        C3D {
            Pointer {}
        }
    }
}

BG {
    T.position(0, -10, 0).scale(10000, 1, 10000) {
        R.cube() { C.rgba(0.74, 0.76, 0.79, 1.0) }
    }
}

let left_preset = {
    label = "left monochrome"
    x_translation = -1.05
    color = [0.18, 0.88, 1.00, 0.72]
}

let center_preset = {
    label = "center monochrome"
    x_translation = 0.0
    color = [0.96, 0.34, 1.00, 0.72]
}

let right_preset = {
    label = "right monochrome"
    x_translation = 1.05
    color = [1.00, 0.78, 0.18, 0.72]
}

let current_settings = center_preset

let status_text = null
let preset_text = null
let gemstone_root = null
let gemstone_top_color = null
let gemstone_bottom_color = null

let left_button = T.position(-1.8, 0.95, -1.6).scale(0.32, 0.32, 0.32) {
    name = "left_button"

    T.scale(1.0, 0.22, 1.0) {
        R.cube() {
            C.rgba(0.12, 0.12, 0.12, 1.0)
            Raycastable.enabled()
        }
    }

    T.position(0.0, 0.0, 0.55).scale(0.62, 0.62, 0.62) {
        R.triangle() {
            C.rgba(1.0, 1.0, 1.0, 1.0)
        }
    }
}

let right_button = T.position(1.8, 0.95, -1.6).scale(0.32, 0.32, 0.32) {
    name = "right_button"

    T.scale(1.0, 0.22, 1.0) {
        R.cube() {
            C.rgba(1.0, 1.0, 1.0, 1.0)
            Raycastable.enabled()
        }
    }

    T.position(0.0, 0.0, 0.55).rotation(0.0, 0.0, 3.14159).scale(0.62, 0.62, 0.62) {
        R.triangle() {
            C.rgba(0.08, 0.08, 0.08, 1.0)
        }
    }
}

let gemstone = T.position(0.0, 0.95, -1.6).scale(0.72, 0.72, 0.72) {
    gemstone_root = T {}

    T.scale(1.35, 0.10, 1.35) {
        R.circle2d() {
            C.rgba(0.18, 0.18, 0.20, 0.18)
        }
    }

    T.position(0.0, 0.18, 0.0).rotation(-1.5708, 0.0, 0.0).scale(0.48, 0.72, 0.48) {
        R.cone() {
            gemstone_top_color = C.rgba(0.96, 0.34, 1.00, 0.72)
            EM.on()
            Raycastable.enabled()
        }
    }

    T.position(0.0, -0.18, 0.0).rotation(1.5708, 0.0, 0.0).scale(0.48, 0.72, 0.48) {
        R.cone() {
            gemstone_bottom_color = C.rgba(0.96, 0.34, 1.00, 0.48)
            EM.on()
        }
    }
}

T.position(-1.95, 2.15, -1.6).scale(0.055, 0.055, 1.0) {
    TXT {
        "preset"
        C.rgba(0.10, 0.10, 0.10, 1.0)
        TextureFiltering.linear()
    }
}

T.position(1.25, 2.15, -1.6).scale(0.055, 0.055, 1.0) {
    TXT {
        "preset"
        C.rgba(0.10, 0.10, 0.10, 1.0)
        TextureFiltering.linear()
    }
}

T.position(0.0, 2.05, -1.6).scale(0.070, 0.070, 1.0) {
    TXT {
        status_text = Text {
            "click to apply settings from table"
            C.rgba(0.04, 0.04, 0.05, 1.0)
            TextureFiltering.linear()
        }
    }
}

T.position(0.0, 1.65, -1.6).scale(0.050, 0.050, 1.0) {
    TXT {
        preset_text = Text {
            "current preset: center monochrome"
            C.rgba(0.22, 0.22, 0.24, 1.0)
            TextureFiltering.linear()
        }
    }
}

left_button
right_button
gemstone

on(left_button, "Click", fn(event) {
    current_settings = left_preset

    // Target shape once table field reads are live:
    // preset_text.set_text("current preset: " + current_settings.label)
    // status_text.set_text("preset staged from table: " + current_settings.label)

    preset_text.set_text("current preset: left monochrome")
    status_text.set_text("preset staged from table: left monochrome")
})

on(right_button, "Click", fn(event) {
    current_settings = right_preset

    // Target shape once table field reads are live:
    // preset_text.set_text("current preset: " + current_settings.label)
    // status_text.set_text("preset staged from table: " + current_settings.label)

    preset_text.set_text("current preset: right monochrome")
    status_text.set_text("preset staged from table: right monochrome")
})

on(gemstone, "Click", fn(event) {
    // Target shape once table reads + color mutation are live:
    //
    // gemstone_root.set_position(current_settings.x_translation, 0.95, -1.6)
    //
    // gemstone_top_color.set_rgba(
    //     current_settings.color[0],
    //     current_settings.color[1],
    //     current_settings.color[2],
    //     current_settings.color[3]
    // )
    //
    // gemstone_bottom_color.set_rgba(
    //     current_settings.color[0],
    //     current_settings.color[1],
    //     current_settings.color[2],
    //     0.48
    // )
    //
    // status_text.set_text("applied from table: " + current_settings.label)

    gemstone_root.set_position(0.0, 0.95, -1.6)
    status_text.set_text("click target wired; table apply pending field access")
})
