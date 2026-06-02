// assets/components/paint_panel_item.mms — individual paint tool item factory (=^･ω･^=)

export fn paint_panel_item(label, item_background_color, title_color) {
    return T {
        name = "paint_panel_item"
        Raycastable.enabled()
        Style {
            display("inline-block")
            width(7.0)
            height(7.5)
            margin(0.4)
            background_color(item_background_color)
            text_align("center")
            vertical_align("middle")
        }
        // Container to stack text and label
        T {
            Style {
                display("block")
                width(100%)
            }
            T {
                Style {
                    display("block")
                    margin_bottom(0.2)
                    margin_top(0.5)
                }
                T {
                    Style {
                        font_size(1)
                        color = title_color
                    }
                    Text { label }
                }
            }
        }
    }
}
