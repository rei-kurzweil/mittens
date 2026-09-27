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
let CONTROL_FONT_SIZE = 0.65
let UNIT_SCALE = 0.08
let THUMB_RADIUS = 0.08
let TRACK_WIDTH = SLIDER_SLOT_WIDTH * UNIT_SCALE
// Slider.width is the thumb-centre travel in world units. Keep the whole
// thumb within the visible track at both ends.
let SLIDER_WIDTH = TRACK_WIDTH - THUMB_RADIUS * 2.0 - 0.04

// Same pearlescent palette as the XR linear velocity panel. Scenes can provide
// their own theme object with these keys.
export fn voice_response_pastel_theme() {
    return {
        title_background = [0.94, 0.87, 0.79, 0.98]
        title_text = [0.28, 0.23, 0.27, 1.0]
        toggle_background = [0.85, 0.82, 0.95, 1.0]
        toggle_icon = [0.40, 0.32, 0.48, 1.0]
        body_background = [0.79, 0.92, 0.82, 0.98]
        body_text = [0.20, 0.30, 0.26, 1.0]
        row_background = [0.91, 0.96, 0.86, 0.98]
        track = [0.85, 0.82, 0.95, 1.0]
        thumb = [0.98, 0.81, 0.93, 1.0]
    }
}

fn response_slider(slider_name, initial, minimum, maximum, step, theme) {
    let track = T.scale(TRACK_WIDTH * 0.5, 0.025, 0.10) {
        R.cube() {
            C.rgba(theme.track[0], theme.track[1], theme.track[2], theme.track[3])
            Raycastable.enabled() { interaction_priority(120.0) }
        }
    }
    let thumb = T.scale(THUMB_RADIUS, 0.16, 0.16) {
        R.sphere() {
            C.rgba(theme.thumb[0], theme.thumb[1], theme.thumb[2], theme.thumb[3])
            Raycastable.enabled() { interaction_priority(120.0) }
        }
    }
    return Slider.range(minimum, maximum).step(step).value(initial).width(SLIDER_WIDTH)
        .track(track).thumb(thumb) { name = slider_name }
}

fn response_row(text, slider, readout, theme) {
    return T {
        Style {
            display("flex") flex_direction("row") width(100%) height(ROW_HEIGHT)
            align_items("center") gap(0.5)
            background_color(theme.row_background) background_z(-0.02)
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
                color(theme.body_text)
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

fn response_content(avatar_slot, tuning, description, filter_slot, theme) {
    let center_slider = response_slider("mouth_rms_center_slider", tuning.center_rms, 0.001, 0.120, 0.001, theme)
    let center_readout = Text { name = "mouth_rms_center_readout" fixed_3(tuning.center_rms) + " RMS" }
    let range_slider = response_slider("mouth_rms_range_slider", tuning.range_rms, 0.001, 0.120, 0.001, theme)
    let range_readout = Text { name = "mouth_rms_range_readout" fixed_3(tuning.range_rms) + " RMS" }
    let amount_slider = response_slider("mouth_amount_slider", tuning.amount, 0.0, 1.0, 0.01, theme)
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

    let filter_controls = T {}
    if filter_slot {
        let cutoff_slider = response_slider("highpass_cutoff_slider", filter_slot.cutoff_hz, 40.0, 400.0, 5.0, theme)
        let cutoff_readout = Text { name = "highpass_cutoff_readout" filter_slot.cutoff_hz + " Hz" }
        let resonance_slider = response_slider("highpass_resonance_slider", filter_slot.resonance, 0.2, 2.0, 0.001, theme)
        let resonance_readout = Text { name = "highpass_resonance_readout" filter_slot.resonance + " Q" }
        on(cutoff_slider, "SliderChanged", fn(event) {
            filter_slot.cutoff_hz = event.value
            if filter_slot.amplitude { filter_slot.amplitude.set_highpass(event.value) }
            cutoff_readout.set_text(event.value + " Hz")
        })
        on(resonance_slider, "SliderChanged", fn(event) {
            filter_slot.resonance = event.value
            if filter_slot.amplitude { filter_slot.amplitude.set_highpass_resonance(event.value) }
            resonance_readout.set_text(event.value + " Q")
        })
        filter_controls = T {
            Style { display("flex") flex_direction("column") width(100%) row_gap(0.20) }
            response_row("high-pass Hz", cutoff_slider, cutoff_readout, theme)
            response_row("resonance Q", resonance_slider, resonance_readout, theme)
        }
    }

    return T {
        name = "mouth_response_settings_content"
        Style { display("flex") flex_direction("column") width(100%) row_gap(0.20) }
        T {
            Style { display("block") width(100%) font_size(0.55) color(theme.body_text) }
            Text { description }
        }
        filter_controls
        response_row("RMS centre", center_slider, center_readout, theme)
        response_row("RMS range", range_slider, range_readout, theme)
        response_row("mouth amount", amount_slider, amount_readout, theme)
    }
}

// Required: root_name, title, avatar_slot, tuning, description.
// Optional: filter_slot with amplitude, cutoff_hz and resonance; theme.
// The caller sets avatar_slot.avatar after authoring its AVC later in the scene.
export fn mouth_response_panel(options) {
    let theme = options["theme"]
    if !theme { theme = voice_response_pastel_theme() }
    let filter_slot = options["filter_slot"]
    let panel = info_panel({
        root_name = options.root_name
        width_gu = 32.0
        unit_scale = UNIT_SCALE
        title = options.title
        title_background_color = theme.title_background
        title_text_color = theme.title_text
        toggle_background_color = theme.toggle_background
        toggle_icon_color = theme.toggle_icon
        toggle_icon_glow_intensity = 0.25
        body_background_color = theme.body_background
        body_text_color = theme.body_text
        content = response_content(options.avatar_slot, options.tuning, options.description, filter_slot, theme)
    })
    let body_mount = panel.query("#accordion_body_mount")
    on(panel, "DataEvent", fn(event) {
        if event == "AccordionRestoreRequested" {
            body_mount.attach(info_panel_body({
                content = response_content(options.avatar_slot, options.tuning, options.description, filter_slot, theme)
                body_background_color = theme.body_background
                body_text_color = theme.body_text
            }))
        }
    })
    return panel
}
