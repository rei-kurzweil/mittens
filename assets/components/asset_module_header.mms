// asset_module_header.mms — module separator for assets panel

export fn asset_module_header(module_name) {
    return T {
        name = "asset_module_header"
        Style {
            display("block")
            width(100%)
            height(2.5)
            margin_top(1.0)
            margin_bottom(0.5)
            background_color([0.1, 0.4, 0.1, 0.9]) // Greenish
            padding_xy(0.5, 0.45)
            vertical_align("middle")
        }
        T.position(0.0, 0.0, 0.0) {
            Text {
                module_name
                C.rgba(0.9, 1.0, 0.9, 1.0)
            }
        }
    }
}
