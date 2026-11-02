// Optional visual study; no application imports this scene.
import { title_bar, minimize_button } from "./title_bar.mms"
import { ui_unlit_material } from "../materials/ui_unlit.mms"

BGC.rgba(0.16, 0.12, 0.18, 1)
T.position(-1.5, 0.65, -2.6).scale(0.03, 0.03, 0.03) {
    ui_unlit_material()
    LayoutRoot { available_width(100) unit_scale(1)
        T {
            name = "preview_window"
            Style { width(100%) }
            title_bar("TERMINAL", [minimize_button()])
            T {
                Style {
                    height(24) padding(2) background_color([0.025, 0.02, 0.03, 1])
                    color([1, 0.76, 0.30, 1]) font_size(1.3)
                }
                T { Style { font_size(1.3) } Text { "Grip the title to move the panel." } }
                T { Style { font_size(1.3) } Text { "A circular chevron requests minimize." } }
            }
        }
        T { Style { height(3) } }
        T { Style { width(100%) } title_bar("FILES", [minimize_button()]) }
    }
}
