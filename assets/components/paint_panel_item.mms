// assets/components/paint_panel_item.mms — individual paint tool item factory (=^･ω･^=)

export fn paint_panel_item(label, icon_factory) {
    return T {
        name = "paint_panel_item"
        Raycastable.enabled()
        Style {
            display("inline-block")
            width(7.0)
            height(7.5)
            margin(0.4)
            background_color = [0.22, 0.22, 0.22, 1.0]
            text_align("center")
            vertical_align("middle")
        }
        // Container to stack text and icon
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
                Text {
                    label
                    Style { font_size(0.6) }
                    C.rgba(0.9, 0.9, 0.9, 1.0)
                }
            }
            T {
                Style {
                    display("block")
                    // Center the icon in the block
                    text_align("center")
                    vertical_align("middle")
                }
                // Call the factory to get the icon component
                icon_factory()
            }
        }
    }
}
