// assets.mms — main assets panel factory (=^･ω･^=)

import { assets_content } from "./assets_content.mms"

export fn assets(title, items, title_color, panel_background_color, item_background_color) {
    return LayoutRoot {
        name = "assets_layout_root"
        available_width(60.0)
        available_height(80.0)
        unit_scale(0.08)

        Style {
            background_color(panel_background_color)
            background_z(-0.01)
        }

        // Title Bar (Block by default)
        T {
            name = "title_bar"
            Style {
                height(6.0)
                background_color(panel_background_color)
                background_z(-0.01)
                font_size(1)
                margin_xy(0.5, 0.5)
            }
            T.position(2.0, 1.8, 0.0) {
                T {
                    Style {
                        color = title_color
                    }
                    Text { title }
                }
            }
        }

        // Content Area
        Selection {
            name = "assets_selection"
            assets_content(items, item_background_color)
        }
    }
}
