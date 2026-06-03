// asset_item.mms — individual asset item factory (=^･ω･^=)

export fn asset_item(label, item_background_color) {
    return T {
        name = "asset_item"
        Style {
            display("inline-block")
            width(8.0)
            height(5.0)
            margin(0.5)
            background_color(item_background_color)
            background_z(0.001)
            font_size(0.6)
            word_wrap("break-all")
        }
        // Text label positioned inside the item
        T.position(0.2, 0.2, 0.005) {
            Text {
                label
                C.rgba(0.0, 0.0, 0.0, 1.0)
            }
        }
    }
}
