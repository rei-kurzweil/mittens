import { info_panel } from "../ui/info_panel.mms"

let ROW_HEIGHT = 2.9
let INK = [0.13, 0.11, 0.17, 1.0]
let DISABLED_INK = [0.47, 0.45, 0.48, 1.0]

fn auto_accept_text() {
    if NetworkSettings.asset_transfers.auto_accept() { return "On" }
    return "Off"
}

fn setting_row(label, control, faded) {
    let ink = INK
    if faded { ink = DISABLED_INK }
    return T {
        Style {
            display("flex") flex_direction("row") width(100%) height(ROW_HEIGHT)
            align_items("center") gap(0.55) padding_xy(0.6, 0.0)
            background_color([0.95, 0.91, 0.83, 1.0]) background_z(-0.01)
        }
        T {
            Style { display("block") width(0.0) flex_grow(1.0) color(ink) font_size(0.68) }
            Text { label }
        }
        control
    }
}

fn settings_content() {
    let auto_accept = T {
        name = "network_asset_auto_accept"
        Grabbable.off()
        Raycastable.click_only() { interaction_priority(120.0) }
        Style {
            display("flex") width(7.4) height(2.1) align_items("center") justify_content("center")
            background_color([0.88, 0.81, 0.66, 1.0]) background_z(0.01) color(INK) font_size(0.64)
        }
        T.position(0.0, 0.0, 0.03) {
            Raycastable.disabled()
            Text { name = "network_asset_auto_accept_value" auto_accept_text() }
        }
    }
    on(auto_accept, "Click", fn(event) {
        NetworkSettings.asset_transfers.set_auto_accept(!NetworkSettings.asset_transfers.auto_accept())
    })
    let update_rate = T {
        name = "network_server_update_rate"
        Raycastable.disabled()
        Style {
            display("flex") width(7.4) height(2.1) align_items("center") justify_content("center")
            background_color([0.85, 0.83, 0.82, 1.0]) color(DISABLED_INK) font_size(0.55)
        }
        T.position(0.0, 0.0, 0.03) { Text { "Coming soon" } }
    }
    return T {
        Style { display("flex") flex_direction("column") width(100%) row_gap(0.48) }
        setting_row("Auto-accept asset transfers", auto_accept, false)
        setting_row("Desired server update rate", update_rate, true)
    }
}

// Required: root_name. The caller owns world placement.
export fn network_settings_panel(options) {
    let panel = info_panel({
        root_name = options.root_name title = "Network settings"
        width_gu = 40.0 unit_scale = 0.06 content_factory = settings_content
    })
    let root = panel.query("#" + options.root_name)
    let selection_guard = Selectable.off()
    root.attach(selection_guard)
    on_global("DataEvent", fn(event) {
        if event != "NetworkSettingsChanged" { return }
        let current_panel = query("#" + options.root_name)
        if current_panel {
            let readout = current_panel.query("#network_asset_auto_accept_value")
            if readout { readout.set_text(auto_accept_text()) }
        }
    })
    return panel
}
