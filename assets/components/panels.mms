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
import { grid_tool_icon } from "./icons.mms"
import { grid_visibility_icon } from "./icons.mms"


// ── Shared constants ──────────────────────────────────────────────────────────

let TITLE_BAR_HEIGHT_GU = 3.0
let TITLE_CONTENT_GAP_GU = 0.5
let TITLE_LABEL_PADDING_X_GU = 0.25
let SETTINGS_PANEL_WIDTH_GU = 16.0

fn editor_settings_mode_row(row_name, label, mode_value) {
    return T {
        name = row_name
        Option {
            Data {
                name = "editor_settings_payload"
                row_name = row_name
                label = label
                mode_value = mode_value
                row_kind = "EditorMode"
                interactive = true
            }
        }
        Raycastable.click_only()
        Style {
            display("block")
            width(100%)
            margin_xy(0.25, 0.20)
            padding_xy(0.55, 0.45)
            color([0, 0, 0, 1.0])
            background_color([0.92, 0.97, 0.92, 1.0])
            background_z(-0.01)
            text_align("left")
            vertical_align("middle")
        }
        T {
            Text { label }
        }
    }
}

fn editor_settings_armature_row() {
    return T {
        name = "editor_settings_armature_visibility"
        Data {
            name = "editor_settings_payload"
            row_kind = "GLTFArmatureVisibility"
            visible = true
            interactive = true
        }
        Raycastable.click_only()
        Toggle.off()
        Style {
            display("block")
            width(100%)
            margin_xy(0.25, 0.20)
            padding_xy(0.55, 0.45)
            color([0, 0, 0, 1.0])
            background_color([0.92, 0.97, 0.92, 1.0])
            background_z(-0.01)
            text_align("left")
            vertical_align("middle")
        }
        T {
            Style {
                display("block")
            }
            T {
                Style {
                    display("inline-block")
                }
                Text { "show armature" }
            }
            T {
                name = "armature_toggle_slot"
                Style {
                    display("inline-block")
                    margin_left(0.65)
                }
            }
        }
    }
}

fn editor_settings_bounds_row() {
    return T {
        name = "editor_settings_bounds_visibility"
        Data {
            name = "editor_settings_payload"
            row_kind = "GLTFBoundsVisibility"
            visible = false
            interactive = true
        }
        Raycastable.click_only()
        Toggle.off()
        Style {
            display("block")
            width(100%)
            margin_xy(0.25, 0.20)
            padding_xy(0.55, 0.45)
            color([0, 0, 0, 1.0])
            background_color([0.92, 0.97, 0.92, 1.0])
            background_z(-0.01)
            text_align("left")
            vertical_align("middle")
        }
        T {
            Style { display("block") }
            T {
                Style { display("inline-block") }
                Text { "show bounds" }
            }
            T {
                name = "bounds_toggle_slot"
                Style {
                    display("inline-block")
                    margin_left(0.65)
                }
            }
        }
    }
}

fn editor_settings_collider_row(row_name, label, row_kind, slot_name) {
    return T {
        name = row_name
        Data {
            name = "editor_settings_payload"
            row_kind = row_kind
            interactive = true
        }
        Raycastable.click_only()
        Toggle.off()
        Style {
            display("block")
            width(100%)
            margin_xy(0.25, 0.20)
            padding_xy(0.55, 0.45)
            color([0, 0, 0, 1.0])
            background_color([0.92, 0.97, 0.92, 1.0])
            background_z(-0.01)
            text_align("left")
            vertical_align("middle")
        }
        T {
            Style { display("block") }
            T {
                Style { display("inline-block") }
                Text { label }
            }
            T {
                name = slot_name
                Style { display("inline-block") margin_left(0.65) }
            }
        }
    }
}

export fn editor_settings_panel(title, title_color, panel_background_color, config) {
    return T {
        name = "editor_settings_panel_root"
        Style {
            display("block")
            width(SETTINGS_PANEL_WIDTH_GU)
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
                background_color([0.96, 0.92, 0.18, 0.80])
                background_z(-0.001)
                padding(0.25)
            }

            T {
                name = "editor_settings_mode_rows"
                Style {
                    display("block")
                }
                editor_settings_mode_row("editor_settings_mode_select", "Select", "select")
                editor_settings_mode_row("editor_settings_mode_cursor_3d", "3D Cursor", "cursor_3d")
                editor_settings_mode_row("editor_settings_mode_select_cursor", "Select + Cursor", "select_cursor")
            }
            if config.show_armature { editor_settings_armature_row() }
            if config.show_bounds { editor_settings_bounds_row() }
            if config.show_colliders {
                editor_settings_collider_row("editor_settings_colliders_visibility", "show all colliders", "AllCollidersVisibility", "colliders_toggle_slot")
            }
            if config.show_gltf_colliders {
                editor_settings_collider_row("editor_settings_gltf_colliders_visibility", "show GLTF colliders", "GltfCollidersVisibility", "gltf_colliders_toggle_slot")
            }

            Selection.root("#editor_settings_mode_rows") { name = "editor_settings_selection" }
        }
    }
}

// ── paint_panel ───────────────────────────────────────────────────────────────

let PAINT_PANEL_WIDTH_GU = 26.0
let PAINT_PANEL_STATUS_BAR_HEIGHT_GU = 4.0
let PAINT_PANEL_CONTENT_STATUS_GAP_GU = 0.5
let PAINT_PANEL_CONTENT_HEIGHT_GU = 8.5
let PAINT_PANEL_TOTAL_HEIGHT_GU = TITLE_BAR_HEIGHT_GU + TITLE_CONTENT_GAP_GU + PAINT_PANEL_CONTENT_HEIGHT_GU + PAINT_PANEL_CONTENT_STATUS_GAP_GU + PAINT_PANEL_STATUS_BAR_HEIGHT_GU
let COLOR_PANEL_WIDTH_GU = 16.0
let COLOR_PANEL_HEIGHT_GU = 18.5

let tool_names = ["Free Draw", "Grid Tool", "Line", "Spray Can", "Fill", "Erase"]

fn tool_at_index(idx) {
    if idx == 0 { return pencil_icon() }
    if idx == 1 { return grid_tool_icon() }
    if idx == 2 { return line_icon() }
    if idx == 3 { return spray_can_icon() }
    if idx == 4 { return fill_icon() }
    if idx == 5 { return erase_icon() }
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

            Selection { name = "paint_tool_selection" }
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

fn color_swatch(name, label, idx, rgba, rgba_text) {
    return T {
        name = name
        Option {
            Data {
                name = "color_swatch_payload"
                row_kind = "ColorSwatch"
                label = label
                index = idx
                rgba = rgba_text
                interactive = true
            }
        }
        Raycastable.click_only()
        Style {
            display("inline-block")
            width(2.5)
            height(2.5)
            margin_xy(0.2, 0.2)
            background_color(rgba)
            background_z(-0.01)
        }
    }
}

export fn color_panel(title, title_color, panel_background_color) {
    return T {
        name = "color_panel_root"
        Style {
            display("block")
            width(COLOR_PANEL_WIDTH_GU)
            height(COLOR_PANEL_HEIGHT_GU)
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
            T.position(0.0, 0.0, 0.0) { Text { title } }
        }
        T {
            name = "content_slot"
            Style {
                display("block")
                background_color([0.96, 0.92, 0.18, 0.80])
                background_z(-0.001)
                padding(0.4)
            }
            color_swatch("swatch_0", "Snow", 0, [0.96, 0.95, 0.90, 1.0], "0.96,0.95,0.90,1.0")
            color_swatch("swatch_1", "Apricot", 1, [0.96, 0.75, 0.46, 1.0], "0.96,0.75,0.46,1.0")
            color_swatch("swatch_2", "Tomato", 2, [0.91, 0.34, 0.25, 1.0], "0.91,0.34,0.25,1.0")
            color_swatch("swatch_3", "Berry", 3, [0.66, 0.17, 0.24, 1.0], "0.66,0.17,0.24,1.0")
            color_swatch("swatch_4", "Sun", 4, [0.95, 0.84, 0.22, 1.0], "0.95,0.84,0.22,1.0")
            color_swatch("swatch_5", "Moss", 5, [0.63, 0.74, 0.24, 1.0], "0.63,0.74,0.24,1.0")
            color_swatch("swatch_6", "Leaf", 6, [0.24, 0.58, 0.27, 1.0], "0.24,0.58,0.27,1.0")
            color_swatch("swatch_7", "Pine", 7, [0.12, 0.31, 0.19, 1.0], "0.12,0.31,0.19,1.0")
            color_swatch("swatch_8", "Sky", 8, [0.46, 0.78, 0.94, 1.0], "0.46,0.78,0.94,1.0")
            color_swatch("swatch_9", "Ocean", 9, [0.19, 0.52, 0.80, 1.0], "0.19,0.52,0.80,1.0")
            color_swatch("swatch_10", "Ink", 10, [0.14, 0.25, 0.50, 1.0], "0.14,0.25,0.50,1.0")
            color_swatch("swatch_11", "Night", 11, [0.10, 0.12, 0.24, 1.0], "0.10,0.12,0.24,1.0")
            color_swatch("swatch_12", "Lilac", 12, [0.78, 0.60, 0.87, 1.0], "0.78,0.60,0.87,1.0")
            color_swatch("swatch_13", "Rose", 13, [0.90, 0.45, 0.66, 1.0], "0.90,0.45,0.66,1.0")
            color_swatch("swatch_14", "Clay", 14, [0.61, 0.44, 0.31, 1.0], "0.61,0.44,0.31,1.0")
            color_swatch("swatch_15", "Char", 15, [0.16, 0.16, 0.16, 1.0], "0.16,0.16,0.16,1.0")
            Selection.root("#content_slot") { name = "color_panel_selection" }
        }
    }
}

// ── pose_capture_panel ────────────────────────────────────────────────────────

let POSE_PANEL_WIDTH_GU = 29.5
let POSE_PANEL_CONTENT_HEIGHT_GU = 51.0
let POSE_PANEL_STATUS_HEIGHT_GU = 2.5
let POSE_PANEL_TOTAL_HEIGHT_GU = TITLE_BAR_HEIGHT_GU + TITLE_CONTENT_GAP_GU + POSE_PANEL_CONTENT_HEIGHT_GU + TITLE_CONTENT_GAP_GU + POSE_PANEL_STATUS_HEIGHT_GU

export fn pose_capture_panel(title, title_color, panel_background_color) {
    return T {
        name = "pose_capture_panel_root"
        Style {
            display("block")
            width(POSE_PANEL_WIDTH_GU)
            height(POSE_PANEL_TOTAL_HEIGHT_GU)
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
                padding_xy(0.5, 0.5)
                color = title_color
                text_align("left")
                vertical_align("middle")
            }
            T.position(0.0, 0.0, 0.0) {
                Text { title }
            }
        }

        T {
            name = "pose_panel_content_area"
            Raycastable.enabled()
            Style {
                display("block")
                width(100%)
                height(POSE_PANEL_CONTENT_HEIGHT_GU)
                margin_bottom(TITLE_CONTENT_GAP_GU)
                overflow("scroll")
                background_color([0.96, 0.92, 0.18, 0.80])
                background_z(-0.001)
            }
            Selection.root("#content_slot") { name = "pose_capture_selection" }
            T {
                name = "content_slot"
                Style {
                    display("block")
                    width(100%)
                }
            }
        }

        T {
            name = "pose_panel_status_wrap"
            Raycastable.enabled()
            Style {
                display("block")
                height(POSE_PANEL_STATUS_HEIGHT_GU)
                padding_xy(0.25, 0.45)
                text_align("left")
                vertical_align("middle")
                background_color([0.08, 0.24, 0.11, 0.92])
                background_z(-0.01)
                color = [0.92, 1.00, 0.92, 1.0]
            }
            T.position(0.0, 0.0, 0.0) {
                Text {
                    name = "pose_panel_status_value"
                    "idle"
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
            name = "world_panel_content_area"
            Raycastable.enabled()
            Style {
                display("block")
                height(WORLD_PANEL_CONTENT_HEIGHT_GU)
                width(100%)
                margin_bottom(GAP_GU)
                overflow("scroll")
                background_color([0.96, 0.92, 0.18, 0.80])
                background_z(-0.001)
            }
            Selection.root("#content_slot") { name = "world_panel_selection" }
            T {
                name = "content_slot"
                Style {
                    display("block")
                    width(100%)
                }
            }
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
    let pin_button = panel_button("pin_button", "Pin")
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
                pin_button
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
                    name = "inspector_sidebar_area"
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
                    T {
                        name = "sidebar_slot"
                        Style {
                            display("block")
                            width(100%)
                        }
                    }
                    Selection.root("#sidebar_slot") { name = "inspector_panel_selection" }
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

// ── grid_panel ───────────────────────────────────────────────────────────────

let GRID_PANEL_WIDTH_GU = 29.5
let GRID_PANEL_CONTENT_HEIGHT_GU = 51.0
let GRID_PANEL_ADD_BUTTON_HEIGHT_GU = 3.0
let GRID_PANEL_TOTAL_HEIGHT_GU = TITLE_BAR_HEIGHT_GU + TITLE_CONTENT_GAP_GU + GRID_PANEL_CONTENT_HEIGHT_GU + GAP_GU + GRID_PANEL_ADD_BUTTON_HEIGHT_GU

export fn grid_panel(title, items, title_color, panel_background_color, item_background_color) {
    let _unused_items = items
    let _unused_item_background_color = item_background_color

    return T {
        name = "grid_panel_root"
        Style {
            display("block")
            width(GRID_PANEL_WIDTH_GU)
            height(GRID_PANEL_TOTAL_HEIGHT_GU)
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
                name = "title_label_wrap"
                Style {
                    display("inline-block")
                    width(23.0)
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

            T {
                name = "title_icon_wrap"
                Style {
                    display("inline-block")
                    width(5.0)
                    height(TITLE_BAR_HEIGHT_GU)
                    text_align("center")
                    vertical_align("middle")
                }
                T.position(0.0, 0.0, 0.0) {
                    T.scale(0.25, 0.25, 1.0) {
                        grid_visibility_icon()
                    }
                }
            }
        }

        T {
            name = "grid_panel_content_area"
            Raycastable.enabled()
            Style {
                display("block")
                height(GRID_PANEL_CONTENT_HEIGHT_GU)
                width(100%)
                margin_bottom(GAP_GU)
                overflow("scroll")
                background_color([0.96, 0.92, 0.18, 0.80])
                background_z(-0.001)
            }
            Selection.root("#content_slot").optional() { name = "grid_panel_selection" }
            T {
                name = "content_slot"
                Style {
                    display("block")
                    width(100%)
                }
            }
        }

        T {
            name = "grid_add_button"
            Raycastable.enabled()
            Style {
                display("block")
                height(GRID_PANEL_ADD_BUTTON_HEIGHT_GU)
                background_color([0.10, 0.55, 0.18, 1.0])
                background_z(-0.01)
            }

            T {
                name = "grid_add_button_label"
                Style {
                    display("inline-block")
                    width(24.0)
                    height(GRID_PANEL_ADD_BUTTON_HEIGHT_GU)
                    padding_xy(0.25, TITLE_LABEL_PADDING_X_GU)
                    text_align("left")
                    vertical_align("middle")
                    color = [0.75, 1.00, 0.45, 1.0]
                }
                T.position(0.0, 0.0, 0.0) {
                    Text { "Add Grid" }
                }
            }

        }
    }
}
