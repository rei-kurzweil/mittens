// asset_item.mms — individual asset item factory (=^･ω･^=)

export fn asset_item(name) {
    return T {
        name = "asset_item"
        Style {
            display("inline-block")
            width(18.0)
            height(20.0)
            margin(1.0)
            background_color = [0.25, 0.25, 0.25, 1.0]
        }
        // Text label positioned inside the item
        T.position(1.0, 1.0, 0.05).scale(0.08, 0.08, 0.08) {
            Text {
                name
                C.rgba(0.9, 0.9, 0.9, 1.0)
            }
        }
    }
}
