// The caller supplies the roster and owns avatar construction and selection.
fn avatar_button(choice, on_select) {
    let button = T {
        name = "avatar_picker_" + choice.id
        Style {
            display("flex") width(6.5) height(3.6)
            align_items("center") justify_content("center")
            font_size(0.68)
        }
        // Layout puts the origin at the upper left. Give the full 6.5 x 3.6
        // layout-unit face one stable pointer target, including under the label.
        T.position(0.195, -0.108, 0.01).scale(0.39, 0.216, 0.014) {
            R.cube() {
                C.rgba(0.88, 0.81, 0.66, 1.0)
                Raycastable.click_only() { interaction_priority(180.0) }
            }
        }
        T.position(0.0, 0.0, 0.03) { Raycastable.disabled() Text { choice.label } }
    }
    on(button, "Click", fn(event) { on_select(choice.id) })
    return button
}

export fn avatar_picker_panel(options) {
    let status = Text { name = "avatar_picker_status" options.status }
    let panel_width = 1.6
    for choice in options.choices { panel_width = panel_width + 6.95 }
    let content = T {
        Style {
            display("flex") flex_direction("column") width(panel_width)
            padding(0.8) row_gap(0.55)
            background_color([0.98, 0.95, 0.84, 1.0]) background_z(-0.02)
            color([0.12, 0.10, 0.16, 1.0])
        }
        T {
            Style { display("block") width(100%) height(1.25) font_size(0.82) }
            T.position(0.0, 0.0, 0.03) { Text { "Your avatar" } }
        }
        T {
            Style { display("flex") flex_direction("row") width(100%) gap(0.45) }
            for choice in options.choices { avatar_button(choice, options.on_select) }
        }
        T {
            Style { display("block") width(100%) height(1.45) font_size(0.60) }
            T.position(0.0, 0.0, 0.03) { status }
        }
    }
    let panel = T {
        name = options.root_name
        Selectable.off()
        Unlit {}
        LayoutRoot {
            available_width(panel_width)
            unit_scale(0.06)
            content
        }
    }
    return { root = panel set_status = fn(text) { status.set_text(text) } }
}
