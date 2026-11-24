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

fn camera_view_text(enabled) {
    if enabled { return "On" }
    return "Off"
}

fn camera_view_row(camera_view) {
    let enabled = camera_view.enabled()
    let toggle = null
    if enabled { toggle = Toggle.on() { name = "avatar_camera_toggle" } }
    else { toggle = Toggle.off() { name = "avatar_camera_toggle" } }
    on(toggle, "ToggleChanged", fn(event) { camera_view.set(event.value) })
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
            Text { "Third person camera" }
        }
        T {
            name = "avatar_camera_control"
            Grabbable.off()
            Raycastable.click_only() { interaction_priority(120.0) }
            toggle
            Style {
                display("flex") width(9.0) height(2.1)
                align_items("center") justify_content("center") font_size(0.65)
                background_color([0.88, 0.81, 0.66, 1.0]) background_z(0.01)
            }
            T.position(0.0, 0.0, 0.03) {
                Raycastable.disabled()
                Text { name = "avatar_camera_readout" camera_view_text(enabled) }
            }
        }
    }
}

fn avatar_settings_content(options) {
    let desired_meters = AvatarSettings.height.meters()
    let slider_start = desired_meters
    if slider_start == null { slider_start = SLIDER_START_METERS }
    let slider = settings_slider("avatar_height_slider", 0.05, 3.0, 0.01, slider_start, TRACK_WIDTH_GU)
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
        if options["camera_view"] { camera_view_row(options.camera_view) }
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
        if event != "XrSettingsChanged" && event != "AvatarSettingsChanged" && event != "AvatarCameraViewChanged" { return }
        let current_panel = query("#" + options.root_name)
        if !current_panel { return }
        if event == "AvatarCameraViewChanged" && options["camera_view"] {
            let toggle = current_panel.query("#avatar_camera_toggle")
            let readout = current_panel.query("#avatar_camera_readout")
            let enabled = options.camera_view.enabled()
            if toggle { toggle.sync_value(enabled) }
            if readout { readout.set_text(camera_view_text(enabled)) }
        }
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
