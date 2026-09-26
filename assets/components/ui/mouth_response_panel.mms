// Compact, live AVC mouth-response controls. The caller owns the panel's world
// transform; this factory owns the accordion restore and recreated sliders.
import { info_panel, info_panel_body } from "./info_panel.mms"

fn fixed_3(value) {
    let scaled = Math.round(Math.abs(value) * 1000.0)
    let whole = Math.floor(scaled / 1000.0)
    let fraction = scaled - whole * 1000.0
    let padding = ""
    if fraction < 10.0 { padding = "00" } else if fraction < 100.0 { padding = "0" }
    let sign = ""
    if value > 0.0 { sign = "+" } else if value < 0.0 { sign = "-" }
    return sign + whole + "." + padding + fraction
}

let ROW_HEIGHT = 3.0
let LABEL_WIDTH = 10.0
let SLIDER_SLOT_WIDTH = 13.0
let READOUT_WIDTH = 6.0
let CONTROL_FONT_SIZE = 0.8
let SLIDER_WIDTH = 13.0

fn response_slider(slider_name, initial, minimum, maximum, step) {
    // Slider.width and the visible cube use the same local width. The panel's
    // unit scale converts both to world space after layout placement.
    let track = T.scale(SLIDER_WIDTH * 0.5, 0.025, 0.10) {
        R.cube() {
            C.rgba(0.24, 0.45, 0.65, 1.0)
            Raycastable.enabled() { interaction_priority(120.0) }
        }
    }
    let thumb = T.scale(0.1125, 0.225, 0.30) {
        R.sphere() {
            C.rgba(1.0, 0.46, 0.64, 1.0)
            Raycastable.enabled() { interaction_priority(120.0) }
        }
    }
    return Slider.range(minimum, maximum).step(step).value(initial).width(SLIDER_WIDTH)
        .track(track).thumb(thumb) { name = slider_name }
}

fn response_row(text, slider, readout) {
    return T {
        Style {
            display("flex") flex_direction("row") width(100%) height(ROW_HEIGHT)
            align_items("center") gap(0.5)
            background_color([0.055, 0.075, 0.10, 0.94]) background_z(-0.02)
        }
        T {
            Style {
                display("flex") width(LABEL_WIDTH) height(ROW_HEIGHT)
                align_items("center") padding_xy(0.25, 0.0) font_size(CONTROL_FONT_SIZE)
            }
            T.position(0.0, 0.0, 0.03) { Text { text } }
        }
        T {
            Style {
                display("flex") width(SLIDER_SLOT_WIDTH) height(ROW_HEIGHT)
                flex_grow(1.0) align_items("center") justify_content("center")
            }
            slider
        }
        T {
            Style {
                display("flex") width(READOUT_WIDTH) height(ROW_HEIGHT)
                align_items("center") justify_content("center") font_size(CONTROL_FONT_SIZE)
                color([0.76, 0.92, 1.0, 1.0])
            }
            T.position(0.0, 0.0, 0.03) { readout }
        }
    }
}

fn apply_tuning(avatar_slot, tuning) {
    let avatar = avatar_slot.avatar
    if avatar {
        avatar.set_mouth_open_rms_center_range(tuning.center_rms, tuning.range_rms)
        avatar.set_mouth_open_amount(tuning.amount)
    }
}

fn response_content(avatar_slot, tuning, description) {
    let center_slider = response_slider("mouth_rms_center_slider", tuning.center_rms, 0.001, 0.120, 0.001)
    let center_readout = Text { name = "mouth_rms_center_readout" fixed_3(tuning.center_rms) + " RMS" }
    let range_slider = response_slider("mouth_rms_range_slider", tuning.range_rms, 0.001, 0.120, 0.001)
    let range_readout = Text { name = "mouth_rms_range_readout" fixed_3(tuning.range_rms) + " RMS" }
    let amount_slider = response_slider("mouth_amount_slider", tuning.amount, 0.0, 1.0, 0.01)
    let amount_readout = Text { name = "mouth_amount_readout" fixed_3(tuning.amount) }

    on(center_slider, "SliderChanged", fn(event) {
        tuning.center_rms = event.value
        if tuning.range_rms * 0.5 > tuning.center_rms {
            tuning.range_rms = tuning.center_rms * 2.0
            range_slider.sync_value(tuning.range_rms)
            range_readout.set_text(fixed_3(tuning.range_rms) + " RMS")
        }
        apply_tuning(avatar_slot, tuning)
        center_readout.set_text(fixed_3(tuning.center_rms) + " RMS")
    })
    on(range_slider, "SliderChanged", fn(event) {
        tuning.range_rms = event.value
        if tuning.range_rms * 0.5 > tuning.center_rms {
            tuning.range_rms = tuning.center_rms * 2.0
            range_slider.sync_value(tuning.range_rms)
        }
        apply_tuning(avatar_slot, tuning)
        range_readout.set_text(fixed_3(tuning.range_rms) + " RMS")
    })
    on(amount_slider, "SliderChanged", fn(event) {
        tuning.amount = event.value
        apply_tuning(avatar_slot, tuning)
        amount_readout.set_text(fixed_3(tuning.amount))
    })

    return T {
        name = "mouth_response_settings_content"
        Style { display("flex") flex_direction("column") width(100%) row_gap(0.20) }
        T {
            Style { display("block") width(100%) font_size(0.60) color([0.78, 0.84, 0.92, 1.0]) }
            Text { description }
        }
        response_row("RMS centre", center_slider, center_readout)
        response_row("RMS range", range_slider, range_readout)
        response_row("mouth amount", amount_slider, amount_readout)
    }
}

// Required: root_name, title, avatar_slot, tuning, description.
// The caller sets avatar_slot.avatar after authoring its AVC later in the scene.
export fn mouth_response_panel(options) {
    let panel = info_panel({
        root_name = options.root_name
        width_gu = 32.0
        unit_scale = 0.08
        title = options.title
        background_color = [0.10, 0.20, 0.28, 0.98]
        toggle_background_color = [0.16, 0.38, 0.54, 1.0]
        content = response_content(options.avatar_slot, options.tuning, options.description)
    })
    let body_mount = panel.query("#accordion_body_mount")
    on(panel, "DataEvent", fn(event) {
        if event == "AccordionRestoreRequested" {
            body_mount.attach(info_panel_body({
                content = response_content(options.avatar_slot, options.tuning, options.description)
            }))
        }
    })
    return panel
}
