// asset_item.mms — individual asset item factory (=^･ω･^=)

export fn asset_item(label, item_background_color) {
    return T {
        name = "asset_item"
        Style {
            display("inline-block")
            width(18.0)
            height(20.0)
            margin(1.0)
            background_color(item_background_color)
            background_z(0.001)
            font_size(1.0)
        }
        // Text label positioned inside the item
        T.position(1.0, 1.0, 0.005) {
            Text {
                label
                C.rgba(0.0, 0.0, 0.0, 1.0)
            }
        }
    }
}
