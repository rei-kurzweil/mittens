// assets_content.mms — content area factory (=^･ω･^=)

import { asset_item } from "./asset_item.mms"

export fn assets_content(items) {
    return T {
        name = "assets_content_area"
        Style {
            height(74.0)
            overflow("scroll")
            background_color = [0.1, 0.1, 0.1, 1.0]
        }
        
        // Wrap items in a container that takes full width
        T {
            name = "items_container"
            Style {
                width(100%)
            }
            
            for item in items {
                asset_item(item)
            }
        }
    }
}
