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
let INSPECTOR_PANEL_CONTENT_HEIGHT_GU = 54.0
let TITLE_BAR_HEIGHT_GU = 3.0
let TITLE_CONTENT_GAP_GU = 0.5
let INSPECTOR_PANEL_TOTAL_HEIGHT_GU = TITLE_BAR_HEIGHT_GU + TITLE_CONTENT_GAP_GU + INSPECTOR_PANEL_CONTENT_HEIGHT_GU
let TEXT_SCALE = 0.08
let TITLE_LABEL_PADDING_X_GU = 0.25

fn inspector_panel_row(label) {
    let row = T.position(0.0, 0.0, 0.1) {
        Style {
            margin_xy(0.25, 0.20)
            padding_xy(0.55, 0.45)
            background_color = [0.92, 0.92, 0.92, 0.80]
        }
        T.position(0.0, 0.0, 0.015).scale(TEXT_SCALE, TEXT_SCALE, TEXT_SCALE) {
            Text {
                label
                C.rgba(0.0, 0.0, 0.0, 1.0)
                EM.on()
            }
        }
    }
    return row
}

export fn inspector_panel(title, items) {
    let root = T {
        name = "inspector_panel_root"

        LayoutRoot {
            name = "panel_layout_root"
            available_width(INSPECTOR_PANEL_WIDTH_GU)
            available_height(INSPECTOR_PANEL_TOTAL_HEIGHT_GU)
            unit_scale(TEXT_SCALE)

            T {
                name = "title_bar"
                Style {
                    height(TITLE_BAR_HEIGHT_GU)
                    margin_bottom(TITLE_CONTENT_GAP_GU)
                    background_color = [0.18, 0.78, 0.22, 0.95]
                }

                T {
                    name = "title_label_wrap"
                    Style {
                        width(100%)
                        height(TITLE_BAR_HEIGHT_GU)
                        padding_xy(0.0, TITLE_LABEL_PADDING_X_GU)
                        color = [0.90, 1.00, 0.92, 1.0]
                    }
                    T.position(0.0, 0.0, 0.015) {
                        T.scale(TEXT_SCALE, TEXT_SCALE, TEXT_SCALE) {
                            Text {
                                name = "title_label"
                                title
                            }
                        }
                    }
                }
            }

            T {
                name = "content_slot"
                Style {
                    height(INSPECTOR_PANEL_CONTENT_HEIGHT_GU)
                    overflow("scroll")
                    background_color = [0.96, 0.92, 0.18, 0.80]
                }

                T {
                    name = "rows_mount"
                    Style {
                        width(100%)
                    }

                    for item in items {
                        inspector_panel_row(item)
                    }
                }
            }
        }
    }

    return root
}