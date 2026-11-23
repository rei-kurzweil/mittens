import { info_panel } from "../ui/info_panel.mms"
import { settings_slider, settings_slider_row, xr_floor_settings_content, sync_xr_floor_settings } from "./xr_settings_panel.mms"

let TRACK_WIDTH_GU = 21.0
let SLIDER_START_METERS = 1.7

fn height_text(meters) {
    if meters == null { return "Model default" }
    let cm = Math.round(meters * 100.0)
    let whole = Math.floor(cm / 100.0)
    let fraction = cm - whole * 100.0
    let digits = "" + fraction
    if fraction < 10 { digits = "0" + digits }
    return whole + "." + digits + " m"
}

fn avatar_settings_content(options) {
    let desired_meters = AvatarSettings.height.meters()
    let slider_start = desired_meters
    if slider_start == null { slider_start = SLIDER_START_METERS }
    let slider = settings_slider("avatar_height_slider", 0.5, 3.0, 0.01, slider_start, TRACK_WIDTH_GU)
    let readout = Text { name = "avatar_height_readout" height_text(desired_meters) }
    on(slider, "SliderChanged", fn(event) { readout.set_text(height_text(event.value)) })
    on(slider, "SliderCommitted", fn(event) {
        let previous = AvatarSettings.height.meters()
        AvatarSettings.height.set_meters(event.value)
        if previous == AvatarSettings.height.meters() && options["on_height_recommitted"] {
            options.on_height_recommitted()
        }
    })
    return T {
        Style { display("flex") flex_direction("column") width(100%) row_gap(0.5) }
        settings_slider_row("Desired height", slider, readout, TRACK_WIDTH_GU)
        if options["show_xr"] != false {
            T {
                Style { display("block") width(100%) font_size(0.70) }
                Text { "XR settings" }
            }
            xr_floor_settings_content(options["calibration_file"], TRACK_WIDTH_GU)
        }
    }
}

export fn avatar_settings_panel(options) {
    let panel = info_panel({
        root_name = options.root_name
        width_gu = 44.0
        unit_scale = 0.08
        title = "Humanoid settings"
        toggle_icon_glow_intensity = 0.0
        content_factory = fn() { return avatar_settings_content(options) }
    })
    let root = panel.query("#" + options.root_name)
    let selection_guard = Selectable.off()
    root.attach(selection_guard)
    on_global("DataEvent", fn(event) {
        if event != "XrSettingsChanged" && event != "AvatarSettingsChanged" { return }
        let current_panel = query("#" + options.root_name)
        if !current_panel { return }
        if event == "XrSettingsChanged" { sync_xr_floor_settings(current_panel) }
        if event == "AvatarSettingsChanged" {
            let value = AvatarSettings.height.meters()
            let slider = current_panel.query("#avatar_height_slider")
            let readout = current_panel.query("#avatar_height_readout")
            if slider && value != null { slider.sync_value(value) }
            if readout { readout.set_text(height_text(value)) }
        }
    })
    return panel
}
