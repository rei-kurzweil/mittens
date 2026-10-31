// MMS owns both window frames; the repeating file grid is native Rust.
import { f32 } from "types"

export fn caption(text) { return Text { text } }
export fn action(label, id) {
    return T {
        name = id
        Raycastable.enabled()
        Style {
            display("inline-block") padding_xy(0.8, 0.55) margin_right(0.5)
            background_color([0.98, 0.75, 0.26, 1.0]) color([0.13, 0.085, 0.015, 1.0])
        }
        Text { label }
    }
}
fn window(title, content, window_width, dark, gold_definition) {
    let gold_material = Material(gold_definition).inputs({ brightness = 1.0 })
    let background = [1.0, 0.965, 0.84, 1.0]
    let foreground = [0.12, 0.08, 0.025, 1.0]
    if dark { background = [0.018, 0.016, 0.012, 1.0] foreground = [1.0, 0.72, 0.16, 1.0] }
    return T {
        Style { position("relative") width(window_width) }
        // Solid amber title. The narrow rails continue its two outer edges.
        T {
            gold_material
            Style {
                height(4.6) padding_xy(1.1, 1.0) font_size(1.65)
                background_color([0.98, 0.70, 0.20, 1.0]) color([0.12, 0.075, 0.012, 1.0])
            }
            Text { title }
        }
        T {
            Style { display("flex") flex_direction("row") column_gap(0.65) height(49.5) }
            T { Style { width(0.45) height(49.5) background_color([0.93, 0.60, 0.12, 1]) } }
            T {
                Style {
                    width(window_width - 2.2) margin_top(0.8)
                    padding(0.9) height(48.7) background_color(background) color(foreground)
                }
                content
            }
            T { Style { width(0.45) height(49.5) background_color([0.93, 0.60, 0.12, 1]) } }
        }
    }
}
export fn shell(title, subtitle, connection, browser, terminal, footer) {
    let ui_definition = MaterialDefinition(MaterialBase()).fragment("assets/shaders/mitten-droid-ui.frag").inputs({ brightness: f32 })
    let material = Material(ui_definition).inputs({ brightness = 1.0 })
    let definition = MaterialDefinition(MaterialBase()).fragment("assets/shaders/mitten-droid-gold.frag").inputs({ brightness: f32 })
    return LayoutRoot {
        name = "shell_layout"
        available_width(126) available_height(66)
        material
        Style { width(126) font_size(1.05) color([1.0, 0.75, 0.25, 1.0]) }
        T { Style { margin_bottom(0.5) font_size(1.3) color([0.20, 0.09, 0.16, 1]) } Text { title } }
        T { Style { margin_bottom(1) font_size(0.9) color([0.20, 0.09, 0.16, 1]) } Text { subtitle } }
        T { Style { margin_bottom(1) color([0.20, 0.09, 0.16, 1]) } connection }
        T {
            Style { display("flex") flex_direction("row") column_gap(2.0) }
            window("TERMINAL", terminal, 63, true, definition)
            window("FILES", browser, 61, false, definition)
        }
        T { Style { margin_top(1) font_size(0.9) color([0.20, 0.09, 0.16, 1]) } footer }
    }
}
