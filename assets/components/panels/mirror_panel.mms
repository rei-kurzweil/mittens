import { title_bar } from "../ui/title_bar.mms"

export fn mirror_panel(options) {
    let state = { enabled = false }
    let label = Text { name = options.name + "_toggle_label" "Off" }
    let button = T {
        name = options.name + "_toggle"
        Grabbable.off()
        Raycastable.click_only() { interaction_priority(180.0) }
        Style {
            display("flex") width(6.0) height(3.15)
            align_items("center") justify_content("center") font_size(1.10)
            background_color([0.98, 0.87, 0.68, 1.0]) background_z(0.015)
        }
        T.position(0.0, 0.0, 0.03) { Raycastable.disabled() label }
    }
    let mirror = Mirror.quality(options.quality).enabled(false) { name = options.name }
    on(button, "Click", fn(event) {
        state.enabled = !state.enabled
        mirror.set_enabled(state.enabled)
        if state.enabled { label.set_text("On") } else { label.set_text("Off") }
    })

    let header_unit = 0.085
    let header_height = 4.6 * header_unit
    return T {
        name = options.name + "_frame"
        Grabbable {}
        T.position(-options.width / 2.0, options.height / 2.0 + header_height + 0.025, 0.0) {
            Unlit {}
            LayoutRoot { available_width(options.width / header_unit) unit_scale(header_unit)
                title_bar("Mirror", [button], false)
            }
        }
        T.scale(options.width, options.height, 1.0) {
            R.plane() { Raycastable.drag_only() mirror }
        }
    }
}
