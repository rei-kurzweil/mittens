// assets_content.mms — content area factory (=^･ω･^=)

import { asset_item } from "./asset_item.mms"

export fn assets_content(items, item_background_color) {
    return T {
        name = "assets_content_area"
        id = "assets_content_area"
        Selection {
            id = "assets_selection"
        }
        Style {
            display("block")
            width(100%)
            padding_xy(0.25, 0.25)
            background_color([0.2, 0.4, 0.6, 0.1]) // Subtle debug blue
        }

        // Items are attached manually to "assets_content_area"
        // by InspectorSystemStopgapMmsAdapter.
    }
}
