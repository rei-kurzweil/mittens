// assets.mms — main assets panel factory (=^･ω･^=)

import { assets_content } from "./assets_content.mms"

let ASSET_PANEL_WIDTH_GU = 30.0
let TITLE_BAR_HEIGHT_GU = 3.0
let TITLE_CONTENT_GAP_GU = 0.5
let ASSET_PANEL_CONTENT_HEIGHT_GU = 57.0 
let ASSET_PANEL_TOTAL_HEIGHT_GU = TITLE_BAR_HEIGHT_GU + TITLE_CONTENT_GAP_GU + ASSET_PANEL_CONTENT_HEIGHT_GU

export fn assets(title, items, title_color, panel_background_color, item_background_color) {
    return T {
        name = "assets_root"
        Style {
            display("block")
            width(ASSET_PANEL_WIDTH_GU)
            height(ASSET_PANEL_TOTAL_HEIGHT_GU)
            margin_xy(0.5, 0.5)
        }

        // Title Bar
        T {
            name = "title_bar"
            Style {
                display("block")
                height(TITLE_BAR_HEIGHT_GU)
                margin_bottom(TITLE_CONTENT_GAP_GU)
                background_color(panel_background_color)
                background_z(-0.01)
                font_size(1)
                color = title_color
                padding_xy(0.5, 0.5)
                text_align("left")
                vertical_align("middle")
            }
            T.position(0.0, 0.0, 0.0) {
                Text { title }
            }
        }

        // Content Area
        T {
            name = "content_slot"
            Style {
                display("block")
                height(ASSET_PANEL_CONTENT_HEIGHT_GU)
                overflow("scroll")
                background_color([0.96, 0.92, 0.18, 0.80]) 
                background_z(-0.001)
            }
            Selection {
                name = "assets_selection"
                assets_content(items, item_background_color)
            }
        }
    }
}
