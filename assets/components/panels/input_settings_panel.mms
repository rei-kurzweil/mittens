// Importable input settings. InputSettingsSystem owns the preferences; scenes
// apply them to their own input components on InputSettingsChanged.
import { info_panel } from "../ui/info_panel.mms"

let STICK_MODES = [
    { id = "strafe" label = "Left stick only" },
    { id = "turn" label = "Right stick turns" },
]

fn radio_text(mode, selected) {
    if mode.id == selected { return "(o) " + mode.label }
    return "( ) " + mode.label
}

fn stick_mode_button(mode, settings_file) {
    let button = T {
        name = "input_stick_mode_" + mode.id
        Grabbable.off()
        Raycastable.click_only() { interaction_priority(120.0) }
        Style {
            display("flex") width(13.0) height(2.4)
            align_items("center") padding_xy(0.5, 0.0) font_size(0.62)
            background_color([0.88, 0.81, 0.66, 1.0]) background_z(0.01)
        }
        T.position(0.0, 0.0, 0.03) {
            Raycastable.disabled()
            Text { name = "input_stick_mode_" + mode.id + "_label" radio_text(mode, InputSettings.xr.stick_mode()) }
        }
    }
    on(button, "Click", fn(event) {
        if settings_file { InputSettings.xr.set_stick_mode(mode.id, { file = settings_file }) }
        else { InputSettings.xr.set_stick_mode(mode.id) }
    })
    return button
}

export fn sync_input_settings(panel) {
    let selected = InputSettings.xr.stick_mode()
    for mode in STICK_MODES {
        let label = panel.query("#input_stick_mode_" + mode.id + "_label")
        if label { label.set_text(radio_text(mode, selected)) }
    }
}

fn input_settings_content(settings_file) {
    return T {
        name = "input_settings_content"
        Style { display("flex") flex_direction("column") width(100%) row_gap(0.5) }
        T {
            Style { display("block") width(100%) font_size(0.70) }
            Text { "XR controls" }
        }
        T {
            Style {
                display("flex") flex_direction("row") width(100%) height(3.2)
                align_items("center") gap(0.35)
                background_color([0.95, 0.91, 0.79, 1.0]) background_z(-0.02)
            }
            T {
                Style {
                    display("flex") width(8.0) height(3.2)
                    align_items("center") padding_xy(0.35, 0.0) font_size(0.62)
                }
                T.position(0.0, 0.0, 0.03) { Text { "Sticks" } }
            }
            for mode in STICK_MODES { stick_mode_button(mode, settings_file) }
        }
        T {
            Style { display("block") width(100%) font_size(0.55) }
            Text { "The left stick always moves and strafes." }
        }
    }
}

// Required: root_name. Optional: settings_file, which saves each choice
// (see InputSettings.xr.default_file()). The caller owns world placement.
export fn input_settings_panel(options) {
    let panel = info_panel({
        root_name = options.root_name
        width_gu = 40.0
        unit_scale = 0.08
        title = "Input settings"
        toggle_icon_glow_intensity = 0.0
        content_factory = fn() { return input_settings_content(options["settings_file"]) }
    })
    let root = panel.query("#" + options.root_name)
    let selection_guard = Selectable.off()
    root.attach(selection_guard)
    on_global("DataEvent", fn(event) {
        if event != "InputSettingsChanged" { return }
        let current_panel = query("#" + options.root_name)
        if current_panel { sync_input_settings(current_panel) }
    })
    return panel
}
