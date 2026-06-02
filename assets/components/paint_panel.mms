// assets/components/paint_panel.mms — paint tools panel factory (=^･ω･^=)

import { paint_panel_item } from "./paint_panel_item.mms"

export fn paint_panel(title, title_color, panel_background_color, item_background_color) {
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
                background_color(panel_background_color)
                background_z(-0.01)
            }

            T {
                name = "paint_panel_title_bar"
                Style {
                    display("block")
                    height(2.5)
                    margin_bottom(0.4)
                    padding_xy(0.2, 0.3)
                    font_size(1)
                    color = title_color
                    text_align("left")
                    vertical_align("middle")
                    background_z(-0.01)
                }
                T.position(0.0, 0.0, 0.0) {
                    Text { title }
                }
            }

            paint_panel_item("Free Draw", item_background_color, title_color)
            paint_panel_item("Line", item_background_color, title_color)
            paint_panel_item("Spray Can", item_background_color, title_color)
            paint_panel_item("Fill", item_background_color, title_color)
            paint_panel_item("Erase", item_background_color, title_color)
        }
    }
}
