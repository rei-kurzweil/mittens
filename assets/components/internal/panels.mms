// panels.mms — panel factories (=^･ω･^=)
//
// Consolidates all full-size panel components (inspector_panel, world_panel,
// paint_panel) that were previously split across individual files.

import { paint_panel_item } from "./panel_items.mms"
import { world_panel_content } from "./panel_items.mms"
import { world_panel_status } from "./panel_items.mms"
import { inspector_panel_content } from "./panel_items.mms"
import { assets_content } from "./assets_content.mms"
import {
    pencil_icon,
    line_icon,
    spray_can_icon,
    color_icon,
    erase_icon,
    grid_tool_icon,
    grid_visibility_icon,
} from "../icons.mms"
import { accordion, accordion_body } from "./ui/accordion.mms"


// ── Shared constants ──────────────────────────────────────────────────────────

let TITLE_BAR_HEIGHT_GU = 3.0
let TITLE_CONTENT_GAP_GU = 0.5
let TITLE_LABEL_PADDING_X_GU = 0.25
let SETTINGS_PANEL_WIDTH_GU = 16.0
// Editor panel roots are positioned directly in world space. Their private
// accordion LayoutRoots therefore need the same GU -> WU conversion as the
// shared editor layout; nested LayoutRoots do not inherit unit_scale.
let EDITOR_PANEL_UNIT_SCALE = 0.08
let EDITOR_ACCORDION_TOGGLE_BACKGROUND = [0.95, 0.73, 0.16, 1.0]
let EDITOR_DARK_CHROME_BACKGROUND = [0.20, 0.21, 0.23, 1.0]
let EDITOR_DARK_CHROME_TEXT = [0.96, 0.97, 0.98, 1.0]

fn panel_title(title, title_color) {
    return T {
        name = "title_label_wrap"
        Style {
            display("flex")
            width(0.0)
            flex_grow(1.0)
            height(3.5)
            align_items("center")
            padding(1.0)
            color(title_color)
        }
        T.position(0.0, 0.0, 0.02) {
            Text { name = "title_label" title }
        }
    }
}

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
        Text { label }
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

fn editor_settings_visibility_row(row_name, label, row_kind, slot_name) {
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

fn editor_settings_camera_row() {
    return T {
        name = "editor_settings_cameras_visibility"
        Data {
            name = "editor_settings_payload"
            row_kind = "CameraVisibility"
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
        T { Style { display("inline-block") } Text { "show cameras" } }
        T {
            name = "cameras_toggle_slot"
            Style { display("inline-block") margin_left(0.65) }
        }
    }
}

export fn editor_settings_panel_body(config) {
    return accordion_body(T {
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
                editor_settings_mode_row("editor_settings_mode_paint", "Paint", "paint")
            }
            if config.show_armature { editor_settings_armature_row() }
            if config.show_bounds { editor_settings_bounds_row() }
            if config.show_cameras { editor_settings_camera_row() }
            if config.show_spring_bones {
                editor_settings_visibility_row("editor_settings_spring_bones_visibility", "show spring bones", "SpringBonesVisibility", "spring_bones_toggle_slot")
            }
            if config.show_zones {
                editor_settings_visibility_row("editor_settings_zones_visibility", "show zones", "ZonesVisibility", "zones_toggle_slot")
            }

            Selection.root("#editor_settings_mode_rows") { name = "editor_settings_selection" }
    })
}

export fn editor_settings_panel(title, title_color, panel_background_color, config) {
    return accordion({
        unlit = true
        root_name = "editor_settings_panel_root"
        width_gu = SETTINGS_PANEL_WIDTH_GU
        unit_scale = EDITOR_PANEL_UNIT_SCALE
        background_color = panel_background_color
        toggle_background_color = EDITOR_ACCORDION_TOGGLE_BACKGROUND
        children = [panel_title(title, title_color)]
        body = editor_settings_panel_body(config)
    })
}

// ── paint_panel ───────────────────────────────────────────────────────────────

let PAINT_PANEL_WIDTH_GU = 26.0
let PAINT_PANEL_STATUS_BAR_HEIGHT_GU = 4.0
let PAINT_PANEL_CONTENT_STATUS_GAP_GU = 0.5
let PAINT_PANEL_CONTENT_HEIGHT_GU = 8.5
let PAINT_PANEL_TOTAL_HEIGHT_GU = TITLE_BAR_HEIGHT_GU + TITLE_CONTENT_GAP_GU + PAINT_PANEL_CONTENT_HEIGHT_GU + PAINT_PANEL_CONTENT_STATUS_GAP_GU + PAINT_PANEL_STATUS_BAR_HEIGHT_GU
// Temporary explicit width: LayoutRoot does not yet accept an auto-sized inline
// constraint. This fits the active well, its separator, and eight swatches per
// row, keeping the 16-color palette to two rows.
let COLOR_PANEL_WIDTH_GU = 32.6

let tool_names = ["Free Draw", "Grid Tool", "Line", "Spray Can", "Color", "Erase"]

fn tool_at_index(idx) {
    if idx == 0 { return pencil_icon() }
    if idx == 1 { return grid_tool_icon() }
    if idx == 2 { return line_icon() }
    if idx == 3 { return spray_can_icon() }
    if idx == 4 { return color_icon() }
    if idx == 5 { return erase_icon() }
    return T {}
}

export fn paint_panel_body(item_background_color, title_color) {
    return accordion_body(T {
        name = "paint_panel_body"
        Style { display("block") width(100%) }
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

        // Snap is opt-in, and only has an effect when the current paint hit is
        // a grid.  Keeping the control here makes that rule discoverable.
        T {
            name = "paint_snap_settings"
            Style {
                display("block")
                width(100%)
                margin_bottom(PAINT_PANEL_CONTENT_STATUS_GAP_GU)
                padding_xy(0.5, 0.35)
                background_color([0.82, 0.84, 0.86, 0.90])
                background_z(-0.001)
                color([0, 0, 0, 1.0])
            }
            T { Style { display("inline-block") margin_right(0.5) } Text { "Snap?" } }
            T {
                Option { Data { name = "paint_snap_payload" label = "yes" interactive = true } }
                Raycastable.click_only()
                Style { display("inline-block") margin_right(0.35) padding_xy(0.4, 0.2) background_color([0.92, 0.97, 0.92, 1.0]) background_z(-0.01) }
                Text { "yes" }
            }
            T {
                Option { Data { name = "paint_snap_payload" label = "no" interactive = true } }
                Raycastable.click_only()
                Style { display("inline-block") padding_xy(0.4, 0.2) background_color([0.96, 0.90, 0.90, 1.0]) background_z(-0.01) }
                Text { "no" }
            }
            Selection { name = "paint_snap_selection" }
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
                background_color(EDITOR_DARK_CHROME_BACKGROUND)
                background_z(-0.01)
                color = EDITOR_DARK_CHROME_TEXT
            }
            T {
                name = "paint_panel_status_root"
                Text {
                    name = "paint_panel_status_value"
                    "paint inactive: no asset selected"
                }
            }
        }
    })
}

export fn paint_panel(title, title_color, panel_background_color, item_background_color) {
    return accordion({
        unlit = true
        root_name = "paint_panel_root"
        width_gu = PAINT_PANEL_WIDTH_GU
        unit_scale = EDITOR_PANEL_UNIT_SCALE
        background_color = panel_background_color
        toggle_background_color = EDITOR_ACCORDION_TOGGLE_BACKGROUND
        children = [panel_title(title, title_color)]
        body = paint_panel_body(item_background_color, title_color)
    })
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

export fn color_panel_body() {
    return accordion_body(T {
        name = "color_panel_body"
        Style { display("block") width(100%) }
        T {
            name = "content_slot"
            Style {
                display("block")
                background_color([0.96, 0.92, 0.18, 0.80])
                background_z(-0.001)
                padding(0.4)
            }
            // White is also EditorPaintSystem's deterministic initial color.
            T {
                name = "active_color_well"
                Style {
                    display("inline-block")
                    vertical_align("top")
                    width(5.4)
                    height(5.4)
                    margin_xy(0.2, 0.2)
                    background_color([1.0, 1.0, 1.0, 1.0])
                    background_z(-0.01)
                }
            }
            T {
                name = "palette_options"
                Style {
                    display("inline-block")
                    vertical_align("top")
                    width(23.2)
                    margin_left(1.0)
                }
                // Row 1: gamut extremes and neutral endpoints.
                color_swatch("swatch_0", "Red", 0, [1.0, 0.0, 0.0, 1.0], "1.0,0.0,0.0,1.0")
                color_swatch("swatch_1", "Green", 1, [0.0, 1.0, 0.0, 1.0], "0.0,1.0,0.0,1.0")
                color_swatch("swatch_2", "Blue", 2, [0.0, 0.0, 1.0, 1.0], "0.0,0.0,1.0,1.0")
                color_swatch("swatch_3", "Cyan", 3, [0.0, 1.0, 1.0, 1.0], "0.0,1.0,1.0,1.0")
                color_swatch("swatch_4", "Yellow", 4, [1.0, 1.0, 0.0, 1.0], "1.0,1.0,0.0,1.0")
                color_swatch("swatch_5", "Magenta", 5, [1.0, 0.0, 1.0, 1.0], "1.0,0.0,1.0,1.0")
                color_swatch("swatch_6", "Black", 6, [0.0, 0.0, 0.0, 1.0], "0.0,0.0,0.0,1.0")
                color_swatch("swatch_7", "White", 7, [1.0, 1.0, 1.0, 1.0], "1.0,1.0,1.0,1.0")

                // Row 2: four intermediate hues at full value, followed by the
                // same tessellation at half value.
                color_swatch("swatch_8", "Orange", 8, [1.0, 0.5, 0.0, 1.0], "1.0,0.5,0.0,1.0")
                color_swatch("swatch_9", "Chartreuse", 9, [0.5, 1.0, 0.0, 1.0], "0.5,1.0,0.0,1.0")
                color_swatch("swatch_10", "Azure", 10, [0.0, 0.5, 1.0, 1.0], "0.0,0.5,1.0,1.0")
                color_swatch("swatch_11", "Violet", 11, [0.5, 0.0, 1.0, 1.0], "0.5,0.0,1.0,1.0")
                color_swatch("swatch_12", "Dark Orange", 12, [0.5, 0.25, 0.0, 1.0], "0.5,0.25,0.0,1.0")
                color_swatch("swatch_13", "Dark Chartreuse", 13, [0.25, 0.5, 0.0, 1.0], "0.25,0.5,0.0,1.0")
                color_swatch("swatch_14", "Dark Azure", 14, [0.0, 0.25, 0.5, 1.0], "0.0,0.25,0.5,1.0")
                color_swatch("swatch_15", "Dark Violet", 15, [0.25, 0.0, 0.5, 1.0], "0.25,0.0,0.5,1.0")
            }
            Selection.root("#palette_options").visual_feedback(false) { name = "color_panel_selection" }
        }
    })
}

export fn color_panel(title, title_color, panel_background_color) {
    return accordion({
        unlit = true
        root_name = "color_panel_root"
        width_gu = COLOR_PANEL_WIDTH_GU
        unit_scale = EDITOR_PANEL_UNIT_SCALE
        background_color = panel_background_color
        toggle_background_color = EDITOR_ACCORDION_TOGGLE_BACKGROUND
        children = [panel_title(title, title_color)]
        body = color_panel_body()
    })
}

// ── pose_capture_panel ────────────────────────────────────────────────────────

let POSE_PANEL_WIDTH_GU = 29.5
let POSE_PANEL_CONTENT_HEIGHT_GU = 51.0
let POSE_PANEL_STATUS_HEIGHT_GU = 2.5
let POSE_PANEL_TOTAL_HEIGHT_GU = TITLE_BAR_HEIGHT_GU + TITLE_CONTENT_GAP_GU + POSE_PANEL_CONTENT_HEIGHT_GU + TITLE_CONTENT_GAP_GU + POSE_PANEL_STATUS_HEIGHT_GU

export fn pose_capture_panel_body() {
    return accordion_body(T {
        name = "pose_capture_panel_body"
        Style { display("block") width(100%) }
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
                background_color(EDITOR_DARK_CHROME_BACKGROUND)
                background_z(-0.01)
                color = EDITOR_DARK_CHROME_TEXT
            }
            Text {
                name = "pose_panel_status_value"
                "idle"
            }
        }
    })
}

export fn pose_capture_panel(title, title_color, panel_background_color) {
    return accordion({
        unlit = true
        root_name = "pose_capture_panel_root"
        width_gu = POSE_PANEL_WIDTH_GU
        unit_scale = EDITOR_PANEL_UNIT_SCALE
        background_color = panel_background_color
        toggle_background_color = EDITOR_ACCORDION_TOGGLE_BACKGROUND
        children = [panel_title(title, title_color)]
        body = pose_capture_panel_body()
    })
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
            background_color(EDITOR_DARK_CHROME_BACKGROUND)
            color = EDITOR_DARK_CHROME_TEXT
        }
        Text { label }
    }
    return root
}

export fn world_panel_body(working_file_path) {
    let status = world_panel_status("idle")
    return accordion_body(T {
        name = "world_panel_body"
        Style { display("block") width(100%) }
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
                background_color(EDITOR_DARK_CHROME_BACKGROUND)
                background_z(-0.01)
                color = EDITOR_DARK_CHROME_TEXT
            }
            status
        }
    })
}

export fn world_panel(title, items, title_color, panel_background_color, item_background_color, working_file_path) {
    let _unused_items = items
    let _unused_item_background_color = item_background_color
    return accordion({
        unlit = true
        root_name = "world_panel_root"
        width_gu = WORLD_PANEL_WIDTH_GU
        unit_scale = EDITOR_PANEL_UNIT_SCALE
        background_color = panel_background_color
        toggle_background_color = EDITOR_ACCORDION_TOGGLE_BACKGROUND
        children = [
            panel_title(title, title_color),
            panel_button("save_button", "Save"),
            panel_button("load_button", "Load"),
        ]
        body = world_panel_body(working_file_path)
    })
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

export fn inspector_panel_body() {
    return accordion_body(T {
        name = "inspector_panel_body"
        Style { display("block") width(100%) }
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
    })
}

export fn inspector_panel(title, items, title_color, panel_background_color, item_background_color) {
    let _unused_items = items
    let _unused_item_background_color = item_background_color
    return accordion({
        unlit = true
        root_name = "inspector_panel_root"
        width_gu = INSPECTOR_PANEL_WIDTH_GU
        unit_scale = EDITOR_PANEL_UNIT_SCALE
        background_color = panel_background_color
        toggle_background_color = EDITOR_ACCORDION_TOGGLE_BACKGROUND
        children = [
            panel_title(title, title_color),
            T {
                name = "pin_slot"
                Style { display("block") }
                panel_button("pin_button", "Pin")
            },
        ]
        body = inspector_panel_body()
    })
}

// ── asset_panel ───────────────────────────────────────────────────────────────

// 39.0 GU baseline × 1.20. Keep this at the shared shell source so every
// editor layout receives the same readable Assets-panel width.
let ASSET_PANEL_WIDTH_GU = 46.8
let ASSET_PANEL_CONTENT_HEIGHT_GU = 57.0
let ASSET_PANEL_TOTAL_HEIGHT_GU = TITLE_BAR_HEIGHT_GU + TITLE_CONTENT_GAP_GU + ASSET_PANEL_CONTENT_HEIGHT_GU

export fn asset_panel_body(items, item_background_color) {
    return accordion_body(T {
        name = "asset_panel_body"
        Style { display("block") width(100%) }
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
    })
}

export fn asset_panel(title, items, title_color, panel_background_color, item_background_color) {
    return accordion({
        unlit = true
        root_name = "assets_root"
        width_gu = ASSET_PANEL_WIDTH_GU
        unit_scale = EDITOR_PANEL_UNIT_SCALE
        background_color = panel_background_color
        toggle_background_color = EDITOR_ACCORDION_TOGGLE_BACKGROUND
        children = [panel_title(title, title_color)]
        body = asset_panel_body(items, item_background_color)
    })
}

// ── grid_panel ───────────────────────────────────────────────────────────────

// 29.5 GU baseline × 1.45. Grid rows now include Local/World alongside the
// existing visibility, enablement, binding, and deletion controls.
let GRID_PANEL_WIDTH_GU = 42.775
let GRID_PANEL_CONTENT_HEIGHT_GU = 51.0
let GRID_PANEL_ADD_BUTTON_HEIGHT_GU = 3.0
let GRID_PANEL_TOTAL_HEIGHT_GU = TITLE_BAR_HEIGHT_GU + TITLE_CONTENT_GAP_GU + GRID_PANEL_CONTENT_HEIGHT_GU + GAP_GU + GRID_PANEL_ADD_BUTTON_HEIGHT_GU

export fn grid_panel_body() {
    return accordion_body(T {
        name = "grid_panel_body"
        Style { display("block") width(100%) }
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
                background_color(EDITOR_DARK_CHROME_BACKGROUND)
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
                    color = EDITOR_DARK_CHROME_TEXT
                }
                Text { "Add Grid" }
            }

        }
    })
}

export fn grid_panel(title, items, title_color, panel_background_color, item_background_color) {
    let _unused_items = items
    let _unused_item_background_color = item_background_color
    let visibility_control = T {
        name = "title_icon_wrap"
        Raycastable.click_only()
        Style {
            display("flex")
            width(5.0)
            height(3.5)
            align_items("center")
            justify_content("center")
        }
        T.scale(0.25 * EDITOR_PANEL_UNIT_SCALE, 0.25 * EDITOR_PANEL_UNIT_SCALE, 1.0) { grid_visibility_icon() }
    }
    return accordion({
        unlit = true
        root_name = "grid_panel_root"
        width_gu = GRID_PANEL_WIDTH_GU
        unit_scale = EDITOR_PANEL_UNIT_SCALE
        background_color = panel_background_color
        toggle_background_color = EDITOR_ACCORDION_TOGGLE_BACKGROUND
        children = [panel_title(title, title_color), visibility_control]
        body = grid_panel_body()
    })
}
