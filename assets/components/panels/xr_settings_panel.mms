// Importable live XR settings. XrSettingsSystem owns calibration independently
// of panel lifetime; the caller owns world placement and optional title bar.
import { info_panel, info_panel_body } from "../ui/info_panel.mms"

let UNIT_SCALE = 0.08
let TEXT_COLOR = [0.0, 0.0, 0.0, 1.0]
let BODY_COLOR = [0.98, 0.95, 0.84, 1.0]
let GOLD = [0.57, 0.47, 0.31, 1.0]
let XR_TRACK_WIDTH_GU = 10.5
let THUMB_RADIUS = 0.075

fn correction_text(meters) {
    let tenths_cm = Math.round(Math.abs(meters) * 1000.0)
    let cm = Math.floor(tenths_cm / 10.0)
    let fraction = tenths_cm - cm * 10.0
    let sign = ""
    if meters > 0.0 { sign = "+" } else if meters < 0.0 { sign = "-" }
    return sign + cm + "." + fraction + " cm"
}

export fn settings_slider(name, low, high, step, value, track_width_gu) {
    let track_width = track_width_gu * UNIT_SCALE
    let base_travel = XR_TRACK_WIDTH_GU * UNIT_SCALE - THUMB_RADIUS * 2.0 - 0.04
    let track = T.scale(track_width * 0.5, 0.025, 0.10) {
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
    return Slider.range(low, high).step(step).value(value)
        .width(base_travel * track_width_gu / XR_TRACK_WIDTH_GU).track(track).thumb(thumb) { name = name }
}

export fn settings_slider_row(label, slider, readout, track_width_gu) {
    return T {
        Style {
            display("flex") flex_direction("row") width(100%) height(3.2)
            align_items("center") gap(0.35)
            background_color([0.95, 0.91, 0.79, 1.0]) background_z(-0.02)
        }
        T {
            Style {
                display("flex") width(12.0) height(3.2)
                align_items("center") padding_xy(0.35, 0.0) font_size(0.62)
            }
            T.position(0.0, 0.0, 0.03) { Text { label } }
        }
        T {
            Grabbable.off()
            Style {
                display("flex") width(track_width_gu) height(3.2) flex_grow(1.0)
                align_items("center") justify_content("center")
            }
            slider
        }
        T {
            Style {
                display("flex") width(6.5) height(3.2)
                align_items("center") justify_content("center") font_size(0.65)
            }
            T.position(0.0, 0.0, 0.03) { readout }
        }
    }
}

export fn xr_floor_settings_content(calibration_file, track_width_gu) {
    let correction = XrSettings.floor.correction_meters()
    let slider = settings_slider("xr_floor_correction_slider", -1.0, 1.0, 0.005, correction, track_width_gu)
    let readout = Text { name = "xr_floor_correction_readout" correction_text(correction) }
    let reset = T {
        name = "xr_floor_correction_reset"
        Raycastable.click_only() { interaction_priority(120.0) }
        Style {
            display("flex") width(9.0) height(2.1)
            align_items("center") justify_content("center") font_size(0.62)
            background_color([0.88, 0.81, 0.66, 1.0]) background_z(0.01)
        }
        Text { "Reset to zero" }
    }
    on(slider, "SliderChanged", fn(event) {
        XrSettings.floor.set_correction_meters(event.value)
        readout.set_text(correction_text(XrSettings.floor.correction_meters()))
    })
    on(slider, "SliderCommitted", fn(event) {
        if calibration_file != null {
            XrSettings.floor.set_correction_meters(event.value, {file = calibration_file})
        }
    })
    on(reset, "Click", fn(event) {
        if calibration_file != null {
            XrSettings.floor.set_correction_meters(0.0, {file = calibration_file})
        } else {
            XrSettings.floor.set_correction_meters(0.0)
        }
        slider.sync_value(XrSettings.floor.correction_meters())
        readout.set_text(correction_text(XrSettings.floor.correction_meters()))
    })
    return T {
        name = "xr_settings_content"
        Style { display("flex") flex_direction("column") width(100%) row_gap(0.5) }
        settings_slider_row("Floor correction", slider, readout, track_width_gu)
        T {
            Style { display("block") width(100%) font_size(0.55) }
            Text { "Positive lowers view and hands; negative raises them." }
        }
        T {
            Style {
                display("flex") width(100%) height(2.6)
                align_items("center") justify_content("flex-end")
            }
            reset
        }
    }
}

export fn sync_xr_floor_settings(panel) {
    let value = XrSettings.floor.correction_meters()
    let slider = panel.query("#xr_floor_correction_slider")
    let readout = panel.query("#xr_floor_correction_readout")
    if slider { slider.sync_value(value) }
    if readout { readout.set_text(correction_text(value)) }
}

// Required: root_name. Optional: title and calibration_file (save on commit/reset).
// Callers explicitly load their chosen file once during scene initialization.
export fn xr_settings_panel(options) {
    let calibration_file = options["calibration_file"]
    let title = "XR settings"
    if options["title"] != null { title = options.title }
    let panel = info_panel({
        root_name = options.root_name
        width_gu = 32.0
        unit_scale = UNIT_SCALE
        title = title
        title_background_color = [0.93, 0.87, 0.71, 1.0]
        title_text_color = TEXT_COLOR
        toggle_background_color = [0.88, 0.81, 0.66, 1.0]
        toggle_icon_color = GOLD
        toggle_icon_glow_intensity = 0.0
        body_background_color = BODY_COLOR
        body_text_color = TEXT_COLOR
        content = xr_floor_settings_content(calibration_file, XR_TRACK_WIDTH_GU)
    })
    let panel_root = panel.query("#" + options.root_name)
    let selection_guard = Selectable.off()
    panel_root.attach(selection_guard)
    let body_mount = panel.query("#accordion_body_mount")
    on(panel_root, "DataEvent", fn(event) {
        if event == "AccordionRestoreRequested" {
            body_mount.attach(info_panel_body({
                content = xr_floor_settings_content(calibration_file, XR_TRACK_WIDTH_GU)
                body_background_color = BODY_COLOR
                body_text_color = TEXT_COLOR
            }))
        }
    })
    on_global("DataEvent", fn(event) {
        if event == "XrSettingsChanged" {
            let current_panel = query("#" + options.root_name)
            if current_panel { sync_xr_floor_settings(current_panel) }
        }
    })
    return panel
}
