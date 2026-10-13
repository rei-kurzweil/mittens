// Importable graphics settings UI preview. Native controls update only this
// instance's local display state; backend settings will be connected later.
import { info_panel, info_panel_body } from "../ui/info_panel.mms"

let QUALITY_STEPS = [64, 128, 256, 512, 1024, 1440, 2048]
let UNIT_SCALE = 0.08
let ROW_HEIGHT = 3.2
let TEXT_COLOR = [0.96, 0.93, 0.82, 1.0]
let BODY_COLOR = [0.055, 0.065, 0.09, 0.98]
let ROW_COLOR = [0.10, 0.12, 0.16, 1.0]
let GOLD = [1.0, 0.82, 0.24, 1.0]
let TRACK_WIDTH = 10.5 * UNIT_SCALE
let THUMB_RADIUS = 0.075
let SLIDER_WIDTH = TRACK_WIDTH - THUMB_RADIUS * 2.0 - 0.04

fn enabled_text(enabled) {
    if enabled { return "On" }
    return "Off"
}

fn preview_toggle(enabled) {
    if enabled { return Toggle.on() { name = "graphics_mirrors_toggle" } }
    return Toggle.off() { name = "graphics_mirrors_toggle" }
}

fn setting_label(text) {
    return T {
        Style {
            display("flex") width(12.0) height(ROW_HEIGHT)
            align_items("center") padding_xy(0.35, 0.0) font_size(0.62)
        }
        T.position(0.0, 0.0, 0.03) { Text { text } }
    }
}

fn setting_row(label, control, readout) {
    return T {
        Style {
            display("flex") flex_direction("row") width(100%) height(ROW_HEIGHT)
            align_items("center") gap(0.35)
            background_color(ROW_COLOR) background_z(-0.02)
        }
        setting_label(label)
        T {
            Style {
                display("flex") width(10.5) height(ROW_HEIGHT) flex_grow(1.0)
                align_items("center") justify_content("center")
            }
            control
        }
        T {
            Style {
                display("flex") width(6.5) height(ROW_HEIGHT)
                align_items("center") justify_content("center") font_size(0.65)
            }
            T.position(0.0, 0.0, 0.03) { readout }
        }
    }
}

fn settings_content(state) {
    let mirror_toggle = preview_toggle(state.enabled)
    let toggle_control = T {
        name = "graphics_mirrors_toggle_control"
        Raycastable.click_only() { interaction_priority(120.0) }
        mirror_toggle
        Style {
            display("flex") width(5.0) height(2.1)
            align_items("center") justify_content("center") font_size(0.65)
            color(TEXT_COLOR) background_color([0.20, 0.23, 0.29, 1.0])
            background_z(0.01)
        }
        Text { "Mirrors" }
    }
    let toggle_readout = Text {
        name = "graphics_mirrors_enabled_readout"
        enabled_text(state.enabled)
    }
    let track = T.scale(TRACK_WIDTH * 0.5, 0.025, 0.10) {
        R.cube() {
            C.rgba(0.35, 0.30, 0.17, 1.0)
            Raycastable.enabled() { interaction_priority(120.0) }
        }
    }
    let thumb = T.scale(THUMB_RADIUS, 0.14, 0.14) {
        R.sphere() {
            C.rgba(GOLD[0], GOLD[1], GOLD[2], GOLD[3])
            Raycastable.enabled() { interaction_priority(120.0) }
        }
    }
    let quality_slider = Slider.range(0.0, 6.0).step(1.0).value(state.quality_index).width(SLIDER_WIDTH)
        .track(track).thumb(thumb) { name = "graphics_mirrors_quality_slider" }
    let quality_readout = Text {
        name = "graphics_mirrors_quality_readout"
        "" + QUALITY_STEPS[state.quality_index]
    }

    on(mirror_toggle, "ToggleChanged", fn(event) {
        state.enabled = event.value
        toggle_readout.set_text(enabled_text(state.enabled))
    })
    on(quality_slider, "SliderChanged", fn(event) {
        state.quality_index = event.value
        quality_readout.set_text("" + QUALITY_STEPS[state.quality_index])
    })

    return T {
        name = "graphics_settings_content"
        Style { display("flex") flex_direction("column") width(100%) row_gap(0.35) }
        setting_row("Mirrors", toggle_control, toggle_readout)
        setting_row("Resolution preference", quality_slider, quality_readout)
    }
}

// Required: root_name. Optional: title. The caller owns world placement.
export fn graphics_settings_panel(options) {
    let title = "Graphics settings"
    if options["title"] { title = options.title }
    let state = { enabled = true quality_index = 3 }
    let panel = info_panel({
        root_name = options.root_name
        width_gu = 32.0
        unit_scale = UNIT_SCALE
        title = title
        title_background_color = [0.16, 0.14, 0.09, 0.98]
        title_text_color = TEXT_COLOR
        toggle_background_color = [0.25, 0.20, 0.10, 1.0]
        toggle_icon_color = GOLD
        toggle_icon_glow_intensity = 0.3
        body_background_color = BODY_COLOR
        body_text_color = TEXT_COLOR
        content = settings_content(state)
    })
    let body_mount = panel.query("#accordion_body_mount")
    on(panel, "DataEvent", fn(event) {
        if event == "AccordionRestoreRequested" {
            body_mount.attach(info_panel_body({
                content = settings_content(state)
                body_background_color = BODY_COLOR
                body_text_color = TEXT_COLOR
            }))
        }
    })
    return panel
}
