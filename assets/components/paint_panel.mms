// assets/components/paint_panel.mms — paint tools panel factory (=^･ω･^=)

import { paint_panel_item } from "./paint_panel_item.mms"
import { 
    pencil_icon, 
    line_icon, 
    spray_can_icon, 
    fill_icon, 
    erase_icon 
} from "./icons.mms"

export fn paint_panel() {
    return LayoutRoot {
        name = "paint_panel_layout_root"
        available_width(41.0)
        available_height(10.0)
        unit_scale(0.1)

        T {
            name = "paint_panel_container"
            Style {
                width(100%)
                height(100%)
                padding(0.5)
                background_color = [0.12, 0.12, 0.12, 0.95]
            }
            
            paint_panel_item("Free Draw", pencil_icon)
            paint_panel_item("Line", line_icon)
            paint_panel_item("Spray Can", spray_can_icon)
            paint_panel_item("Fill", fill_icon)
            paint_panel_item("Erase", erase_icon)
        }
    }
}
