// assets/components/paint_panel.mms — paint tools panel factory (=^･ω･^=)

import { paint_panel_item } from "./paint_panel_item.mms"

let PAINT_PANEL_WIDTH_GU = 41.0
let TITLE_BAR_HEIGHT_GU = 3.0
let TITLE_CONTENT_GAP_GU = 0.5
let PAINT_PANEL_CONTENT_HEIGHT_GU = 57.0
let PAINT_PANEL_TOTAL_HEIGHT_GU = TITLE_BAR_HEIGHT_GU + TITLE_CONTENT_GAP_GU + PAINT_PANEL_CONTENT_HEIGHT_GU

export fn paint_panel(title, title_color, panel_background_color, item_background_color) {
    return T {
        name = "paint_panel_root"
        Style {
            display("block")
            width(PAINT_PANEL_WIDTH_GU)
            height(PAINT_PANEL_TOTAL_HEIGHT_GU)
            margin_xy(0.5, 0.5)
        }

        T {
            name = "title_bar"
            Style {
                display("block")
                height(TITLE_BAR_HEIGHT_GU)
                margin_bottom(TITLE_CONTENT_GAP_GU)
                padding_xy(0.5, 0.5)
                color = title_color
                background_color(panel_background_color)
                text_align("left")
                vertical_align("middle")
                background_z(-0.01)
            }
            T.position(0.0, 0.0, 0.0) {
                Text { title }
            }
        }

        T {
            name = "content_slot"
            Style {
                display("block")
                height(PAINT_PANEL_CONTENT_HEIGHT_GU)
                overflow("scroll")
                background_color([0.96, 0.92, 0.18, 0.80])
                background_z(-0.001)
                padding(0.5)
            }

            paint_panel_item("Free Draw", item_background_color, title_color)
            paint_panel_item("Line", item_background_color, title_color)
            paint_panel_item("Spray Can", item_background_color, title_color)
            paint_panel_item("Fill", item_background_color, title_color)
            paint_panel_item("Erase", item_background_color, title_color)
        }
    }
}
