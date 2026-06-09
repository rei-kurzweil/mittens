// panels.mms — panel factories (=^･ω･^=)
//
// Consolidates all full-size panel components (inspector_panel, world_panel,
// paint_panel) that were previously split across individual files.

import { paint_panel_item } from "./panel_items.mms"
import { world_panel_content } from "./panel_items.mms"
import { world_panel_status } from "./panel_items.mms"
import { inspector_panel_content } from "./panel_items.mms"
import { assets_content } from "./assets_content.mms"
import { pencil_icon } from "./icons.mms"
import { line_icon } from "./icons.mms"
import { spray_can_icon } from "./icons.mms"
import { fill_icon } from "./icons.mms"
import { erase_icon } from "./icons.mms"

// ── Shared constants ──────────────────────────────────────────────────────────

let TITLE_BAR_HEIGHT_GU = 3.0
let TITLE_CONTENT_GAP_GU = 0.5
let TITLE_LABEL_PADDING_X_GU = 0.25

// ── paint_panel ───────────────────────────────────────────────────────────────

let PAINT_PANEL_WIDTH_GU = 41.0
let PAINT_PANEL_STATUS_BAR_HEIGHT_GU = 4.0
let PAINT_PANEL_CONTENT_STATUS_GAP_GU = 0.5
let PAINT_PANEL_CONTENT_HEIGHT_GU = 8.5
let PAINT_PANEL_TOTAL_HEIGHT_GU = TITLE_BAR_HEIGHT_GU + TITLE_CONTENT_GAP_GU + PAINT_PANEL_CONTENT_HEIGHT_GU + PAINT_PANEL_CONTENT_STATUS_GAP_GU + PAINT_PANEL_STATUS_BAR_HEIGHT_GU

let tool_names = ["Free Draw", "Line", "Spray Can", "Fill", "Erase"]

fn tool_at_index(idx) {
    if idx == 0 { return pencil_icon() }
    if idx == 1 { return line_icon() }
    if idx == 2 { return spray_can_icon() }
    if idx == 3 { return fill_icon() }
    if idx == 4 { return erase_icon() }
    return T {}
}

export fn paint_panel(title, title_color, panel_background_color, item_background_color) {
    return T {
        name = "paint_panel_root"
        Style {
            display("block")
            width(PAINT_PANEL_WIDTH_GU)
            height(PAINT_PANEL_TOTAL_HEIGHT_GU)
            margin_xy(0.5, 0.5)
        }

        T {
            name = "title_bar"
            Raycastable.enabled()
            Style {
                display("block")
                height(TITLE_BAR_HEIGHT_GU)
                margin_bottom(TITLE_CONTENT_GAP_GU)
                padding_xy(0.5, 0.5)
                color = title_color
                background_color(panel_background_color)
                text_align("left")
                vertical_align("middle")
                background_z(-0.01)
            }
            T.position(0.0, 0.0, 0.0) {
                Text { title }
            }
        }

        T {
            name = "content_slot"
            Style {
                display("block")
                height(PAINT_PANEL_CONTENT_HEIGHT_GU)
                margin_bottom(PAINT_PANEL_CONTENT_STATUS_GAP_GU)
                background_color([0.96, 0.92, 0.18, 0.80])
                background_z(-0.001)
                padding(0.5)
            }

            let idx = 0
            for name in tool_names {
                paint_panel_item(name, tool_at_index(idx), item_background_color, title_color)
                idx = idx + 1
            }

            Selection {
                name = "paint_tool_selection"
                payload_selector = "[name='paint_panel_payload']"
            }
        }

        T {
            name = "paint_status_wrap"
            Style {
                display("block")
                height(PAINT_PANEL_STATUS_BAR_HEIGHT_GU)
                width(100%)
                padding_xy(0.25, 0.45)
                text_align("left")
                vertical_align("middle")
                word_wrap("normal")
                background_color([0.08, 0.24, 0.11, 0.92])
                background_z(-0.01)
            }
            T {
                name = "paint_panel_status_root"
                T.position(0.0, 0.0, 0.0) {
                    Text {
                        name = "paint_panel_status_value"
                        "paint inactive: no asset selected"
                    }
                }
            }
        }
    }
}

// ── world_panel ───────────────────────────────────────────────────────────────

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
            Raycastable.enabled()
            Style {
                display("block")
                height(PATH_BAR_HEIGHT_GU)
                width(100%)
                margin_bottom(GAP_GU)
                padding_xy(0.25, 0.45)
                vertical_align("middle")
                word_wrap("normal")
                word_wrap_tokens(["/"])
                background_color([0.2, 0.01, 0.18, 0.8])
                background_z(-0.01)
            }
            T.position(0.0, 0.0, 0.0) {
                TextInput {
                    name = "path_input"
                    working_file_path
                }
            }
        }

        T {
            name = "title_bar"
            Raycastable.enabled()
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
            Raycastable.enabled()
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
            Raycastable.enabled()
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

// ── inspector_panel ───────────────────────────────────────────────────────────

let INSPECTOR_PANEL_WIDTH_GU = 44.0
let INSPECTOR_PANEL_CONTENT_HEIGHT_GU = 57.0
let INSPECTOR_PANEL_TOTAL_HEIGHT_GU = TITLE_BAR_HEIGHT_GU + TITLE_CONTENT_GAP_GU + INSPECTOR_PANEL_CONTENT_HEIGHT_GU
let INSPECTOR_PANEL_SPLIT_GAP_GU = 0.5
let INSPECTOR_PANEL_SIDEBAR_WIDTH_GU = 15.0
let INSPECTOR_PANEL_DETAIL_WIDTH_GU = INSPECTOR_PANEL_WIDTH_GU - INSPECTOR_PANEL_SIDEBAR_WIDTH_GU - INSPECTOR_PANEL_SPLIT_GAP_GU
let INSPECTOR_PANEL_PIN_SLOT_WIDTH_GU = 5.0
let INSPECTOR_PANEL_TITLE_TEXT_WIDTH_GU = INSPECTOR_PANEL_WIDTH_GU - INSPECTOR_PANEL_PIN_SLOT_WIDTH_GU

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
            Raycastable.enabled()
            Style {
                display("block")
                height(TITLE_BAR_HEIGHT_GU)
                margin_bottom(TITLE_CONTENT_GAP_GU)
                background_color(panel_background_color)
                background_z(-0.01)
            }

            T {
                name = "title_text_slot"
                Style {
                    display("inline-block")
                    width(INSPECTOR_PANEL_TITLE_TEXT_WIDTH_GU)
                    height(TITLE_BAR_HEIGHT_GU)
                    padding_xy(0.0, TITLE_LABEL_PADDING_X_GU)
                    font_size(1)
                    vertical_align("middle")
                    color = title_color
                }
                T {
                    name = "title_label_wrap"
                    Style {
                        display("block")
                        width(100%)
                        height(TITLE_BAR_HEIGHT_GU)
                        vertical_align("middle")
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
                name = "pin_slot"
                Style {
                    display("inline-block")
                    width(INSPECTOR_PANEL_PIN_SLOT_WIDTH_GU)
                    height(TITLE_BAR_HEIGHT_GU)
                    vertical_align("middle")
                }
            }
        }

        T {
            name = "content_slot"
            Raycastable.enabled()
            Style {
                display("block")
                height(INSPECTOR_PANEL_CONTENT_HEIGHT_GU)
                background_color([0.96, 0.92, 0.18, 0.80])
                background_z(-0.01)
            }

            T {
                name = "content_area"
                Style {
                    display("block")
                    width(100%)
                    height(100%)
                    font_size(1)
                }

                T {
                    name = "sidebar_slot"
                    Style {
                        display("inline-block")
                        width(40%)
                        height(100%)
                        margin_right(1gu)
                        font_size(1)
                        background_color([0.9, 0.9, 0.9, 0.95])
                        background_z(-0.005)
                        overflow("scroll")
                    }
                    inspector_panel_content(items, item_background_color)
                }

                T {
                    name = "detail_slot"
                    Style {
                        display("inline-block")
                        width(60%)
                        height(100%)
                        font_size(1)
                        background_color([0.26, 0.26, 0.26, 0.95])
                        background_z(-0.005)
                        overflow("scroll")
                    }
                }
            }
        }
    }

    return root
}

// ── asset_panel ───────────────────────────────────────────────────────────────

let ASSET_PANEL_WIDTH_GU = 39.0
let ASSET_PANEL_CONTENT_HEIGHT_GU = 57.0
let ASSET_PANEL_TOTAL_HEIGHT_GU = TITLE_BAR_HEIGHT_GU + TITLE_CONTENT_GAP_GU + ASSET_PANEL_CONTENT_HEIGHT_GU

export fn asset_panel(title, items, title_color, panel_background_color, item_background_color) {
    return T {
        name = "assets_root"
        Style {
            display("block")
            width(ASSET_PANEL_WIDTH_GU)
            height(ASSET_PANEL_TOTAL_HEIGHT_GU)
            margin_xy(0.5, 0.5)
        }

        T {
            name = "title_bar"
            Raycastable.enabled()
            Style {
                display("block")
                height(TITLE_BAR_HEIGHT_GU)
                margin_bottom(TITLE_CONTENT_GAP_GU)
                background_color(panel_background_color)
                background_z(-0.01)
                font_size(1)
                color = title_color
                padding_xy(0.5, 0.5)
                text_align("left")
                vertical_align("middle")
            }
            T.position(0.0, 0.0, 0.0) {
                Text { title }
            }
        }

        T {
            name = "content_slot"
            Raycastable.enabled()
            Style {
                display("block")
                height(ASSET_PANEL_CONTENT_HEIGHT_GU)
                overflow("scroll")
                background_color([0.96, 0.92, 0.18, 0.80])
                background_z(-0.001)
            }
            assets_content(items, item_background_color)
        }
    }
}
