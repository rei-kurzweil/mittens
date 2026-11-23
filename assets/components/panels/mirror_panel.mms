import { title_bar } from "../ui/title_bar.mms"

export fn mirror_panel(options) {
    let header_unit = 0.085
    let button_width = 6.0
    let button_height = 3.15
    let label_size = 1.10
    fn centered_label(text) {
        // The caption is one line of fixed-width glyphs. Updating its text
        // does not reposition the existing layout anchor.
        return [(button_width - byte_len(text) * label_size) * header_unit / 2.0,
            -(button_height - label_size) * header_unit / 2.0, 0.03]
    }
    let state = { enabled = false }
    let label = Text { name = options.name + "_toggle_label" "Off" }
    let initial = centered_label("Off")
    let label_anchor = T.position(initial[0], initial[1], initial[2]) { Raycastable.disabled() label }
    let button = T {
        name = options.name + "_toggle"
        Grabbable.off()
        Raycastable.click_only() { interaction_priority(180.0) }
        Style {
            display("flex") width(button_width) height(button_height)
            text_align("center") vertical_align("middle") font_size(label_size)
            background_color([0.98, 0.87, 0.68, 1.0]) background_z(0.015)
        }
        label_anchor
    }
    let mirror = Mirror.quality(options.quality).enabled(false) { name = options.name }
    on(button, "Click", fn(event) {
        state.enabled = !state.enabled
        mirror.set_enabled(state.enabled)
        let caption = "Off"
        if state.enabled { caption = "On" }
        label.set_text(caption)
        label_anchor.update_transform(centered_label(caption))
    })

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
