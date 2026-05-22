// assets/components/world_panel.mms
//
// Reusable World panel shell for MMS-driven panel rendering.
//
// `world_panel(title, items)` returns a panel root with:
// - named title bar nodes
// - named save/load buttons
// - named status text
// - a named `content_slot`
// - a nested `world_panel_content(items)` subtree for the rerenderable body

import { world_panel_content } from "./world_panel_content.mms"

let WORLD_PANEL_WIDTH_GU = 29.5
let WORLD_PANEL_CONTENT_HEIGHT_GU = 54.0
let TITLE_BAR_HEIGHT_GU = 3.0
let TITLE_CONTENT_GAP_GU = 0.5
let WORLD_PANEL_TOTAL_HEIGHT_GU = TITLE_BAR_HEIGHT_GU + TITLE_CONTENT_GAP_GU + WORLD_PANEL_CONTENT_HEIGHT_GU
let TEXT_SCALE = 0.08
let TITLEBAR_BUTTON_WIDTH_GU = 6.875
let TITLEBAR_BUTTON_HEIGHT_GU = 2.4
let TITLEBAR_BUTTON_MARGIN_TOP_BOTTOM_GU = 0.3
let TITLEBAR_BUTTON_MARGIN_LEFT_GU = 0.625
let TITLE_LABEL_WIDTH_GU = 14.5
let TITLE_LABEL_PADDING_X_GU = 0.25

fn panel_button(node_name, label) {
    let root = T {
        name = node_name
        Raycastable.enabled()
        Style {
            display("inline-block")
            width(TITLEBAR_BUTTON_WIDTH_GU)
            height(TITLEBAR_BUTTON_HEIGHT_GU)
            margin_top(TITLEBAR_BUTTON_MARGIN_TOP_BOTTOM_GU)
            margin_bottom(TITLEBAR_BUTTON_MARGIN_TOP_BOTTOM_GU)
            margin_left(TITLEBAR_BUTTON_MARGIN_LEFT_GU)
            padding_xy(0.0, 0.45)
            text_align("center")
            background_color = [0.10, 0.55, 0.18, 1.0]
            background_z(0.02)
            color = [0.75, 1.00, 0.45, 1.0]
        }
        T.position(0.0, 0.0, 0.05) {
            T.scale(TEXT_SCALE, TEXT_SCALE, TEXT_SCALE) {
                Text { label }
            }
        }
    }
    return root
}

export fn world_panel(title, items) {
    let save_button = panel_button("save_button", "Save")
    let load_button = panel_button("load_button", "Load")
    let content = world_panel_content(items)

    let root = T {
        name = "world_panel_root"

        T.position(0.02, 0.6, 0.05) {
            name = "save_status_wrap"
            T.position(0.0, 0.0, 0.015).scale(TEXT_SCALE, TEXT_SCALE, TEXT_SCALE) {
                Text {
                    name = "panel_status_value"
                    "idle"
                    C.rgba(0.92, 1.00, 0.92, 1.0)
                }
            }
        }

        LayoutRoot {
            name = "panel_layout_root"
            available_width(WORLD_PANEL_WIDTH_GU)
            available_height(WORLD_PANEL_TOTAL_HEIGHT_GU)
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
                        display("inline-block")
                        width(TITLE_LABEL_WIDTH_GU)
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

                save_button
                load_button
            }

            T {
                name = "content_slot"
                Style {
                    height(WORLD_PANEL_CONTENT_HEIGHT_GU)
                    overflow("scroll")
                    background_color = [0.96, 0.92, 0.18, 0.80]
                }

                content
            }

        }
    }

    return root
}