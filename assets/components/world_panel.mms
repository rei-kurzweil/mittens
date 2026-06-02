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
import { world_panel_status } from "./world_panel_status.mms"

let WORLD_PANEL_WIDTH_GU = 29.5
let PATH_BAR_HEIGHT_GU = 2.5
let TITLE_BAR_HEIGHT_GU = 3.0
let STATUS_BAR_HEIGHT_GU = 2.5
let GAP_GU = 0.5
let WORLD_PANEL_CONTENT_HEIGHT_GU = 51.0
let WORLD_PANEL_TOTAL_HEIGHT_GU = PATH_BAR_HEIGHT_GU + GAP_GU + TITLE_BAR_HEIGHT_GU + GAP_GU + WORLD_PANEL_CONTENT_HEIGHT_GU + GAP_GU + STATUS_BAR_HEIGHT_GU
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
            vertical_align("middle")
            background_color([0.10, 0.55, 0.18, 1.0])
            color = [0.75, 1.00, 0.45, 1.0]
        }
        T.position(0.0, 0.0, 0.0) {
            Text { label }
        }
    }
    return root
}

export fn world_panel(title, items, title_color, panel_background_color, item_background_color, working_file_path) {
    let save_button = panel_button("save_button", "Save")
    let load_button = panel_button("load_button", "Load")
    let content = world_panel_content(items, item_background_color)
    let status = world_panel_status("idle")

    let panel = T {
        name = "world_panel_root"
        Style {
            display("block")
            width(WORLD_PANEL_WIDTH_GU)
            height(WORLD_PANEL_TOTAL_HEIGHT_GU)
            margin_xy(0.5, 0.5)
        }

        T {
            name = "path_input_wrap"
            Style {
                display("block")
                height(PATH_BAR_HEIGHT_GU)
                margin_bottom(GAP_GU)
                padding_xy(0.25, 0.45)
                vertical_align("middle")
                background_color([0.2, 0.01, 0.18, 0.8])
                background_z(-0.01)
            }
            TextInput {
                name = "path_input"
                working_file_path
            }
        }

        T {
            name = "title_bar"
            Style {
                display("block")
                height(TITLE_BAR_HEIGHT_GU)
                margin_bottom(GAP_GU)
                background_color(panel_background_color)
                background_z(-0.01)
            }

            T {
                name = "title_label_wrap"
                Style {
                    display("inline-block")
                    width(TITLE_LABEL_WIDTH_GU)
                    height(TITLE_BAR_HEIGHT_GU)
                    padding_xy(0.25, TITLE_LABEL_PADDING_X_GU)
                    text_align("left")
                    vertical_align("middle")
                    color = title_color
                }
                T.position(0.0, 0.0, 0.0) {
                    Text {
                        name = "title_label"
                        title
                    }
                }
            }

            save_button
            load_button
        }

        T {
            name = "content_slot"
            Style {
                display("block")
                height(WORLD_PANEL_CONTENT_HEIGHT_GU)
                margin_bottom(GAP_GU)
                overflow("scroll")
                background_color([0.96, 0.92, 0.18, 0.80])
                background_z(-0.001)
            }
            content
        }

        T {
            name = "save_status_wrap"
            Style {
                display("block")
                height(STATUS_BAR_HEIGHT_GU)
                padding_xy(0.25, 0.45)
                text_align("left")
                vertical_align("middle")
                background_color([0.08, 0.24, 0.11, 0.92])
                background_z(-0.01)
                color = [0.92, 1.00, 0.92, 1.0]
            }
            status
        }
    }

    return panel
}
