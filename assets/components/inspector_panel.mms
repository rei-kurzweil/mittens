// assets/components/inspector_panel.mms
//
// Reusable Inspector panel factory for MMS-driven panel rendering.
//
// `inspector_panel(title, items)` returns a panel root with:
// - named title bar nodes
// - a named `rows_mount` container
// - one visible row per entry in `items`
//
// v1 item contract: `items` is an array of display strings.

let INSPECTOR_PANEL_WIDTH_GU = 22.0
let INSPECTOR_PANEL_CONTENT_HEIGHT_GU = 57.0
let TITLE_BAR_HEIGHT_GU = 3.0
let TITLE_CONTENT_GAP_GU = 0.5
let INSPECTOR_PANEL_TOTAL_HEIGHT_GU = TITLE_BAR_HEIGHT_GU + TITLE_CONTENT_GAP_GU + INSPECTOR_PANEL_CONTENT_HEIGHT_GU
let TITLE_LABEL_PADDING_X_GU = 0.25

fn inspector_panel_row(label, item_background_color) {
    let row = T {
        name = "inspector_panel_row"
        Style {
            display("block")
            width(100%)
            margin_xy(0.25, 0.20)
            padding_xy(0.55, 0.45)
            font_size(1)
            word_wrap("normal")
            background_color = item_background_color
            background_z(-0.01)
        }
        T {
            Style {
                display("block")
                width(100%)
            }
            Text {
                label
                C.rgba(0.0, 0.0, 0.0, 1.0)
                EM.on()
            }
        }
    }
    return row
}

export fn inspector_panel_content(items, item_background_color) {
    let root = T {
        name = "inspector_panel_content_root"
        Style {
            display("block")
            width(100%)
        }

        T {
            name = "rows_mount"
            Style {
                width(100%)
            }

            for item in items {
                inspector_panel_row(item, item_background_color)
            }
        }
    }

    return root
}

export fn inspector_panel(title, items, title_color, panel_background_color, item_background_color) {
    let root = T {
        name = "inspector_panel_root"
        Style {
            display("block")
            width(INSPECTOR_PANEL_WIDTH_GU)
            height(INSPECTOR_PANEL_TOTAL_HEIGHT_GU)
            margin_xy(0.5, 0.5)
        }

        T {
            name = "title_bar"
            Style {
                display("block")
                height(TITLE_BAR_HEIGHT_GU)
                margin_bottom(TITLE_CONTENT_GAP_GU)
                background_color(panel_background_color)
                background_z(-0.01)
            }

            T {
                name = "title_label_wrap"
                Style {
                    display("block")
                    width(100%)
                    height(TITLE_BAR_HEIGHT_GU)
                    padding_xy(0.0, TITLE_LABEL_PADDING_X_GU)
                    font_size(1)
                    vertical_align("middle")
                    color = title_color
                }
                T.position(0.0, 0.0, 0.015) {
                    Text {
                        name = "title_label"
                        title
                    }
                }
            }
        }

        T {
            name = "content_slot"
            Style {
                display("block")
                height(INSPECTOR_PANEL_CONTENT_HEIGHT_GU)
                overflow("scroll")
                background_color([0.96, 0.92, 0.18, 0.80])
                background_z(-0.01)
            }

            inspector_panel_content(items, item_background_color)
        }
    }

    return root
}
