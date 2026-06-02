// assets_content.mms — content area factory (=^･ω･^=)

import { asset_item } from "./asset_item.mms"

export fn assets_content(items, item_background_color) {
    return T {
        name = "assets_content_area"
        Style {
            width(100%)
            padding_xy(0.25, 0.25)
        }

        for item in items {
            asset_item(item, item_background_color)
        }
    }
}
