// Standalone title-bar building blocks. Existing panels do not use these yet.
import { minimize_circle_icon } from "../icons.mms"

// The caller owns window behavior and supplies buttons in display order.
// Future close/minimize/maximize order puts minimize in the middle.
// Maximize sizing is deliberately left to the owning panel/container.
// Use LayoutRoot.unit_scale(1); scale the owning panel's transform for world size.
export fn title_bar(title, buttons) {
    return T {
        name = "title_bar"
        Grabbable.parent()
        Raycastable.enabled()
        Style {
            display("flex") flex_direction("row") align_items("center")
            width(100%) height(4.6) padding_xy(0.6, 0)
            background_color([0.98, 0.70, 0.20, 1]) color([0.12, 0.075, 0.012, 1])
        }
        T {
            Style { width(0) flex_grow(1) padding_xy(0.5, 0) font_size(1.65) }
            Text { title }
        }
        for button in buttons { button }
    }
}

// Emits a request; removing/restoring the window body belongs to its owner.
export fn minimize_button() {
    let button = T {
        name = "title_bar_minimize"
        Grabbable.off()
        Raycastable.enabled() { interaction_priority(110.0) }
        Style { width(4.0) height(4.0) }
        T.position(2, -2, 0.02).scale(2.0, 2.0, 1) {
            minimize_circle_icon([0.12, 0.075, 0.012, 1], 0)
        }
    }
    on(button, "Click", fn(event) { emit_data(button, "TitleBarMinimizeRequested", null) })
    return button
}
