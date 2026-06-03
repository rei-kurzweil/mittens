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
            font_size(0.6gu)
            word_wrap("break-all")
        }

        // Preview rendering area
        T {
            name = "preview_slot"
            id = "preview_slot"
            Style {
                display("block")
                width(8.5)
                height(5.0)
                text_align("center")
                vertical_align("middle")
            }
        }

        // Text label positioned inside the item
        T {
            Style {
                display("block")
                width(8.5)
                text_align("center")
                word_wrap("break-all")
            }
            Text {
                label
                C.rgba(0.0, 0.0, 0.0, 1.0)
            }
        }
    }
}
