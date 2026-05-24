// assets.mms — main assets panel factory (=^･ω･^=)

import { assets_content } from "./assets_content.mms"

export fn assets(title, items) {
    return LayoutRoot {
        name = "assets_layout_root"
        available_width(60.0)
        available_height(80.0)
        unit_scale(0.08)

        // Title Bar (Block by default)
        T {
            name = "title_bar"
            Style {
                height(6.0)
                background_color = [0.15, 0.15, 0.15, 1.0]
            }
            T.position(2.0, 1.8, 0.05).scale(1.2, 1.2, 1.2) {
                Text {
                    title
                    C.rgba(1.0, 1.0, 1.0, 1.0)
                }
            }
        }

        // Content Area
        assets_content(items)
    }
}
