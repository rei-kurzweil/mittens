// panel_items.mms — panel item factories (=^･ω･^=)
//
// Consolidates all panel item-level components (buttons, rows, status text,
// content bodies) that panels import.

// ── paint_panel_item ──────────────────────────────────────────────────────────

export fn paint_panel_item(label, icon, item_background_color, title_color) {
    return T {
        name = "paint_panel_item"
        Option {}
        Raycastable.enabled()
        Style {
            display("inline-block")
            width(7.0)
            height(7.5)
            margin(0.4)
            background_color(item_background_color)
            background_z(-0.01)
            text_align("center")
            vertical_align("middle")
        }
        T {
            Style {
                display("block")
                width(100%)
            }
            T {
                Style {
                    display("block")
                    height(4.0)
                    margin_top(0.4)
                    margin_bottom(0.3)
                    text_align("center")
                    vertical_align("middle")
                }
                FitBounds.to_container() {
                    T {
                        icon
                    }
                }
            }
            T {
                Style {
                    display("block")
                    width(100%)
                    padding_xy(0.2, 0.0)
                    color = [0,0,0,1]
                    word_wrap("normal")
                }
                Text { label }
            }
        }
    }
}

// ── world_panel_status ────────────────────────────────────────────────────────

export fn world_panel_status(label) {
    let root = T {
        name = "panel_status_root"
        T.position(0.0, 0.0, 0.0) {
            Text {
                name = "panel_status_value"
                label
            }
        }
    }

    return root
}

// ── world_panel_content ───────────────────────────────────────────────────────

let TEXT_SCALE = 0.08
let DEFAULT_ROW_BG = [0.92, 0.97, 0.92, 1.0]
let SELECTED_ROW_BG = [1.00, 0.88, 0.20, 0.96]

fn world_panel_row(row_name, label, bg) {
    let row = T {
        name = row_name
        Raycastable.click_only()
        Style {
            display("block")
            width(100%)
            margin_xy(0.25, 0.20)
            padding_xy(0.55, 0.45)
            font_size(1)
            background_color = bg
            background_z(-0.01)
        }
        T {
            Text {
                label
                C.rgba(0.06, 0.09, 0.08, 1.0)
            }
        }
    }
    return row
}

export fn world_panel_content_selected(items, selected_index, item_background_color) {
    let root = T {
        name = "world_panel_content_root"
        Style {
            display("block")
            width(100%)
        }

        T {
            name = "rows_mount"
            Style {
                display("block")
                width(100%)
            }

            let idx = 0
            for item in items {
                let row_name = "item_" + idx
                let bg = item_background_color
                if idx == selected_index {
                    bg = SELECTED_ROW_BG
                }
                world_panel_row(row_name, item, bg)
                idx = idx + 1
            }
        }
    }

    return root
}

export fn world_panel_content(items, item_background_color) {
    return world_panel_content_selected(items, -1, item_background_color)
}

// ── inspector_panel_content ───────────────────────────────────────────────────

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
