// asset_item.mms — individual asset item factory (=^･ω･^=)

export fn asset_item(label, item_background_color) {
    return T {
        name = "asset_item"
        Style {
            display("inline-block")
            width(8.5)
            height(6.5)
            margin(0.5)
            background_color(item_background_color)
            background_z(0.001)
            font_size(0.6)
            word_wrap("break-all")
        }

        // Preview rendering area
        T {
            name = "preview_slot"
            id = "preview_slot"
            T.position(4.25, 3.5, 0.05) { }
        }

        // Text label positioned inside the item (at the bottom now)
        T.position(0.2, 0.2, 0.005) {
            Text {
                label
                C.rgba(0.0, 0.0, 0.0, 1.0)
            }
        }
    }
}
