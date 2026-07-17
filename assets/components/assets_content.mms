// assets_content.mms — content area factory (=^･ω･^=)

import { asset_item } from "./asset_item.mms"

export fn assets_content(items, item_background_color) {
    return T {
        name = "assets_content_area"
        id = "assets_content_area"
        Selection { name = "assets_selection" }
        Style {
            display("block")
            width(100%)
            padding_xy(0.25, 0.25)
        }

        // Items are attached manually to "assets_content_area"
        // by InspectorSystemStopgapMmsAdapter.
    }
}
